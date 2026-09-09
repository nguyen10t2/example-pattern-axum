use chrono::Utc;
use std::{collections::HashSet, sync::Arc};
use tracing::info;
use uuid::Uuid;

use crate::{
    domain::{
        GroupRole, SplitType,
        expenses::{
            entity::{NewExpenseEntity, NewExpenseShareEntity},
            mapper::ExpenseMapper,
            repository::ExpenseRepository,
            request::CreateExpenseRequest,
            response::ExpenseResponse,
            strategy::{SplitContext, SplitShareInput, SplitStrategyFactory},
        },
        groups::{
            membership::{self, member_entries},
            repository::GroupRepository,
        },
        shared::{PaginatedResponse, PaginationQuery},
    },
    errors::{AppError, BusinessError},
    utils::cache::{Cache, CacheStore},
};
use sqlx::PgPool;

pub struct ExpenseService<ER: ExpenseRepository, GR: GroupRepository> {
    expense_repo: ER,
    group_repo: GR,
    cache: Arc<Cache>,
    pool: PgPool,
}

impl<ER: ExpenseRepository, GR: GroupRepository> ExpenseService<ER, GR> {
    /// Ghép repo expense/group, cache và pool thành service.
    pub const fn new(expense_repo: ER, group_repo: GR, cache: Arc<Cache>, pool: PgPool) -> Self {
        Self { expense_repo, group_repo, cache, pool }
    }

    /// Tạo expense + shares trong 1 transaction, xóa cache summary nhóm.
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember`/`PayerNotInGroup`/`UserNotInGroup` khi sai thành viên,
    /// lỗi strategy khi split không hợp lệ, lỗi DB khi ghi.
    pub async fn create(&self, data: CreateExpenseRequest, created_by_id: Uuid) -> Result<ExpenseResponse, AppError> {
        // 1. Authorization & Strategy Validation
        self.validate_creation(&data, created_by_id).await?;

        let expense_id = Uuid::now_v7();
        let split_type = data.split_type.unwrap_or(SplitType::EQUAL);
        let expense_date = data.expense_date.unwrap_or_else(Utc::now);

        let new_expense = NewExpenseEntity {
            id: expense_id,
            group_id: data.group_id,
            created_by_id,
            payer_id: data.payer_id,
            amount: data.amount,
            currency: data.currency,
            description: data.description,
            split_type,
            expense_date,
        };

        let mut tx = self.pool.begin().await?;

        let expense = self.expense_repo.create(&mut *tx, &new_expense).await?;

        let new_shares: Vec<NewExpenseShareEntity> = data
            .shares
            .iter()
            .map(|s| NewExpenseShareEntity {
                id: Uuid::now_v7(),
                expense_id: expense.id,
                user_id: s.user_id,
                share_amount: s.share_amount,
                share_percentage: s.share_percentage,
            })
            .collect();

        let shares = self.expense_repo.create_shares(&mut *tx, &new_shares).await?;

        tx.commit().await?;

        // Invalidate group summary cache
        self.cache.delete(&format!("group_summary:{}", data.group_id)).await;

        info!(expense_id = %expense.id, group_id = %expense.group_id, "Expense created");

        let share_responses = shares.into_iter().map(|s| ExpenseMapper::to_share_response(&s, None)).collect();

        Ok(ExpenseMapper::to_response_from_entity(expense, None, Some(share_responses)))
    }

    /// Kiểm tra quyền + tính hợp lệ của split trước khi tạo expense.
    ///
    /// # Errors
    ///
    /// Trả lỗi membership hoặc lỗi strategy khi split sai.
    async fn validate_creation(&self, data: &CreateExpenseRequest, created_by_id: Uuid) -> Result<(), AppError> {
        let members = member_entries(&self.cache, &self.pool, &self.group_repo, data.group_id).await?;
        let member_ids: HashSet<Uuid> = members.iter().map(|m| m.user_id).collect();

        if !member_ids.contains(&created_by_id) {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        }

        if !member_ids.contains(&data.payer_id) {
            return Err(AppError::Business(BusinessError::PayerNotInGroup));
        }

        for share in &data.shares {
            if !member_ids.contains(&share.user_id) {
                return Err(AppError::Business(BusinessError::UserNotInGroup(share.user_id.to_string())));
            }
        }

        let split_type = data.split_type.unwrap_or(SplitType::EQUAL);
        let strategy = SplitStrategyFactory::get_strategy(&split_type);

        let context = SplitContext {
            total_amount: data.amount,
            shares: data
                .shares
                .iter()
                .map(|s| SplitShareInput {
                    user_id: s.user_id,
                    share_amount: s.share_amount,
                    share_percentage: s.share_percentage,
                })
                .collect(),
        };

        strategy.validate(&context)?;
        Ok(())
    }

    /// Lấy expense kèm shares; nếu có user thì check membership song song.
    ///
    /// # Errors
    ///
    /// Trả `ExpenseNotFound` khi id không tồn tại, `NotGroupMember` khi ngoài nhóm.
    pub async fn find_by_id(&self, id: Uuid, current_user_id: Option<Uuid>) -> Result<ExpenseResponse, AppError> {
        let expense = self.expense_repo.find_by_id(&self.pool, id).await?;
        let expense = expense.ok_or(AppError::Business(BusinessError::ExpenseNotFound))?;

        let ((), shares) = if let Some(user_id) = current_user_id {
            tokio::try_join!(self.ensure_membership(expense.group_id, user_id), async {
                self.expense_repo.find_shares_by_expense(&self.pool, id).await.map_err(AppError::from)
            })?
        } else {
            ((), self.expense_repo.find_shares_by_expense(&self.pool, id).await?)
        };

        let share_responses = shares.into_iter().map(ExpenseMapper::to_share_response_with_user).collect();

        Ok(ExpenseMapper::to_response_with_payer(expense, Some(share_responses)))
    }

    /// Liệt kê expense của nhóm có phân trang (phải là thành viên).
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember` khi ngoài nhóm.
    pub async fn find_by_group(
        &self,
        group_id: Uuid,
        current_user_id: Uuid,
        pagination: PaginationQuery,
    ) -> Result<PaginatedResponse<ExpenseResponse>, AppError> {
        let limit = pagination.limit();
        let offset = pagination.offset();

        let ((), (items, total)) = tokio::try_join!(self.ensure_membership(group_id, current_user_id), async {
            self.expense_repo.find_by_group(&self.pool, group_id, limit, offset).await.map_err(AppError::from)
        },)?;

        let response_items = items.into_iter().map(|e| ExpenseMapper::to_response_with_payer(e, None)).collect();

        Ok(PaginatedResponse::new(response_items, total, pagination.page(), limit))
    }

    /// Xóa mềm expense (người tạo hoặc admin), xóa cache summary nhóm.
    ///
    /// # Errors
    ///
    /// Trả `ExpenseNotFound` khi id không tồn tại, `NotGroupMember` khi ngoài nhóm,
    /// `DeletePermissionDenied` khi không có quyền.
    pub async fn delete_expense(&self, id: Uuid, current_user_id: Uuid) -> Result<(), AppError> {
        let expense = self.expense_repo.find_by_id(&self.pool, id).await?;
        let expense = expense.ok_or(AppError::Business(BusinessError::ExpenseNotFound))?;

        let members = member_entries(&self.cache, &self.pool, &self.group_repo, expense.group_id).await?;
        let member = members.iter().find(|m| m.user_id == current_user_id);

        let Some(member) = member else {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        };

        if expense.created_by_id != current_user_id && member.role != GroupRole::ADMIN {
            return Err(AppError::Business(BusinessError::DeletePermissionDenied));
        }

        self.expense_repo.soft_delete(&self.pool, id).await?;
        self.cache.delete(&format!("group_summary:{}", expense.group_id)).await;

        info!(expense_id = %id, deleted_by = %current_user_id, "Expense deleted");
        Ok(())
    }

    /// Chặn nếu user không phải thành viên nhóm của expense.
    ///
    /// # Errors
    ///
    /// Trả `NotGroupMember` khi user ngoài nhóm.
    async fn ensure_membership(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        membership::ensure_membership(&self.cache, &self.pool, &self.group_repo, group_id, user_id).await
    }
}
