use crate::domain::users::entity::{NewUserEntity, UpdateUserEntity, UserEntity};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: Uuid) -> Result<Option<UserEntity>, sqlx::Error>;
    async fn find_by_email(&self, email: &str) -> Result<Option<UserEntity>, sqlx::Error>;
    async fn find_by_google_id(&self, google_id: &str) -> Result<Option<UserEntity>, sqlx::Error>;
    async fn create(&self, user: &NewUserEntity) -> Result<UserEntity, sqlx::Error>;
    async fn update(&self, id: Uuid, data: &UpdateUserEntity) -> Result<UserEntity, sqlx::Error>;
    async fn soft_delete(&self, id: Uuid) -> Result<Option<UserEntity>, sqlx::Error>;
}
