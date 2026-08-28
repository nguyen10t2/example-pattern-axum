use crate::domain::{Currency, SplitType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseShareResponse {
    pub id: Uuid,
    pub expense_id: Uuid,
    pub user_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_name: Option<String>,
    pub share_amount: i64,
    pub share_percentage: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpenseResponse {
    pub id: Uuid,
    pub group_id: Uuid,
    pub created_by_id: Uuid,
    pub payer_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer_name: Option<String>,
    pub amount: i64,
    pub currency: Currency,
    pub description: String,
    pub split_type: SplitType,
    pub expense_date: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares: Option<Vec<ExpenseShareResponse>>,
}
