use crate::domain::users::{entity::UserEntity, response::UserResponse};

pub struct UserMapper;

impl UserMapper {
    #[must_use]
    /// Map user entity sang response (giấu password hash).
    pub fn to_response(entity: UserEntity) -> UserResponse {
        UserResponse {
            id: entity.id,
            full_name: entity.full_name,
            email: entity.email,
            phone: entity.phone,
            preferred_currency: entity.preferred_currency,
            is_active: entity.is_active,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }

    /// Map danh sách user entity sang response.
    pub fn to_response_list(entities: Vec<UserEntity>) -> Vec<UserResponse> {
        entities.into_iter().map(Self::to_response).collect()
    }
}

impl From<UserEntity> for UserResponse {
    fn from(entity: UserEntity) -> Self {
        UserMapper::to_response(entity)
    }
}
