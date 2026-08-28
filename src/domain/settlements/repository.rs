use crate::domain::settlements::entity::{NewSettlementEntity, SettlementEntity, SettlementWithUsers};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait SettlementRepository: Send + Sync {
    async fn create(&self, data: &NewSettlementEntity) -> Result<SettlementEntity, sqlx::Error>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<SettlementEntity>, sqlx::Error>;
    async fn find_by_group(&self, group_id: Uuid) -> Result<Vec<SettlementEntity>, sqlx::Error>;
    async fn find_paginated_by_group(
        &self,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SettlementWithUsers>, i64), sqlx::Error>;
    async fn soft_delete(&self, id: Uuid) -> Result<Option<SettlementEntity>, sqlx::Error>;
}
