use crate::domain::expenses::{
    entity::{
        ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
        NewExpenseEntity, NewExpenseShareEntity,
    },
    repository::ExpenseRepository,
};
use async_trait::async_trait;
use sqlx::{Executor, FromRow, Postgres, Row};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct PostgresExpenseRepository;

impl PostgresExpenseRepository {
    /// Tạo repository expense (stateless).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

/// Gom shares theo expense trong O(E+S): 1 pass `HashMap` rồi ráp.
///
/// Thay vòng `filter` O(E×S) cho từng expense — cải thiện đo được ở bench
/// `share_grouping` (E càng lớn càng rõ).
#[must_use]
pub fn assemble_expenses_with_shares(
    expenses: Vec<ExpenseEntity>,
    shares: Vec<ExpenseShareEntity>,
) -> Vec<ExpenseWithSharesEntity> {
    let mut by_expense: HashMap<Uuid, Vec<ExpenseShareEntity>> = HashMap::with_capacity(expenses.len());
    for share in shares {
        by_expense.entry(share.expense_id).or_default().push(share);
    }
    expenses
        .into_iter()
        .map(|expense| {
            let shares = by_expense.remove(&expense.id).unwrap_or_default();
            ExpenseWithSharesEntity { expense, shares }
        })
        .collect()
}

#[async_trait]
impl ExpenseRepository for PostgresExpenseRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewExpenseEntity,
    ) -> Result<ExpenseEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, ExpenseEntity>(
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
        .fetch_one(executor)
        .await
    }

    async fn create_shares<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        shares: &[NewExpenseShareEntity],
    ) -> Result<Vec<ExpenseShareEntity>, sqlx::Error> {
        if shares.is_empty() {
            return Ok(vec![]);
        }

        let ids: Vec<Uuid> = shares.iter().map(|s| s.id).collect();
        let expense_ids: Vec<Uuid> = shares.iter().map(|s| s.expense_id).collect();
        let user_ids: Vec<Uuid> = shares.iter().map(|s| s.user_id).collect();
        let amounts: Vec<i64> = shares.iter().map(|s| s.share_amount).collect();
        let percentages: Vec<Option<i32>> = shares.iter().map(|s| s.share_percentage).collect();

        sqlx::query_as::<Postgres, ExpenseShareEntity>(
            "INSERT INTO expense_shares (id, expense_id, user_id, share_amount, share_percentage)
             SELECT * FROM UNNEST($1::uuid[], $2::uuid[], $3::uuid[], $4::bigint[], $5::integer[])
             RETURNING *",
        )
        .bind(&ids)
        .bind(&expense_ids)
        .bind(&user_ids)
        .bind(&amounts)
        .bind(&percentages)
        .fetch_all(executor)
        .await
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseWithPayer>, sqlx::Error> {
        sqlx::query_as::<Postgres, ExpenseWithPayer>(
            "SELECT e.id, e.group_id, e.created_by_id, e.payer_id, u.full_name AS payer_name,
                    e.amount, e.currency, e.description, e.split_type, e.expense_date,
                    e.deleted_at, e.created_at, e.updated_at
             FROM expenses e
             INNER JOIN users u ON e.payer_id = u.id
             WHERE e.id = $1 AND e.deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }

    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error> {
        // Single round trip: items + total via window count (evaluated before LIMIT).
        let rows = sqlx::query(
            "SELECT e.id, e.group_id, e.created_by_id, e.payer_id, u.full_name AS payer_name,
                    e.amount, e.currency, e.description, e.split_type, e.expense_date,
                    e.deleted_at, e.created_at, e.updated_at, COUNT(*) OVER() AS total_count
             FROM expenses e
             INNER JOIN users u ON e.payer_id = u.id
             WHERE e.group_id = $1 AND e.deleted_at IS NULL
             ORDER BY e.expense_date DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(group_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(executor)
        .await?;

        let total = rows.first().map(|row| row.try_get::<i64, _>("total_count")).transpose()?.unwrap_or(0);
        let items = rows.iter().map(ExpenseWithPayer::from_row).collect::<Result<Vec<_>, _>>()?;

        Ok((items, total))
    }

    async fn find_by_group_with_shares<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error> {
        let expenses = sqlx::query_as::<Postgres, ExpenseEntity>(
            "SELECT id, group_id, created_by_id, payer_id, amount, currency, description, split_type,
                    expense_date, deleted_at, created_at, updated_at
             FROM expenses
             WHERE group_id = $1 AND deleted_at IS NULL
             ORDER BY expense_date DESC",
        )
        .bind(group_id)
        .fetch_all(executor)
        .await?;

        if expenses.is_empty() {
            return Ok(vec![]);
        }

        let expense_ids: Vec<Uuid> = expenses.iter().map(|e| e.id).collect();
        let all_shares = sqlx::query_as::<Postgres, ExpenseShareEntity>(
            "SELECT id, expense_id, user_id, share_amount, share_percentage, created_at, updated_at
             FROM expense_shares
             WHERE expense_id = ANY($1)",
        )
        .bind(&expense_ids)
        .fetch_all(executor)
        .await?;

        let results = assemble_expenses_with_shares(expenses, all_shares);

        Ok(results)
    }

    async fn find_shares_by_expense<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        expense_id: Uuid,
    ) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error> {
        sqlx::query_as::<Postgres, ExpenseShareWithUser>(
            "SELECT es.id, es.expense_id, es.user_id, u.full_name AS user_name,
                    es.share_amount, es.share_percentage, es.created_at, es.updated_at
             FROM expense_shares es
             INNER JOIN users u ON es.user_id = u.id
             WHERE es.expense_id = $1",
        )
        .bind(expense_id)
        .fetch_all(executor)
        .await
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, ExpenseEntity>(
            "UPDATE expenses
             SET deleted_at = now()
             WHERE id = $1 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Currency, SplitType};
    use chrono::Utc;

    fn test_expense(id: Uuid) -> ExpenseEntity {
        ExpenseEntity {
            id,
            group_id: Uuid::now_v7(),
            created_by_id: Uuid::now_v7(),
            payer_id: Uuid::now_v7(),
            amount: 300,
            currency: Currency::VND,
            description: String::new(),
            split_type: SplitType::EQUAL,
            expense_date: Utc::now(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_share(expense_id: Uuid, amount: i64) -> ExpenseShareEntity {
        ExpenseShareEntity {
            id: Uuid::now_v7(),
            expense_id,
            user_id: Uuid::now_v7(),
            share_amount: amount,
            share_percentage: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn test_assemble_expenses_with_shares_groups_correctly() {
        let e1 = Uuid::now_v7();
        let e2 = Uuid::now_v7();
        let expenses = vec![test_expense(e1), test_expense(e2)];
        let shares = vec![test_share(e1, 100), test_share(e2, 300), test_share(e1, 200)];

        let grouped = assemble_expenses_with_shares(expenses, shares);

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].expense.id, e1);
        assert_eq!(grouped[0].shares.len(), 2);
        assert_eq!(grouped[1].expense.id, e2);
        assert_eq!(grouped[1].shares.len(), 1);
    }

    #[test]
    fn test_assemble_expenses_with_shares_empty_inputs() {
        let grouped = assemble_expenses_with_shares(vec![], vec![]);
        assert!(grouped.is_empty());

        let grouped = assemble_expenses_with_shares(vec![test_expense(Uuid::now_v7())], vec![]);
        assert_eq!(grouped.len(), 1);
        assert!(grouped[0].shares.is_empty());
    }
}
