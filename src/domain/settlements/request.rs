use crate::domain::Currency;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Deserialize)]
pub struct ParamId {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateSettlementRequest {
    #[serde(rename = "groupId")]
    pub group_id: Uuid,
    #[serde(rename = "senderId")]
    pub sender_id: Uuid,
    #[serde(rename = "receiverId")]
    pub receiver_id: Uuid,
    #[validate(range(min = 1))]
    pub amount: i64,
    pub currency: Currency,
    #[serde(rename = "settledAt")]
    pub settled_at: Option<DateTime<Utc>>,
}
