use crate::domain::expenses::{
    entity::{
        ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
        NewExpenseEntity, NewExpenseShareEntity,
    },
    repository::ExpenseRepository,
};
use async_trait::async_trait;
use uuid::Uuid;

#[derive(Clone)]
pub struct PostgresExpenseRepository {
    pool: sqlx::PgPool,
}

impl PostgresExpenseRepository {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExpenseRepository for PostgresExpenseRepository {
    async fn create(&self, data: &NewExpenseEntity) -> Result<ExpenseEntity, sqlx::Error> {
        sqlx::query_as::<_, ExpenseEntity>(
            "INSERT INTO expenses (
                id, group_id, created_by_id, payer_id, amount,
                currency, description, split_type, expense_date
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING *",
        )
        .bind(data.id)
        .bind(data.group_id)
        .bind(data.created_by_id)
        .bind(data.payer_id)
        .bind(data.amount)
        .bind(data.currency)
        .bind(&data.description)
        .bind(data.split_type)
        .bind(data.expense_date)
        .fetch_one(&self.pool)
        .await
    }

    async fn create_shares(&self, shares: &[NewExpenseShareEntity]) -> Result<Vec<ExpenseShareEntity>, sqlx::Error> {
        if shares.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<Uuid> = shares.iter().map(|s| s.id).collect();
        let expense_ids: Vec<Uuid> = shares.iter().map(|s| s.expense_id).collect();
        let user_ids: Vec<Uuid> = shares.iter().map(|s| s.user_id).collect();
        let amounts: Vec<i64> = shares.iter().map(|s| s.share_amount).collect();
        let percentages: Vec<Option<i32>> = shares.iter().map(|s| s.share_percentage).collect();

        sqlx::query_as::<_, ExpenseShareEntity>(
            "INSERT INTO expense_shares (id, expense_id, user_id, share_amount, share_percentage)
             SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::bigint[], $5::integer[])
             RETURNING *",
        )
        .bind(&ids)
        .bind(&expense_ids)
        .bind(&user_ids)
        .bind(&amounts)
        .bind(&percentages)
        .fetch_all(&self.pool)
        .await
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<ExpenseWithPayer>, sqlx::Error> {
        sqlx::query_as::<_, ExpenseWithPayer>(
            "SELECT e.id, e.group_id, e.created_by_id, e.payer_id, u.full_name AS payer_name,
                    e.amount, e.currency, e.description, e.split_type, e.expense_date,
                    e.deleted_at, e.created_at, e.updated_at
             FROM expenses e
             INNER JOIN users u ON e.payer_id = u.id
             WHERE e.id = $1 AND e.deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    async fn find_by_group(
        &self,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error> {
        let count_row: (i64,) =
            sqlx::query_as("SELECT COUNT(*)::bigint FROM expenses WHERE group_id = $1 AND deleted_at IS NULL")
                .bind(group_id)
                .fetch_one(&self.pool)
                .await?;

        let items = sqlx::query_as::<_, ExpenseWithPayer>(
            "SELECT e.id, e.group_id, e.created_by_id, e.payer_id, u.full_name AS payer_name,
                    e.amount, e.currency, e.description, e.split_type, e.expense_date,
                    e.deleted_at, e.created_at, e.updated_at
             FROM expenses e
             INNER JOIN users u ON e.payer_id = u.id
             WHERE e.group_id = $1 AND e.deleted_at IS NULL
             ORDER BY e.expense_date DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(group_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        Ok((items, count_row.0))
    }

    async fn find_by_group_with_shares(&self, group_id: Uuid) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error> {
        let expenses = sqlx::query_as::<_, ExpenseEntity>(
            "SELECT * FROM expenses
             WHERE group_id = $1 AND deleted_at IS NULL
             ORDER BY expense_date DESC",
        )
        .bind(group_id)
        .fetch_all(&self.pool)
        .await?;

        if expenses.is_empty() {
            return Ok(vec![]);
        }

        let expense_ids: Vec<Uuid> = expenses.iter().map(|e| e.id).collect();
        let all_shares = sqlx::query_as::<_, ExpenseShareEntity>(
            "SELECT * FROM expense_shares
             WHERE expense_id = ANY($1)",
        )
        .bind(&expense_ids)
        .fetch_all(&self.pool)
        .await?;

        let mut results = Vec::with_capacity(expenses.len());
        for expense in expenses {
            let shares = all_shares.iter().filter(|s| s.expense_id == expense.id).cloned().collect();
            results.push(ExpenseWithSharesEntity { expense, shares });
        }

        Ok(results)
    }

    async fn find_shares_by_expense(&self, expense_id: Uuid) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error> {
        sqlx::query_as::<_, ExpenseShareWithUser>(
            "SELECT es.id, es.expense_id, es.user_id, u.full_name AS user_name,
                    es.share_amount, es.share_percentage, es.created_at, es.updated_at
             FROM expense_shares es
             INNER JOIN users u ON es.user_id = u.id
             WHERE es.expense_id = $1",
        )
        .bind(expense_id)
        .fetch_all(&self.pool)
        .await
    }

    async fn soft_delete(&self, id: Uuid) -> Result<Option<ExpenseEntity>, sqlx::Error> {
        sqlx::query_as::<_, ExpenseEntity>(
            "UPDATE expenses
             SET deleted_at = now()
             WHERE id = $1 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }
}
