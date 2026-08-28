use crate::domain::{Currency, GroupRole};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupEntity {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub default_currency: Currency,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGroupEntity {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub default_currency: Currency,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupMemberEntity {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub role: GroupRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewGroupMemberEntity {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub role: GroupRole,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupMemberWithUser {
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub full_name: String,
    pub role: GroupRole,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct GroupWithBalanceEntity {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub invite_code: Option<String>,
    pub default_currency: Currency,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_balance: Option<i64>,
}
