use crate::domain::groups::entity::{
    GroupEntity, GroupMemberEntity, GroupMemberWithUser, GroupWithBalanceEntity, NewGroupEntity, NewGroupMemberEntity,
};
use async_trait::async_trait;
use uuid::Uuid;

#[async_trait]
pub trait GroupRepository: Send + Sync {
    async fn create(&self, data: &NewGroupEntity) -> Result<GroupEntity, sqlx::Error>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<GroupEntity>, sqlx::Error>;
    async fn add_member(&self, data: &NewGroupMemberEntity) -> Result<GroupMemberEntity, sqlx::Error>;
    async fn find_members(&self, group_id: Uuid) -> Result<Vec<GroupMemberWithUser>, sqlx::Error>;
    async fn find_by_invite_code(&self, code: &str) -> Result<Option<GroupEntity>, sqlx::Error>;
    async fn find_all_by_user(&self, user_id: Uuid) -> Result<Vec<GroupWithBalanceEntity>, sqlx::Error>;
    async fn soft_delete(&self, id: Uuid) -> Result<Option<GroupEntity>, sqlx::Error>;
}
