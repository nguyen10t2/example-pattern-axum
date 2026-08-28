use crate::domain::expenses::entity::{
    ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
    NewExpenseEntity, NewExpenseShareEntity,
};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait ExpenseRepository: Send + Sync {
    async fn create(&self, data: &NewExpenseEntity) -> Result<ExpenseEntity, sqlx::Error>;
    async fn create_shares(&self, shares: &[NewExpenseShareEntity]) -> Result<Vec<ExpenseShareEntity>, sqlx::Error>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<ExpenseWithPayer>, sqlx::Error>;
    async fn find_by_group(
        &self,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error>;
    async fn find_by_group_with_shares(&self, group_id: Uuid) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error>;
    async fn find_shares_by_expense(&self, expense_id: Uuid) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error>;
    async fn soft_delete(&self, id: Uuid) -> Result<Option<ExpenseEntity>, sqlx::Error>;
}
