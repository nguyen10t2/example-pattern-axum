use crate::domain::users::{
    entity::{NewUserEntity, UpdateUserEntity, UserEntity},
    repository::UserRepository,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct PostgresUserRepository;

impl PostgresUserRepository {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>("SELECT * FROM users WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    async fn find_by_email<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        email: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>(
            "SELECT * FROM users WHERE LOWER(email) = LOWER($1) AND deleted_at IS NULL",
        )
        .bind(email)
        .fetch_optional(executor)
        .await
    }

    async fn find_by_google_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        google_id: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>("SELECT * FROM users WHERE google_id = $1 AND deleted_at IS NULL")
            .bind(google_id)
            .fetch_optional(executor)
            .await
    }

    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user: &NewUserEntity,
    ) -> Result<UserEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>(
            "INSERT INTO users (
                id, full_name, email, email_verified, password_hash,
                google_id, avatar_url, phone, phone_verified, preferred_currency, is_active
            )
            VALUES ($1, $2, LOWER($3), $4, $5, $6, $7, $8, $9, $10, $11)
            RETURNING *",
        )
        .bind(user.id)
        .bind(&user.full_name)
        .bind(&user.email)
        .bind(user.email_verified)
        .bind(&user.password_hash)
        .bind(&user.google_id)
        .bind(&user.avatar_url)
        .bind(&user.phone)
        .bind(user.phone_verified)
        .bind(user.preferred_currency)
        .bind(user.is_active)
        .fetch_one(executor)
        .await
    }

    async fn update<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
        data: &UpdateUserEntity,
    ) -> Result<UserEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>(
            "UPDATE users
             SET
                full_name = COALESCE($1, full_name),
                password_hash = COALESCE($2, password_hash),
                google_id = COALESCE($3, google_id),
                avatar_url = COALESCE($4, avatar_url),
                phone = CASE WHEN $5 THEN $6 ELSE phone END,
                preferred_currency = COALESCE($7, preferred_currency),
                is_active = COALESCE($8, is_active)
              WHERE id = $9 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(&data.full_name)
        .bind(&data.password_hash)
        .bind(&data.google_id)
        .bind(&data.avatar_url)
        .bind(data.phone.is_some())
        .bind(data.phone.as_ref().cloned())
        .bind(data.preferred_currency)
        .bind(data.is_active)
        .bind(id)
        .fetch_one(executor)
        .await
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, UserEntity>(
            "UPDATE users
             SET deleted_at = now()
             WHERE id = $1 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }
}
