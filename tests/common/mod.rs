#![allow(dead_code)]

use argon2::Argon2;
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Executor, Postgres};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use dsa::{
    config::constants::JWT_REFRESH_TOKEN_EXPIRATION_SECS,
    domain::{
        expenses::entity::{
            ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer, ExpenseWithSharesEntity,
            NewExpenseEntity, NewExpenseShareEntity,
        },
        expenses::repository::ExpenseRepository,
        groups::{
            entity::{
                GroupEntity, GroupMemberEntity, GroupMemberWithUser, GroupWithBalanceEntity, NewGroupEntity,
                NewGroupMemberEntity,
            },
            repository::GroupRepository,
        },
        settlements::{
            entity::{NewSettlementEntity, SettlementEntity, SettlementWithUsers},
            repository::SettlementRepository,
        },
        users::{
            entity::{NewUserEntity, UpdateUserEntity, UserEntity},
            repository::UserRepository,
        },
    },
    utils::{
        cache::{Cache, MemoryCache},
        jwt::JwtConfig,
    },
};

// ---------------------------------------------------------------------------
// Mock User Repository
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct MockUserRepository {
    pub users: Arc<Mutex<Vec<UserEntity>>>,
}

#[async_trait]
impl UserRepository for MockUserRepository {
    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        let users = self.users.lock().await;
        Ok(users.iter().find(|u| u.id == id && u.deleted_at.is_none()).cloned())
    }

    async fn find_by_email<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        email: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        let users = self.users.lock().await;
        Ok(users.iter().find(|u| u.email.to_lowercase() == email.to_lowercase() && u.deleted_at.is_none()).cloned())
    }

    async fn find_by_google_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        google_id: &str,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        let users = self.users.lock().await;
        Ok(users.iter().find(|u| u.google_id.as_deref() == Some(google_id) && u.deleted_at.is_none()).cloned())
    }

    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        user: &NewUserEntity,
    ) -> Result<UserEntity, sqlx::Error> {
        let entity = UserEntity {
            id: user.id,
            full_name: user.full_name.clone(),
            email: user.email.clone(),
            email_verified: user.email_verified,
            password_hash: user.password_hash.clone(),
            google_id: user.google_id.clone(),
            avatar_url: user.avatar_url.clone(),
            phone: user.phone.clone(),
            phone_verified: user.phone_verified,
            preferred_currency: user.preferred_currency,
            is_active: user.is_active,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        {
            let mut users = self.users.lock().await;
            users.push(entity.clone());
        }
        Ok(entity)
    }

    async fn update<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
        data: &UpdateUserEntity,
    ) -> Result<UserEntity, sqlx::Error> {
        let updated = {
            let mut users = self.users.lock().await;
            let user = users.iter_mut().find(|u| u.id == id).unwrap();
            if let Some(ref name) = data.full_name {
                user.full_name.clone_from(name);
            }
            if let Some(ref hash) = data.password_hash {
                user.password_hash = Some(hash.clone());
            }
            if let Some(ref gid) = data.google_id {
                user.google_id = Some(gid.clone());
            }
            if let Some(ref avatar) = data.avatar_url {
                user.avatar_url = Some(avatar.clone());
            }
            if let Some(ref phone) = data.phone {
                user.phone.clone_from(phone);
            }
            if let Some(currency) = data.preferred_currency {
                user.preferred_currency = currency;
            }
            if let Some(active) = data.is_active {
                user.is_active = active;
            }
            user.updated_at = Utc::now();
            let updated = user.clone();
            drop(users);
            updated
        };
        Ok(updated)
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<UserEntity>, sqlx::Error> {
        let mut users = self.users.lock().await;
        if let Some(user) = users.iter_mut().find(|u| u.id == id && u.deleted_at.is_none()) {
            user.deleted_at = Some(Utc::now());
            Ok(Some(user.clone()))
        } else {
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Group Repository
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct MockGroupRepository {
    pub groups: Arc<Mutex<Vec<GroupEntity>>>,
    pub members: Arc<Mutex<Vec<GroupMemberWithUser>>>,
}

#[async_trait]
impl GroupRepository for MockGroupRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        data: &NewGroupEntity,
    ) -> Result<GroupEntity, sqlx::Error> {
        let entity = GroupEntity {
            id: data.id,
            name: data.name.clone(),
            description: data.description.clone(),
            invite_code: data.invite_code.clone(),
            default_currency: data.default_currency,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.groups.lock().await.push(entity.clone());
        Ok(entity)
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error> {
        Ok(self.groups.lock().await.iter().find(|g| g.id == id && g.deleted_at.is_none()).cloned())
    }

    async fn add_member<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        data: &NewGroupMemberEntity,
    ) -> Result<GroupMemberEntity, sqlx::Error> {
        self.members.lock().await.push(GroupMemberWithUser {
            group_id: data.group_id,
            user_id: data.user_id,
            full_name: "Mock Member".to_string(),
            role: data.role,
            joined_at: Utc::now(),
        });
        Ok(GroupMemberEntity { group_id: data.group_id, user_id: data.user_id, role: data.role, joined_at: Utc::now() })
    }

    async fn find_members<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        group_id: Uuid,
    ) -> Result<Vec<GroupMemberWithUser>, sqlx::Error> {
        Ok(self.members.lock().await.iter().filter(|m| m.group_id == group_id).cloned().collect())
    }

    async fn find_by_invite_code<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        code: &str,
    ) -> Result<Option<GroupEntity>, sqlx::Error> {
        Ok(self
            .groups
            .lock()
            .await
            .iter()
            .find(|g| g.invite_code.as_deref() == Some(code) && g.deleted_at.is_none())
            .cloned())
    }

    async fn find_all_by_user<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        user_id: Uuid,
    ) -> Result<Vec<GroupWithBalanceEntity>, sqlx::Error> {
        let user_group_ids: Vec<Uuid> = {
            let members = self.members.lock().await;
            members.iter().filter(|m| m.user_id == user_id).map(|m| m.group_id).collect()
        };

        let items: Vec<GroupWithBalanceEntity> = {
            let groups = self.groups.lock().await;
            groups
                .iter()
                .filter(|g| user_group_ids.contains(&g.id) && g.deleted_at.is_none())
                .map(|g| GroupWithBalanceEntity {
                    id: g.id,
                    name: g.name.clone(),
                    description: g.description.clone(),
                    invite_code: g.invite_code.clone(),
                    default_currency: g.default_currency,
                    deleted_at: g.deleted_at,
                    created_at: g.created_at,
                    updated_at: g.updated_at,
                    user_balance: Some(0),
                })
                .collect()
        };
        Ok(items)
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<GroupEntity>, sqlx::Error> {
        let mut groups = self.groups.lock().await;
        if let Some(g) = groups.iter_mut().find(|g| g.id == id && g.deleted_at.is_none()) {
            g.deleted_at = Some(Utc::now());
            Ok(Some(g.clone()))
        } else {
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Expense Repository
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct MockExpenseRepository {
    pub expenses: Arc<Mutex<Vec<ExpenseWithSharesEntity>>>,
}

#[async_trait]
impl ExpenseRepository for MockExpenseRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        data: &NewExpenseEntity,
    ) -> Result<ExpenseEntity, sqlx::Error> {
        let entity = ExpenseEntity {
            id: data.id,
            group_id: data.group_id,
            created_by_id: data.created_by_id,
            payer_id: data.payer_id,
            amount: data.amount,
            currency: data.currency,
            description: data.description.clone(),
            split_type: data.split_type,
            expense_date: data.expense_date,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.expenses.lock().await.push(ExpenseWithSharesEntity { expense: entity.clone(), shares: vec![] });
        Ok(entity)
    }

    async fn create_shares<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        shares: &[NewExpenseShareEntity],
    ) -> Result<Vec<ExpenseShareEntity>, sqlx::Error> {
        let mut results = Vec::new();
        for s in shares {
            let entity = ExpenseShareEntity {
                id: s.id,
                expense_id: s.expense_id,
                user_id: s.user_id,
                share_amount: s.share_amount,
                share_percentage: s.share_percentage,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };
            results.push(entity);
        }
        if let Some(last) = self.expenses.lock().await.last_mut() {
            last.shares.clone_from(&results);
        }
        Ok(results)
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseWithPayer>, sqlx::Error> {
        let expenses = self.expenses.lock().await;
        Ok(expenses.iter().find(|e| e.expense.id == id && e.expense.deleted_at.is_none()).map(|e| ExpenseWithPayer {
            id: e.expense.id,
            group_id: e.expense.group_id,
            created_by_id: e.expense.created_by_id,
            payer_id: e.expense.payer_id,
            payer_name: "Mock Payer".to_string(),
            amount: e.expense.amount,
            currency: e.expense.currency,
            description: e.expense.description.clone(),
            split_type: e.expense.split_type,
            expense_date: e.expense.expense_date,
            deleted_at: e.expense.deleted_at,
            created_at: e.expense.created_at,
            updated_at: e.expense.updated_at,
        }))
    }

    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        group_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> Result<(Vec<ExpenseWithPayer>, i64), sqlx::Error> {
        let items: Vec<ExpenseWithPayer> = {
            let expenses = self.expenses.lock().await;
            expenses
                .iter()
                .filter(|e| e.expense.group_id == group_id && e.expense.deleted_at.is_none())
                .map(|e| ExpenseWithPayer {
                    id: e.expense.id,
                    group_id: e.expense.group_id,
                    created_by_id: e.expense.created_by_id,
                    payer_id: e.expense.payer_id,
                    payer_name: "Mock Payer".to_string(),
                    amount: e.expense.amount,
                    currency: e.expense.currency,
                    description: e.expense.description.clone(),
                    split_type: e.expense.split_type,
                    expense_date: e.expense.expense_date,
                    deleted_at: e.expense.deleted_at,
                    created_at: e.expense.created_at,
                    updated_at: e.expense.updated_at,
                })
                .collect()
        };
        let total = i64::try_from(items.len()).unwrap();
        Ok((items, total))
    }

    async fn find_by_group_with_shares<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        group_id: Uuid,
    ) -> Result<Vec<ExpenseWithSharesEntity>, sqlx::Error> {
        Ok(self
            .expenses
            .lock()
            .await
            .iter()
            .filter(|e| e.expense.group_id == group_id && e.expense.deleted_at.is_none())
            .cloned()
            .collect())
    }

    async fn find_shares_by_expense<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        expense_id: Uuid,
    ) -> Result<Vec<ExpenseShareWithUser>, sqlx::Error> {
        let expenses = self.expenses.lock().await;
        Ok(expenses.iter().find(|e| e.expense.id == expense_id).map_or_else(Vec::new, |exp| {
            exp.shares
                .iter()
                .map(|s| ExpenseShareWithUser {
                    id: s.id,
                    expense_id: s.expense_id,
                    user_id: s.user_id,
                    user_name: "Share User".to_string(),
                    share_amount: s.share_amount,
                    share_percentage: s.share_percentage,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                })
                .collect()
        }))
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<ExpenseEntity>, sqlx::Error> {
        let mut expenses = self.expenses.lock().await;
        if let Some(e) = expenses.iter_mut().find(|e| e.expense.id == id && e.expense.deleted_at.is_none()) {
            e.expense.deleted_at = Some(Utc::now());
            Ok(Some(e.expense.clone()))
        } else {
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Mock Settlement Repository
// ---------------------------------------------------------------------------
#[derive(Default, Clone)]
pub struct MockSettlementRepository {
    pub settlements: Arc<Mutex<Vec<SettlementEntity>>>,
}

#[async_trait]
impl SettlementRepository for MockSettlementRepository {
    async fn create<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        data: &NewSettlementEntity,
    ) -> Result<SettlementEntity, sqlx::Error> {
        let entity = SettlementEntity {
            id: data.id,
            group_id: data.group_id,
            sender_id: data.sender_id,
            receiver_id: data.receiver_id,
            amount: data.amount,
            currency: data.currency,
            settled_at: data.settled_at,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.settlements.lock().await.push(entity.clone());
        Ok(entity)
    }

    async fn find_by_id<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error> {
        let s = self.settlements.lock().await;
        Ok(s.iter().find(|x| x.id == id && x.deleted_at.is_none()).cloned())
    }

    async fn find_by_group<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        group_id: Uuid,
    ) -> Result<Vec<SettlementEntity>, sqlx::Error> {
        let s = self.settlements.lock().await;
        Ok(s.iter().filter(|x| x.group_id == group_id && x.deleted_at.is_none()).cloned().collect())
    }

    async fn find_paginated_by_group<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        group_id: Uuid,
        _limit: i64,
        _offset: i64,
    ) -> Result<(Vec<SettlementWithUsers>, i64), sqlx::Error> {
        let items: Vec<SettlementWithUsers> = {
            let s = self.settlements.lock().await;
            s.iter()
                .filter(|x| x.group_id == group_id && x.deleted_at.is_none())
                .map(|x| SettlementWithUsers {
                    id: x.id,
                    group_id: x.group_id,
                    sender_id: x.sender_id,
                    sender_name: "Sender".to_string(),
                    receiver_id: x.receiver_id,
                    receiver_name: "Receiver".to_string(),
                    amount: x.amount,
                    currency: x.currency,
                    settled_at: x.settled_at,
                    deleted_at: x.deleted_at,
                    created_at: x.created_at,
                    updated_at: x.updated_at,
                })
                .collect()
        };
        let total = i64::try_from(items.len()).unwrap();
        Ok((items, total))
    }

    async fn soft_delete<'e, E: Executor<'e, Database = Postgres> + Send>(
        &self,
        _executor: E,
        id: Uuid,
    ) -> Result<Option<SettlementEntity>, sqlx::Error> {
        let mut s = self.settlements.lock().await;
        if let Some(item) = s.iter_mut().find(|x| x.id == id && x.deleted_at.is_none()) {
            item.deleted_at = Some(Utc::now());
            Ok(Some(item.clone()))
        } else {
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
pub fn test_jwt_config() -> JwtConfig {
    JwtConfig {
        secret: "test_secret_key_12345678901234567890".to_string(),
        issuer: "splitdebt-test".to_string(),
        audience: "splitdebt-users-test".to_string(),
        access_token_expiration_secs: 900,
        refresh_token_expiration_secs: JWT_REFRESH_TOKEN_EXPIRATION_SECS,
    }
}

pub fn test_argon2() -> Arc<Argon2<'static>> {
    Arc::new(Argon2::default())
}

pub fn test_cache() -> Arc<Cache> {
    Arc::new(Cache::Memory(MemoryCache::new()))
}

pub fn test_pool() -> sqlx::PgPool {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/dsa_test".to_string());
    sqlx::postgres::PgPoolOptions::new().connect_lazy(&url).expect("failed to create lazy test pool")
}
