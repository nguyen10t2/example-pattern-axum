use chrono::Utc;
use std::{collections::HashSet, sync::Arc};
use tracing::info;
use uuid::Uuid;

use crate::{
    domain::{
        GroupRole,
        groups::{
            membership::{self, member_entries},
            repository::GroupRepository,
        },
        settlements::{
            entity::NewSettlementEntity, mapper::SettlementMapper, repository::SettlementRepository,
            request::CreateSettlementRequest, response::SettlementResponse,
        },
        shared::{PaginatedResponse, PaginationQuery},
    },
    errors::{AppError, BusinessError},
    utils::cache::{Cache, CacheStore},
};
use sqlx::PgPool;

pub struct SettlementService<SR: SettlementRepository, GR: GroupRepository> {
    settlement_repo: SR,
    group_repo: GR,
    cache: Arc<Cache>,
    pool: PgPool,
}

impl<SR: SettlementRepository, GR: GroupRepository> SettlementService<SR, GR> {
    /// Ghép repo settlement/group, cache và pool thành service.
    pub const fn new(settlement_repo: SR, group_repo: GR, cache: Arc<Cache>, pool: PgPool) -> Self {
        Self { settlement_repo, group_repo, cache, pool }
    }

    /// Ghi nhận một giao dịch trả nợ, xóa cache summary nhóm.
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember` khi ngoài nhóm, `UserNotInGroup` khi sender/receiver ngoài nhóm.
    pub async fn create(
        &self,
        data: CreateSettlementRequest,
        current_user_id: Uuid,
    ) -> Result<SettlementResponse, AppError> {
        let members = member_entries(&self.cache, &self.pool, &self.group_repo, data.group_id).await?;
        let member_ids: HashSet<Uuid> = members.iter().map(|m| m.user_id).collect();

        if !member_ids.contains(&current_user_id) {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        }

        if !member_ids.contains(&data.sender_id) || !member_ids.contains(&data.receiver_id) {
            return Err(AppError::Business(BusinessError::UserNotInGroup("Sender or receiver".to_string())));
        }

        let new_settlement = NewSettlementEntity {
            id: Uuid::now_v7(),
            group_id: data.group_id,
            sender_id: data.sender_id,
            receiver_id: data.receiver_id,
            amount: data.amount,
            currency: data.currency,
            settled_at: data.settled_at.unwrap_or_else(Utc::now),
        };

        let settlement = self.settlement_repo.create(&self.pool, &new_settlement).await?;

        // Invalidate group summary cache
        self.cache.delete(&format!("group_summary:{}", data.group_id)).await;

        info!(
            settlement_id = %settlement.id,
            group_id = %settlement.group_id,
            amount = settlement.amount,
            "Settlement created"
        );

        Ok(SettlementMapper::to_response(&settlement))
    }

    /// Lấy settlement theo id (phải là thành viên nhóm).
    ///
    /// # Errors
    ///
    /// Trả `SettlementNotFound` khi id không tồn tại, `NotGroupMember` khi ngoài nhóm.
    pub async fn find_by_id(&self, id: Uuid, current_user_id: Uuid) -> Result<SettlementResponse, AppError> {
        let settlement = self.settlement_repo.find_by_id(&self.pool, id).await?;
        let settlement = settlement.ok_or(AppError::Business(BusinessError::SettlementNotFound))?;

        self.ensure_membership(settlement.group_id, current_user_id).await?;
        Ok(SettlementMapper::to_response(&settlement))
    }

    /// Liệt kê settlements của nhóm có phân trang (phải là thành viên).
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember` khi ngoài nhóm.
    pub async fn find_by_group(
        &self,
        group_id: Uuid,
        current_user_id: Uuid,
        pagination: PaginationQuery,
    ) -> Result<PaginatedResponse<SettlementResponse>, AppError> {
        let limit = pagination.limit();
        let offset = pagination.offset();

        let ((), (items, total)) = tokio::try_join!(self.ensure_membership(group_id, current_user_id), async {
            self.settlement_repo
                .find_paginated_by_group(&self.pool, group_id, limit, offset)
                .await
                .map_err(AppError::from)
        },)?;

        let response_items = SettlementMapper::to_response_list_with_users(items);

        Ok(PaginatedResponse::new(response_items, total, pagination.page(), limit))
    }

    /// Hủy settlement (2 bên tham gia hoặc admin), xóa cache summary nhóm.
    ///
    /// # Errors
    ///
    /// Trả `SettlementNotFound` khi id không tồn tại, `NotGroupMember` khi ngoài nhóm,
    /// `DeletePermissionDenied` khi không có quyền.
    pub async fn cancel_settlement(&self, id: Uuid, current_user_id: Uuid) -> Result<(), AppError> {
        let settlement = self.settlement_repo.find_by_id(&self.pool, id).await?;
        let settlement = settlement.ok_or(AppError::Business(BusinessError::SettlementNotFound))?;

        let members = member_entries(&self.cache, &self.pool, &self.group_repo, settlement.group_id).await?;
        let member = members.iter().find(|m| m.user_id == current_user_id);

        let Some(member) = member else {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        };

        if settlement.sender_id != current_user_id
            && settlement.receiver_id != current_user_id
            && member.role != GroupRole::ADMIN
        {
            return Err(AppError::Business(BusinessError::DeletePermissionDenied));
        }

        self.settlement_repo.soft_delete(&self.pool, id).await?;
        self.cache.delete(&format!("group_summary:{}", settlement.group_id)).await;

        info!(settlement_id = %id, deleted_by = %current_user_id, "Settlement cancelled");
        Ok(())
    }

    /// Chặn nếu user không phải thành viên nhóm của settlement.
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember` khi user ngoài nhóm.
    async fn ensure_membership(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        membership::ensure_membership(&self.cache, &self.pool, &self.group_repo, group_id, user_id).await
    }
}
