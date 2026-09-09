use chrono::Utc;
use uuid::Uuid;

use crate::common::{
    MockExpenseRepository, MockGroupRepository, MockSettlementRepository, MockUserRepository, test_cache, test_pool,
};
use dsa::domain::{
    Currency, GroupRole,
    groups::{
        entity::{GroupEntity, GroupMemberWithUser},
        request::AddMemberRequest,
        service::GroupService,
    },
    users::entity::UserEntity,
};
use dsa::errors::{AppError, BusinessError};

type TestGroupService =
    GroupService<MockGroupRepository, MockExpenseRepository, MockSettlementRepository, MockUserRepository>;

fn test_group_service()
-> (TestGroupService, MockGroupRepository, MockExpenseRepository, MockSettlementRepository, MockUserRepository) {
    let group_repo = MockGroupRepository::default();
    let expense_repo = MockExpenseRepository::default();
    let settlement_repo = MockSettlementRepository::default();
    let user_repo = MockUserRepository::default();
    let service = GroupService::new(
        group_repo.clone(),
        expense_repo.clone(),
        settlement_repo.clone(),
        user_repo.clone(),
        test_cache(),
        test_pool(),
    );
    (service, group_repo, expense_repo, settlement_repo, user_repo)
}

async fn seed_user(repo: &MockUserRepository, id: Uuid, email: &str) {
    repo.users.lock().await.push(UserEntity {
        id,
        full_name: "Test User".to_string(),
        email: email.to_string(),
        email_verified: true,
        password_hash: None,
        google_id: None,
        avatar_url: None,
        phone: None,
        phone_verified: false,
        preferred_currency: Currency::VND,
        is_active: true,
        deleted_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
}

async fn seed_group(repo: &MockGroupRepository, id: Uuid) {
    repo.groups.lock().await.push(GroupEntity {
        id,
        name: "Test Group".to_string(),
        description: None,
        invite_code: None,
        default_currency: Currency::VND,
        deleted_at: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    });
}

async fn seed_member(repo: &MockGroupRepository, group_id: Uuid, user_id: Uuid, role: GroupRole) {
    repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id,
        full_name: "Test Member".to_string(),
        role,
        joined_at: Utc::now(),
    });
}

#[tokio::test]
async fn test_ensure_membership_cached_hit() {
    let (service, group_repo, _, _, user_repo) = test_group_service();
    let group_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    seed_user(&user_repo, user_id, "member@example.com").await;
    seed_group(&group_repo, group_id).await;
    seed_member(&group_repo, group_id, user_id, GroupRole::MEMBER).await;

    // Miss lần đầu (nạp cache), hit các lần sau — kết quả phải giống nhau.
    assert!(service.ensure_membership(group_id, user_id).await.is_ok());
    assert!(service.ensure_membership(group_id, user_id).await.is_ok());

    let outsider = Uuid::now_v7();
    assert!(matches!(
        service.ensure_membership(group_id, outsider).await,
        Err(AppError::Business(BusinessError::NotGroupMember))
    ));
}

#[tokio::test]
async fn test_add_member_invalidates_membership_cache() {
    let (service, group_repo, _, _, user_repo) = test_group_service();
    let group_id = Uuid::now_v7();
    let admin_id = Uuid::now_v7();
    let newcomer_id = Uuid::now_v7();
    seed_user(&user_repo, admin_id, "admin@example.com").await;
    seed_user(&user_repo, newcomer_id, "newcomer@example.com").await;
    seed_group(&group_repo, group_id).await;
    seed_member(&group_repo, group_id, admin_id, GroupRole::ADMIN).await;

    // Cache chưa có newcomer.
    assert!(matches!(
        service.ensure_membership(group_id, newcomer_id).await,
        Err(AppError::Business(BusinessError::NotGroupMember))
    ));

    service.add_member(group_id, AddMemberRequest { user_id: newcomer_id, role: None }, admin_id).await.unwrap();

    // Không invalidate thì dòng này vẫn Err (cache cũ) — đây chính là điều cần chứng minh.
    assert!(service.ensure_membership(group_id, newcomer_id).await.is_ok());
}
