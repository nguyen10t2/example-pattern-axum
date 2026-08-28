pub mod debt_engine;
pub mod expenses;
pub mod groups;
pub mod settlements;
pub mod shared;
pub mod users;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "currency_code", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Currency {
    USD,
    #[default]
    VND,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "group_role", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupRole {
    OWNER,
    ADMIN,
    #[default]
    MEMBER,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "split_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SplitType {
    #[default]
    EQUAL,
    EXACT,
    PERCENTAGE,
}
