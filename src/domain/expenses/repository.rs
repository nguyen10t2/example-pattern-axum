use crate::domain::expenses::entity::{
    ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
    NewExpenseEntity, NewExpenseShareEntity,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
pub trait ExpenseRepository: Send + Sync {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewExpenseEntity,
    ) -> Result<ExpenseEntity, sqlx::Error>;

    async fn create_shares<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        shares: &[NewExpenseShareEntity],
    ) -> Result<Vec<ExpenseShareEntity>, sqlx::Error>;

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseWithPayer>, sqlx::Error>;

    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error>;

    async fn find_by_group_with_shares<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error>;

    async fn find_shares_by_expense<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        expense_id: Uuid,
    ) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error>;

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseEntity>, sqlx::Error>;
}
