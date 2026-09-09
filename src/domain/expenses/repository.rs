use crate::domain::expenses::entity::{
    ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
    NewExpenseEntity, NewExpenseShareEntity,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
/// Truy xuất expense và shares (nhận pool hoặc transaction qua `executor`).
pub trait ExpenseRepository: Send + Sync {
    /// Chèn expense mới.
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewExpenseEntity,
    ) -> Result<ExpenseEntity, sqlx::Error>;

    /// Chèn batch các shares của một expense (1 round trip).
    async fn create_shares<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        shares: &[NewExpenseShareEntity],
    ) -> Result<Vec<ExpenseShareEntity>, sqlx::Error>;

    /// Tìm expense kèm payer theo id.
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseWithPayer>, sqlx::Error>;

    /// Liệt kê expense của nhóm có phân trang, kèm tổng số dòng.
    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error>;

    /// Liệt kê expense của nhóm kèm toàn bộ shares (cho summary).
    async fn find_by_group_with_shares<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error>;

    /// Liệt kê shares của một expense kèm user.
    async fn find_shares_by_expense<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        expense_id: Uuid,
    ) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error>;

    /// Xóa mềm expense, trả `None` nếu id không tồn tại.
    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseEntity>, sqlx::Error>;
}
