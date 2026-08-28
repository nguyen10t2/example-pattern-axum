use crate::domain::{Currency, GroupRole};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Deserialize)]
pub struct ParamId {
    pub id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct CreateGroupRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "defaultCurrency")]
    pub default_currency: Option<Currency>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct JoinGroupRequest {
    #[validate(length(min = 4, max = 10))]
    pub code: String,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct AddMemberRequest {
    #[serde(rename = "userId")]
    pub user_id: Uuid,
    pub role: Option<GroupRole>,
}
