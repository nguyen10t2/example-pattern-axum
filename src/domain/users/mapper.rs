use crate::domain::users::{entity::UserEntity, response::UserResponse};

pub struct UserMapper;

impl UserMapper {
    #[must_use]
    pub fn to_response(entity: &UserEntity) -> UserResponse {
        UserResponse {
            id: entity.id,
            full_name: entity.full_name.clone(),
            email: entity.email.clone(),
            phone: entity.phone.clone(),
            preferred_currency: entity.preferred_currency,
            is_active: entity.is_active,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }

    pub fn to_response_list(entities: &[UserEntity]) -> Vec<UserResponse> {
        entities.iter().map(Self::to_response).collect()
    }
}

impl From<UserEntity> for UserResponse {
    fn from(entity: UserEntity) -> Self {
        UserMapper::to_response(&entity)
    }
}
