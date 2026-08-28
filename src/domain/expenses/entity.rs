use crate::domain::{Currency, SplitType};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseEntity {
    pub id: Uuid,
    pub group_id: Uuid,
    pub created_by_id: Uuid,
    pub payer_id: Uuid,
    pub amount: i64,
    pub currency: Currency,
    pub description: String,
    pub split_type: SplitType,
    pub expense_date: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewExpenseEntity {
    pub id: Uuid,
    pub group_id: Uuid,
    pub created_by_id: Uuid,
    pub payer_id: Uuid,
    pub amount: i64,
    pub currency: Currency,
    pub description: String,
    pub split_type: SplitType,
    pub expense_date: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseShareEntity {
    pub id: Uuid,
    pub expense_id: Uuid,
    pub user_id: Uuid,
    pub share_amount: i64,
    pub share_percentage: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewExpenseShareEntity {
    pub id: Uuid,
    pub expense_id: Uuid,
    pub user_id: Uuid,
    pub share_amount: i64,
    pub share_percentage: Option<i32>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseWithPayer {
    pub id: Uuid,
    pub group_id: Uuid,
    pub created_by_id: Uuid,
    pub payer_id: Uuid,
    pub payer_name: String,
    pub amount: i64,
    pub currency: Currency,
    pub description: String,
    pub split_type: SplitType,
    pub expense_date: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpenseShareWithUser {
    pub id: Uuid,
    pub expense_id: Uuid,
    pub user_id: Uuid,
    pub user_name: String,
    pub share_amount: i64,
    pub share_percentage: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ExpenseWithSharesEntity {
    pub expense: ExpenseEntity,
    pub shares: Vec<ExpenseShareEntity>,
}
