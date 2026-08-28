use crate::domain::Currency;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementResponse {
    pub id: Uuid,
    pub group_id: Uuid,
    pub sender_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_name: Option<String>,
    pub receiver_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receiver_name: Option<String>,
    pub amount: i64,
    pub currency: Currency,
    pub settled_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
