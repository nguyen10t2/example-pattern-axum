use crate::domain::users::entity::{NewUserEntity, UpdateUserEntity, UserEntity};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
/// Truy xuất user (nhận pool hoặc transaction qua `executor`).
pub trait UserRepository: Send + Sync {
    /// Tìm user theo id (kể cả đã xóa mềm).
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    /// Tìm user theo email.
    async fn find_by_email<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        email: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    /// Tìm user theo Google `sub`.
    async fn find_by_google_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        google_id: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error>;

    /// Chèn user mới.
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user: &NewUserEntity,
    ) -> Result<UserEntity, sqlx::Error>;

    /// Cập nhật từng phần user theo id.
    async fn update<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
        data: &UpdateUserEntity,
    ) -> Result<UserEntity, sqlx::Error>;

    /// Xóa mềm user (`deleted_at`), trả `None` nếu id không tồn tại.
    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error>;
}
