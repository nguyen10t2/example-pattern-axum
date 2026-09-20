use crate::domain::groups::{
    entity::{
        GroupEntity, GroupMemberEntity, GroupMemberWithUser, GroupWithBalanceEntity, NewGroupEntity,
        NewGroupMemberEntity,
    },
    repository::GroupRepository,
};
use async_trait::async_trait;
use sqlx::{Executor, Postgres};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct PostgresGroupRepository;

impl PostgresGroupRepository {
    /// Tạo repository group (stateless).
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

#[async_trait]
impl GroupRepository for PostgresGroupRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupEntity,
    ) -> Result<GroupEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupEntity>(
            "INSERT INTO groups (id, name, description, invite_code, default_currency)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING *",
        )
        .bind(data.id)
        .bind(&data.name)
        .bind(&data.description)
        .bind(&data.invite_code)
        .bind(data.default_currency)
        .fetch_one(executor)
        .await
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupEntity>(
            "SELECT id, name, description, invite_code, default_currency, deleted_at, created_at, updated_at
             FROM groups WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(id)
        .fetch_optional(executor)
        .await
    }

    async fn add_member<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        data: &NewGroupMemberEntity,
    ) -> Result<GroupMemberEntity, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupMemberEntity>(
            "INSERT INTO group_members (group_id, user_id, role)
             VALUES ($1, $2, $3)
             RETURNING *",
        )
        .bind(data.group_id)
        .bind(data.user_id)
        .bind(data.role)
        .fetch_one(executor)
        .await
    }

    async fn find_members<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        group_id: Uuid,
    ) -> Result<Vec<GroupMemberWithUser>, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupMemberWithUser>(
            "SELECT gm.group_id, gm.user_id, u.full_name, gm.role, gm.joined_at, gm.left_at
             FROM group_members gm
             INNER JOIN users u ON gm.user_id = u.id
             WHERE gm.group_id = $1 AND u.deleted_at IS NULL",
        )
        .bind(group_id)
        .fetch_all(executor)
        .await
    }

    async fn find_by_invite_code<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        code: &str,
    ) -> Result<Option<GroupEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupEntity>(
            "SELECT id, name, description, invite_code, default_currency, deleted_at, created_at, updated_at
             FROM groups WHERE invite_code = $1 AND deleted_at IS NULL",
        )
        .bind(code)
        .fetch_optional(executor)
        .await
    }

    async fn find_all_by_user<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        user_id: Uuid,
    ) -> Result<Vec<GroupWithBalanceEntity>, sqlx::Error> {
        sqlx::query_as::<Postgres, GroupWithBalanceEntity>(
            r"
            WITH user_paid AS (
                SELECT group_id, SUM(amount)::bigint AS total
                FROM expenses
                WHERE payer_id = $1 AND deleted_at IS NULL
                GROUP BY group_id
            ),
            user_owes AS (
                SELECT e.group_id, SUM(es.share_amount)::bigint AS total
                FROM expense_shares es
                INNER JOIN expenses e ON es.expense_id = e.id
                WHERE es.user_id = $1 AND e.deleted_at IS NULL
                GROUP BY e.group_id
            ),
            user_sent AS (
                SELECT group_id, SUM(amount)::bigint AS total
                FROM settlements
                WHERE sender_id = $1 AND deleted_at IS NULL
                GROUP BY group_id
            ),
            user_received AS (
                SELECT group_id, SUM(amount)::bigint AS total
                FROM settlements
                WHERE receiver_id = $1 AND deleted_at IS NULL
                GROUP BY group_id
            )
            SELECT
                g.id,
                g.name,
                g.description,
                g.invite_code,
                g.default_currency,
                g.deleted_at,
                g.created_at,
                g.updated_at,
                (
                    COALESCE(up.total, 0) -
                    COALESCE(uo.total, 0) +
                    COALESCE(us.total, 0) -
                    COALESCE(ur.total, 0)
                )::bigint AS user_balance
            FROM groups g
            INNER JOIN group_members gm ON g.id = gm.group_id
            LEFT JOIN user_paid up ON g.id = up.group_id
            LEFT JOIN user_owes uo ON g.id = uo.group_id
            LEFT JOIN user_sent us ON g.id = us.group_id
            LEFT JOIN user_received ur ON g.id = ur.group_id
            WHERE gm.user_id = $1 AND gm.left_at IS NULL AND g.deleted_at IS NULL
            ",
        )
        .bind(user_id)
        .fetch_all(executor)
        .await
    }

    async fn leave_group<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        user_id: Uuid,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE group_members SET left_at = now() WHERE group_id = $1 AND user_id = $2")
            .bind(group_id)
            .bind(user_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    async fn set_member_role<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        executor: E,
        group_id: Uuid,
        user_id: Uuid,
        role: crate::domain::GroupRole,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE group_members SET role = $1 WHERE group_id = $2 AND user_id = $3")
            .bind(role)
            .bind(group_id)
            .bind(user_id)
            .execute(executor)
            .await?;
        Ok(())
    }

    async fn hard_delete(&self, pool: &sqlx::PgPool, id: Uuid) -> Result<Option<GroupEntity>, sqlx::Error> {
        let mut tx = pool.begin().await?;

        sqlx::query("DELETE FROM expense_shares WHERE expense_id IN (SELECT id FROM expenses WHERE group_id = $1)")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        sqlx::query("DELETE FROM expenses WHERE group_id = $1").bind(id).execute(&mut *tx).await?;

        sqlx::query("DELETE FROM settlements WHERE group_id = $1").bind(id).execute(&mut *tx).await?;

        sqlx::query("DELETE FROM group_members WHERE group_id = $1").bind(id).execute(&mut *tx).await?;

        let group = sqlx::query_as::<Postgres, GroupEntity>("DELETE FROM groups WHERE id = $1 RETURNING *")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(group)
    }
}
