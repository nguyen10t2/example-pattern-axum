use crate::domain::Currency;
use chrono::{DateTime, Utc};
use std::fmt::Display;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Email(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Phone(pub String);

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserEntity {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub email_verified: bool,
    pub password_hash: Option<String>,
    pub google_id: Option<String>,
    pub avatar_url: Option<String>,
    pub phone: Option<String>,
    pub phone_verified: bool,
    pub preferred_currency: Currency,
    pub is_active: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUserEntity {
    pub id: Uuid,
    pub full_name: String,
    pub email: String,
    pub email_verified: bool,
    pub password_hash: Option<String>,
    pub google_id: Option<String>,
    pub avatar_url: Option<String>,
    pub phone: Option<String>,
    pub phone_verified: bool,
    pub preferred_currency: Currency,
    pub is_active: bool,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateUserEntity {
    pub full_name: Option<String>,
    pub password_hash: Option<String>,
    pub google_id: Option<String>,
    pub avatar_url: Option<String>,
    pub phone: Option<Option<String>>,
    pub preferred_currency: Option<Currency>,
    pub is_active: Option<bool>,
}

impl Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
