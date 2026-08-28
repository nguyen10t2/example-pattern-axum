use chrono::Utc;
use std::{collections::HashSet, sync::Arc};
use tracing::info;
use uuid::Uuid;

use crate::{
    domain::{
        GroupRole,
        groups::repository::GroupRepository,
        settlements::{
            entity::NewSettlementEntity, mapper::SettlementMapper, repository::SettlementRepository,
            request::CreateSettlementRequest, response::SettlementResponse,
        },
        shared::{PaginatedResponse, PaginationQuery},
    },
    errors::{AppError, BusinessError},
    utils::cache::CacheStore,
};

pub struct SettlementService<SR: SettlementRepository, GR: GroupRepository> {
    settlement_repo: SR,
    group_repo: GR,
    cache: Arc<dyn CacheStore>,
}

impl<SR: SettlementRepository, GR: GroupRepository> SettlementService<SR, GR> {
    pub fn new(settlement_repo: SR, group_repo: GR, cache: Arc<dyn CacheStore>) -> Self {
        Self { settlement_repo, group_repo, cache }
    }

    pub async fn create(
        &self,
        data: CreateSettlementRequest,
        current_user_id: Uuid,
    ) -> Result<SettlementResponse, AppError> {
        let members = self.group_repo.find_members(data.group_id).await?;
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

        let settlement = self.settlement_repo.create(&new_settlement).await?;

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

    pub async fn find_by_id(&self, id: Uuid, current_user_id: Uuid) -> Result<SettlementResponse, AppError> {
        let settlement = self.settlement_repo.find_by_id(id).await?;
        let settlement = settlement.ok_or(AppError::Business(BusinessError::SettlementNotFound))?;

        self.ensure_membership(settlement.group_id, current_user_id).await?;
        Ok(SettlementMapper::to_response(&settlement))
    }

    pub async fn find_by_group(
        &self,
        group_id: Uuid,
        current_user_id: Uuid,
        pagination: PaginationQuery,
    ) -> Result<PaginatedResponse<SettlementResponse>, AppError> {
        self.ensure_membership(group_id, current_user_id).await?;

        let limit = pagination.limit();
        let offset = pagination.offset();

        let (items, total) = self.settlement_repo.find_paginated_by_group(group_id, limit, offset).await?;

        let response_items = SettlementMapper::to_response_list_with_users(&items);

        Ok(PaginatedResponse::new(response_items, total, pagination.page(), limit))
    }

    pub async fn cancel_settlement(&self, id: Uuid, current_user_id: Uuid) -> Result<(), AppError> {
        let settlement = self.settlement_repo.find_by_id(id).await?;
        let settlement = settlement.ok_or(AppError::Business(BusinessError::SettlementNotFound))?;

        let members = self.group_repo.find_members(settlement.group_id).await?;
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

        self.settlement_repo.soft_delete(id).await?;
        self.cache.delete(&format!("group_summary:{}", settlement.group_id)).await;

        info!(settlement_id = %id, deleted_by = %current_user_id, "Settlement cancelled");
        Ok(())
    }

    async fn ensure_membership(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let members = self.group_repo.find_members(group_id).await?;
        if !members.iter().any(|m| m.user_id == user_id) {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        }
        Ok(())
    }
}
