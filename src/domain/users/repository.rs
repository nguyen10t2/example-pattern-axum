use crate::domain::users::entity::{NewUserEntity, UpdateUserEntity, UserEntity};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    async fn find_by_email<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        email: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    async fn find_by_google_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        google_id: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user: &NewUserEntity,
    ) -> Result<UserEntity, sqlx::Error>;

    async fn update<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
        data: &UpdateUserEntity,
    ) -> Result<UserEntity, sqlx::Error>;

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error>;
}
