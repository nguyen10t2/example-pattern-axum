use crate::domain::{Currency, GroupRole};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub default_currency: Currency,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_balance: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMemberResponse {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub role: GroupRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserBalanceWithFullName {
    pub user_id: Uuid,
    pub net_amount: i64,
    pub full_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementSuggestionWithNames {
    pub from_user_id: Uuid,
    pub to_user_id: Uuid,
    pub amount: i64,
    pub from_user_name: String,
    pub to_user_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupSummaryResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub default_currency: Currency,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_balance: Option<i64>,
    pub balances: Vec<UserBalanceWithFullName>,
    pub suggestions: Vec<SettlementSuggestionWithNames>,
}
