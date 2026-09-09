use crate::domain::settlements::entity::{NewSettlementEntity, SettlementEntity, SettlementWithUsers};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
/// Truy xuất settlement (nhận pool hoặc transaction qua `executor`).
pub trait SettlementRepository: Send + Sync {
    /// Chèn settlement mới.
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewSettlementEntity,
    ) -> Result<SettlementEntity, sqlx::Error>;

    /// Tìm settlement theo id.
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error>;

    /// Liệt kê settlements của nhóm (không phân trang).
    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<SettlementEntity>, sqlx::Error>;

    /// Liệt kê settlements của nhóm có phân trang, kèm tổng số dòng.
    async fn find_paginated_by_group<'e, E: Executor<'e, Database = Postgres> + Copy + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<SettlementWithUsers>, i64), sqlx::Error>;

    /// Xóa mềm settlement, trả `None` nếu id không tồn tại.
    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error>;
}
