use crate::errors::error_codes;
use axum::http::StatusCode;

#[derive(Debug, Clone, thiserror::Error)]
pub enum BusinessError {
    #[error("{}", error_codes::UNAUTHORIZED)]
    Unauthorized,

    #[error("{}", error_codes::FORBIDDEN)]
    Forbidden,

    #[error("{}", error_codes::INVALID_CREDENTIALS)]
    InvalidCredentials,

    #[error("{}", error_codes::USER_ALREADY_EXISTS)]
    UserAlreadyExists,

    #[error("{}", error_codes::USER_NOT_FOUND)]
    UserNotFound(String),

    #[error("{}", error_codes::INVALID_OTP)]
    InvalidOtp,

    #[error("{}", error_codes::INVALID_SESSION)]
    InvalidSession,

    #[error("{}", error_codes::EMAIL_NOT_VERIFIED)]
    EmailNotVerified,

    #[error("{}", error_codes::INVALID_OLD_PASSWORD)]
    InvalidOldPassword,

    #[error("{}", error_codes::GROUP_NOT_FOUND)]
    GroupNotFound,

    #[error("{}", error_codes::NOT_GROUP_MEMBER)]
    NotGroupMember,

    #[error("{}", error_codes::ADMIN_REQUIRED)]
    AdminRequired,

    #[error("{}", error_codes::INVALID_INVITE_CODE)]
    InvalidInviteCode,

    #[error("{}", error_codes::PAYER_NOT_IN_GROUP)]
    PayerNotInGroup,

    #[error("{}", error_codes::USER_NOT_IN_GROUP)]
    UserNotInGroup(String),

    #[error("{}", error_codes::USER_ALREADY_IN_GROUP)]
    UserAlreadyInGroup,

    #[error("{}", error_codes::EXPENSE_NOT_FOUND)]
    ExpenseNotFound,

    #[error("{}", error_codes::SETTLEMENT_NOT_FOUND)]
    SettlementNotFound,

    #[error("{}", error_codes::DELETE_PERMISSION_DENIED)]
    DeletePermissionDenied,

    #[error("{}", error_codes::BAD_REQUEST)]
    BadRequest(String),

    #[error("{}", error_codes::NOT_FOUND)]
    NotFound(String),

    #[error("{}", error_codes::CONFLICT)]
    Conflict(String),

    #[error("{}", error_codes::VALIDATION_ERROR)]
    ValidationError(String),

    #[error("{}", error_codes::TOO_MANY_REQUESTS)]
    TooManyRequests,

    #[error("{field} already exists: {value}")]
    DuplicateField { field: String, value: String },

    #[error("Invalid user ID: {0}")]
    InvalidUserId(String),
}

impl BusinessError {
    #[must_use]
    pub const fn status_code(&self) -> StatusCode {
        match self {
            Self::Unauthorized | Self::InvalidCredentials | Self::InvalidSession => StatusCode::UNAUTHORIZED,
            Self::Forbidden
            | Self::EmailNotVerified
            | Self::NotGroupMember
            | Self::AdminRequired
            | Self::DeletePermissionDenied => StatusCode::FORBIDDEN,
            Self::UserAlreadyExists
            | Self::InvalidOtp
            | Self::InvalidOldPassword
            | Self::PayerNotInGroup
            | Self::UserNotInGroup(_)
            | Self::BadRequest(_)
            | Self::InvalidUserId(_) => StatusCode::BAD_REQUEST,
            Self::UserNotFound(_)
            | Self::GroupNotFound
            | Self::InvalidInviteCode
            | Self::ExpenseNotFound
            | Self::SettlementNotFound
            | Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::UserAlreadyInGroup | Self::Conflict(_) | Self::DuplicateField { .. } => StatusCode::CONFLICT,
            Self::ValidationError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
        }
    }

    #[must_use]
    pub const fn error_code(&self) -> &str {
        match self {
            Self::Unauthorized => error_codes::UNAUTHORIZED,
            Self::Forbidden => error_codes::FORBIDDEN,
            Self::InvalidCredentials => error_codes::INVALID_CREDENTIALS,
            Self::UserAlreadyExists => error_codes::USER_ALREADY_EXISTS,
            Self::UserNotFound(_) => error_codes::USER_NOT_FOUND,
            Self::InvalidOtp => error_codes::INVALID_OTP,
            Self::InvalidSession => error_codes::INVALID_SESSION,
            Self::EmailNotVerified => error_codes::EMAIL_NOT_VERIFIED,
            Self::InvalidOldPassword => error_codes::INVALID_OLD_PASSWORD,
            Self::GroupNotFound => error_codes::GROUP_NOT_FOUND,
            Self::NotGroupMember => error_codes::NOT_GROUP_MEMBER,
            Self::AdminRequired => error_codes::ADMIN_REQUIRED,
            Self::InvalidInviteCode => error_codes::INVALID_INVITE_CODE,
            Self::PayerNotInGroup => error_codes::PAYER_NOT_IN_GROUP,
            Self::UserNotInGroup(_) => error_codes::USER_NOT_IN_GROUP,
            Self::UserAlreadyInGroup => error_codes::USER_ALREADY_IN_GROUP,
            Self::ExpenseNotFound => error_codes::EXPENSE_NOT_FOUND,
            Self::SettlementNotFound => error_codes::SETTLEMENT_NOT_FOUND,
            Self::DeletePermissionDenied => error_codes::DELETE_PERMISSION_DENIED,
            Self::BadRequest(_) | Self::InvalidUserId(_) => error_codes::BAD_REQUEST,
            Self::NotFound(_) => error_codes::NOT_FOUND,
            Self::Conflict(_) | Self::DuplicateField { .. } => error_codes::CONFLICT,
            Self::ValidationError(_) => error_codes::VALIDATION_ERROR,
            Self::TooManyRequests => error_codes::TOO_MANY_REQUESTS,
        }
    }
}
