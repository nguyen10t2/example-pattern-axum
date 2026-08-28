use crate::domain::{Currency, SplitType};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Deserialize)]
pub struct ParamId {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ShareRequest {
    #[serde(rename = "userId")]
    pub user_id: Uuid,
    #[serde(rename = "shareAmount")]
    #[validate(range(min = 0))]
    pub share_amount: i64,
    #[serde(rename = "sharePercentage")]
    pub share_percentage: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateExpenseRequest {
    #[serde(rename = "groupId")]
    pub group_id: Uuid,
    #[serde(rename = "payerId")]
    pub payer_id: Uuid,
    #[validate(range(min = 1))]
    pub amount: i64,
    pub currency: Currency,
    #[validate(length(min = 1, max = 255))]
    pub description: String,
    #[serde(rename = "expenseDate")]
    pub expense_date: Option<DateTime<Utc>>,
    #[serde(rename = "splitType")]
    pub split_type: Option<SplitType>,
    #[validate(nested)]
    pub shares: Vec<ShareRequest>,
}
