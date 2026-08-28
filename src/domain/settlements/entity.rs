use crate::domain::Currency;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettlementEntity {
    pub id: Uuid,
    pub group_id: Uuid,
    pub sender_id: Uuid,
    pub receiver_id: Uuid,
    pub amount: i64,
    pub currency: Currency,
    pub settled_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewSettlementEntity {
    pub id: Uuid,
    pub group_id: Uuid,
    pub sender_id: Uuid,
    pub receiver_id: Uuid,
    pub amount: i64,
    pub currency: Currency,
    pub settled_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SettlementWithUsers {
    pub id: Uuid,
    pub group_id: Uuid,
    pub sender_id: Uuid,
    pub sender_name: String,
    pub receiver_id: Uuid,
    pub receiver_name: String,
    pub amount: i64,
    pub currency: Currency,
    pub settled_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
