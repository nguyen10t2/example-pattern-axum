use crate::domain::groups::entity::{
    GroupEntity, GroupMemberEntity, GroupMemberWithUser, GroupWithBalanceEntity, NewGroupEntity, NewGroupMemberEntity,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[async_trait]
/// Truy xuất nhóm và thành viên (nhận pool hoặc transaction qua `executor`).
pub trait GroupRepository: Send + Sync {
    /// Chèn nhóm mới.
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupEntity,
    ) -> Result<GroupEntity, sqlx::Error>;

    /// Tìm nhóm theo id.
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;

    /// Thêm thành viên vào nhóm.
    async fn add_member<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupMemberEntity,
    ) -> Result<GroupMemberEntity, sqlx::Error>;

    /// Liệt kê thành viên kèm user của nhóm.
    async fn find_members<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<GroupMemberWithUser>, sqlx::Error>;

    /// Tìm nhóm theo invite code.
    async fn find_by_invite_code<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        code: &str,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;

    /// Liệt kê các nhóm mà user tham gia.
    async fn find_all_by_user<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user_id: Uuid,
    ) -> Result<Vec<GroupWithBalanceEntity>, sqlx::Error>;

    /// Xóa mềm nhóm, trả `None` nếu id không tồn tại.
    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error>;
}
