use std::{collections::HashMap, sync::Arc};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    domain::{
        Currency, GroupRole,
        debt_engine::{DebtEngine, ExpenseForEngine, SettlementForEngine, ShareForEngine},
        expenses::repository::ExpenseRepository,
        groups::{
            entity::{NewGroupEntity, NewGroupMemberEntity},
            mapper::GroupMapper,
            repository::GroupRepository,
            request::{AddMemberRequest, CreateGroupRequest},
            response::{
                GroupMemberResponse, GroupResponse, GroupSummaryResponse, SettlementSuggestionWithNames,
                UserBalanceWithFullName,
            },
        },
        settlements::repository::SettlementRepository,
        users::repository::UserRepository,
    },
    errors::{AppError, BusinessError, map_unique_violation},
    utils::{
        cache::{CACHE_EXPIRATION, Cache, CacheStore, CacheStoreExt},
        random::generate_invite_code,
    },
};
use sqlx::PgPool;

pub struct GroupService<GR: GroupRepository, ER: ExpenseRepository, SR: SettlementRepository, UR: UserRepository> {
    group_repo: GR,
    expense_repo: ER,
    settlement_repo: SR,
    user_repo: UR,
    cache: Arc<Cache>,
    pool: PgPool,
}

impl<GR: GroupRepository, ER: ExpenseRepository, SR: SettlementRepository, UR: UserRepository>
    GroupService<GR, ER, SR, UR>
{
    pub fn new(
        group_repo: GR,
        expense_repo: ER,
        settlement_repo: SR,
        user_repo: UR,
        cache: Arc<Cache>,
        pool: PgPool,
    ) -> Self {
        Self { group_repo, expense_repo, settlement_repo, user_repo, cache, pool }
    }

    pub async fn create(&self, data: CreateGroupRequest, creator_id: Uuid) -> Result<GroupResponse, AppError> {
        let group_id = Uuid::now_v7();
        let invite_code = generate_invite_code();

        let new_group = NewGroupEntity {
            id: group_id,
            name: data.name,
            description: data.description,
            invite_code: Some(invite_code),
            default_currency: data.default_currency.unwrap_or(Currency::VND),
        };

        // Automatically add creator as ADMIN (atomic with group creation)
        let new_member = NewGroupMemberEntity { group_id, user_id: creator_id, role: GroupRole::ADMIN };

        let mut tx = self.pool.begin().await?;
        let group = self.group_repo.create(&mut *tx, &new_group).await?;
        self.group_repo
            .add_member(&mut *tx, &new_member)
            .await
            .map_err(|err| map_unique_violation(err, &[("group_members", "already joined")]))?;
        tx.commit().await?;

        info!(group_id = %group.id, name = %group.name, creator_id = %creator_id, "Group created");
        Ok(GroupMapper::to_response(&group))
    }

    pub async fn join_by_invite_code(&self, code: &str, user_id: Uuid) -> Result<GroupResponse, AppError> {
        let group = self.group_repo.find_by_invite_code(&self.pool, code).await?;
        let Some(group) = group else {
            warn!(code = %code, user_id = %user_id, "Invalid invite code attempt");
            return Err(AppError::Business(BusinessError::InvalidInviteCode));
        };

        let new_member = NewGroupMemberEntity { group_id: group.id, user_id, role: GroupRole::MEMBER };

        match self.group_repo.add_member(&self.pool, &new_member).await {
            Ok(_) => {
                info!(group_id = %group.id, user_id = %user_id, "User joined group via invite code");
                self.cache.delete(&format!("group_summary:{}", group.id)).await;
            }
            Err(err) => {
                // If unique violation (already joined), ignore conflict like in TS
                let is_unique_err = match &err {
                    sqlx::Error::Database(db_err) => db_err.code().as_deref() == Some("23505"),
                    _ => false,
                };
                if !is_unique_err {
                    return Err(AppError::from(err));
                }
            }
        }

        Ok(GroupMapper::to_response(&group))
    }

    pub async fn find_all_by_user(&self, user_id: Uuid) -> Result<Vec<GroupResponse>, AppError> {
        let groups = self.group_repo.find_all_by_user(&self.pool, user_id).await?;
        Ok(groups.iter().map(GroupMapper::to_response_with_balance).collect())
    }

    pub async fn find_by_id(&self, id: Uuid, current_user_id: Option<Uuid>) -> Result<GroupResponse, AppError> {
        let group_opt = if let Some(user_id) = current_user_id {
            let ((), group_opt) = tokio::try_join!(self.ensure_membership(id, user_id), async {
                self.group_repo.find_by_id(&self.pool, id).await.map_err(AppError::from)
            },)?;
            group_opt
        } else {
            self.group_repo.find_by_id(&self.pool, id).await?
        };
        let group = group_opt.ok_or(AppError::Business(BusinessError::GroupNotFound))?;

        Ok(GroupMapper::to_response(&group))
    }

    pub async fn get_group_summary(
        &self,
        group_id: Uuid,
        current_user_id: Uuid,
    ) -> Result<GroupSummaryResponse, AppError> {
        let cache_key = format!("group_summary:{}", group_id);

        // Fetch members once; reused for both membership check and summary building
        let members = self.group_repo.find_members(&self.pool, group_id).await?;
        if !members.iter().any(|m| m.user_id == current_user_id) {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        }

        if let Some(cached) = self.cache.get::<GroupSummaryResponse>(&cache_key).await {
            return Ok(cached);
        }

        let (group_opt, expenses_with_shares, settlements) = tokio::try_join!(
            self.group_repo.find_by_id(&self.pool, group_id),
            self.expense_repo.find_by_group_with_shares(&self.pool, group_id),
            self.settlement_repo.find_by_group(&self.pool, group_id),
        )?;

        let group = group_opt.ok_or(AppError::Business(BusinessError::GroupNotFound))?;

        let user_ids: Vec<Uuid> = members.iter().map(|m| m.user_id).collect();
        let user_name_map: HashMap<Uuid, String> = members.iter().map(|m| (m.user_id, m.full_name.clone())).collect();

        // Map for DebtEngine
        let engine_expenses: Vec<ExpenseForEngine> = expenses_with_shares
            .iter()
            .map(|e| ExpenseForEngine {
                payer_id: e.expense.payer_id,
                amount: e.expense.amount,
                shares: e
                    .shares
                    .iter()
                    .map(|s| ShareForEngine { user_id: s.user_id, amount: s.share_amount })
                    .collect(),
            })
            .collect();

        let engine_settlements: Vec<SettlementForEngine> = settlements
            .iter()
            .map(|s| SettlementForEngine { sender_id: s.sender_id, receiver_id: s.receiver_id, amount: s.amount })
            .collect();
        let balances = DebtEngine::calculate_net_balances(&user_ids, &engine_expenses, &engine_settlements).await?;
        let suggestions = DebtEngine::simplify_debts(&balances).await?;

        let summary = GroupSummaryResponse {
            id: group.id,
            name: group.name,
            description: group.description,
            invite_code: group.invite_code,
            default_currency: group.default_currency,
            created_at: group.created_at,
            updated_at: group.updated_at,
            user_balance: None,
            balances: balances
                .into_iter()
                .map(|b| UserBalanceWithFullName {
                    full_name: user_name_map.get(&b.user_id).cloned().unwrap_or_else(|| "Unknown".to_string()),
                    user_id: b.user_id,
                    net_amount: b.net_amount,
                })
                .collect(),
            suggestions: suggestions
                .into_iter()
                .map(|s| SettlementSuggestionWithNames {
                    from_user_name: user_name_map
                        .get(&s.from_user_id)
                        .cloned()
                        .unwrap_or_else(|| "Unknown".to_string()),
                    to_user_name: user_name_map.get(&s.to_user_id).cloned().unwrap_or_else(|| "Unknown".to_string()),
                    from_user_id: s.from_user_id,
                    to_user_id: s.to_user_id,
                    amount: s.amount,
                })
                .collect(),
        };

        self.cache.set(&cache_key, &summary, CACHE_EXPIRATION).await;

        Ok(summary)
    }

    pub async fn add_member(
        &self,
        group_id: Uuid,
        data: AddMemberRequest,
        current_user_id: Uuid,
    ) -> Result<GroupMemberResponse, AppError> {
        self.find_by_id(group_id, None).await?; // Ensure group exists
        self.ensure_admin(group_id, current_user_id).await?;

        let member = self
            .group_repo
            .add_member(
                &self.pool,
                &NewGroupMemberEntity { group_id, user_id: data.user_id, role: data.role.unwrap_or(GroupRole::MEMBER) },
            )
            .await
            .map_err(|err| map_unique_violation(err, &[("group_members", "already joined")]))?;

        let user = self.user_repo.find_by_id(&self.pool, data.user_id).await?;
        let full_name = user.map(|u| u.full_name).unwrap_or_else(|| "Unknown Member".to_string());

        self.cache.delete(&format!("group_summary:{}", group_id)).await;

        Ok(GroupMemberResponse {
            group_id: member.group_id,
            user_id: member.user_id,
            full_name,
            role: member.role,
            joined_at: member.joined_at,
        })
    }

    pub async fn get_members(
        &self,
        group_id: Uuid,
        current_user_id: Uuid,
    ) -> Result<Vec<GroupMemberResponse>, AppError> {
        self.ensure_membership(group_id, current_user_id).await?;
        let members = self.group_repo.find_members(&self.pool, group_id).await?;
        Ok(members.iter().map(GroupMapper::to_member_response).collect())
    }

    pub async fn find_members(
        &self,
        group_id: Uuid,
    ) -> Result<Vec<crate::domain::groups::entity::GroupMemberWithUser>, AppError> {
        let members = self.group_repo.find_members(&self.pool, group_id).await?;
        Ok(members)
    }

    pub async fn delete_group(&self, id: Uuid, current_user_id: Uuid) -> Result<(), AppError> {
        self.ensure_admin(id, current_user_id).await?;
        let result = self.group_repo.soft_delete(&self.pool, id).await?;
        if result.is_none() {
            return Err(AppError::Business(BusinessError::GroupNotFound));
        }

        self.cache.delete(&format!("group_summary:{}", id)).await;
        Ok(())
    }

    pub async fn ensure_membership(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let members = self.group_repo.find_members(&self.pool, group_id).await?;
        if !members.iter().any(|m| m.user_id == user_id) {
            return Err(AppError::Business(BusinessError::NotGroupMember));
        }
        Ok(())
    }

    pub async fn ensure_admin(&self, group_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let members = self.group_repo.find_members(&self.pool, group_id).await?;
        let member = members.iter().find(|m| m.user_id == user_id);
        match member {
            Some(m) if m.role == GroupRole::ADMIN => Ok(()),
            _ => Err(AppError::Business(BusinessError::AdminRequired)),
        }
    }
}
