use crate::domain::settlements::{
    entity::{NewSettlementEntity, SettlementEntity, SettlementWithUsers},
    repository::SettlementRepository,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct PostgresSettlementRepository;

impl PostgresSettlementRepository {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SettlementRepository for PostgresSettlementRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewSettlementEntity,
    ) -> Result<SettlementEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, SettlementEntity>(
            "INSERT INTO settlements (
                id, group_id, sender_id, receiver_id, amount, currency, settled_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING *",
        )
        .bind(data.id)
        .bind(data.group_id)
        .bind(data.sender_id)
        .bind(data.receiver_id)
        .bind(data.amount)
        .bind(data.currency)
        .bind(data.settled_at)
        .fetch_one(executor)
        .await
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, SettlementEntity>("SELECT * FROM settlements WHERE id = $1 AND deleted_at IS NULL")
            .bind(id)
            .fetch_optional(executor)
            .await
    }

    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<SettlementEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, SettlementEntity>(
            "SELECT * FROM settlements WHERE group_id = $1 AND deleted_at IS NULL",
        )
        .bind(group_id)
        .fetch_all(executor)
        .await
    }

    async fn find_paginated_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SettlementWithUsers>, i64), sqlx::Error> {
        let count_row: (i64,) =
            sqlx::query_as("SELECT COUNT(*)::bigint FROM settlements WHERE group_id = $1 AND deleted_at IS NULL")
                .bind(group_id)
                .fetch_one(executor)
                .await?;

        let items = sqlx::query_as::<Postgres, SettlementWithUsers>(
            "SELECT s.id, s.group_id, s.sender_id, u_sender.full_name AS sender_name,
                    s.receiver_id, u_receiver.full_name AS receiver_name,
                    s.amount, s.currency, s.settled_at, s.deleted_at, s.created_at, s.updated_at
             FROM settlements s
             INNER JOIN users u_sender ON s.sender_id = u_sender.id
             INNER JOIN users u_receiver ON s.receiver_id = u_receiver.id
             WHERE s.group_id = $1 AND s.deleted_at IS NULL
             ORDER BY s.settled_at DESC
             LIMIT $2 OFFSET $3",
        )
        .bind(group_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(executor)
        .await?;

        Ok((items, count_row.0))
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, SettlementEntity>(
            "UPDATE settlements
             SET deleted_at = now()
             WHERE id = $1 AND deleted_at IS NULL
             RETURNING *",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }
}
