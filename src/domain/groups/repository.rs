use crate::domain::groups::entity::{
    GroupEntity, GroupMemberEntity, GroupMemberWithUser, GroupWithBalanceEntity, NewGroupEntity, NewGroupMemberEntity,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupEntity,
    ) -> Result<GroupEntity, sqlx::Error>;

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;

    async fn add_member<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupMemberEntity,
    ) -> Result<GroupMemberEntity, sqlx::Error>;

    async fn find_members<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<GroupMemberWithUser>, sqlx::Error>;

    async fn find_by_invite_code<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        code: &str,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;

    async fn find_all_by_user<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user_id: Uuid,
    ) -> Result<Vec<GroupWithBalanceEntity>, sqlx::Error>;

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;
}
