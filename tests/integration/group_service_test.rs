use chrono::Utc;
use uuid::Uuid;

use crate::common::{
    MockExpenseRepository, MockGroupRepository, MockSettlementRepository, MockUserRepository, seed_group, test_cache,
    test_pool,
};
use dsa::domain::{
    Currency, GroupRole, SplitType,
    expenses::entity::{ExpenseEntity, ExpenseShareEntity, ExpenseWithSharesEntity},
    groups::{entity::GroupMemberWithUser, request::CreateGroupRequest, service::GroupService},
};

#[tokio::test]
#[ignore = "requires a live Postgres test database (set TEST_DATABASE_URL) because GroupService::create opens a real transaction"]
async fn test_group_lifecycle_and_summary() {
    let group_repo = MockGroupRepository::default();
    let expense_repo = MockExpenseRepository::default();
    let settlement_repo = MockSettlementRepository::default();
    let user_repo = MockUserRepository::default();
    let cache = test_cache();

    let service = GroupService::new(
        group_repo.clone(),
        expense_repo.clone(),
        settlement_repo.clone(),
        user_repo.clone(),
        cache.clone(),
        test_pool(),
    );

    let creator_id = Uuid::now_v7();
    let group = service
        .create(
            CreateGroupRequest {
                name: "Trip to Da Lat".to_string(),
                description: Some("Weekend getaway".to_string()),
                default_currency: Some(Currency::VND),
            },
            creator_id,
        )
        .await
        .unwrap();

    assert_eq!(group.name, "Trip to Da Lat");
    assert!(group.invite_code.is_some());

    // Member joins via invite code
    let member_id = Uuid::now_v7();
    let invite_code = group.invite_code.clone().unwrap();
    let joined = service.join_by_invite_code(&invite_code, member_id).await.unwrap();
    assert_eq!(joined.id, group.id);

    // Add expense to mock repo: creator paid 200, shared 100 with member
    expense_repo.expenses.lock().await.push(ExpenseWithSharesEntity {
        expense: ExpenseEntity {
            id: Uuid::now_v7(),
            group_id: group.id,
            created_by_id: creator_id,
            payer_id: creator_id,
            amount: 200,
            currency: Currency::VND,
            description: "Hotel".to_string(),
            split_type: SplitType::EQUAL,
            expense_date: Utc::now(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        shares: vec![
            ExpenseShareEntity {
                id: Uuid::now_v7(),
                expense_id: Uuid::now_v7(),
                user_id: creator_id,
                share_amount: 100,
                share_percentage: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            ExpenseShareEntity {
                id: Uuid::now_v7(),
                expense_id: Uuid::now_v7(),
                user_id: member_id,
                share_amount: 100,
                share_percentage: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ],
    });

    // Get Group summary
    let summary = service.get_group_summary(group.id, creator_id).await.unwrap();
    assert_eq!(summary.suggestions.len(), 1);
    assert_eq!(summary.suggestions[0].from_user_id, member_id);
    assert_eq!(summary.suggestions[0].to_user_id, creator_id);
    assert_eq!(summary.suggestions[0].amount, 100);

    // Delete group by admin
    service.delete_group(group.id, creator_id).await.unwrap();

    // Verify deleted
    let after_delete = service.find_by_id(group.id, None).await;
    assert!(after_delete.is_err());
}

fn test_group_service(
    group_repo: &MockGroupRepository,
) -> GroupService<MockGroupRepository, MockExpenseRepository, MockSettlementRepository, MockUserRepository> {
    GroupService::new(
        group_repo.clone(),
        MockExpenseRepository::default(),
        MockSettlementRepository::default(),
        MockUserRepository::default(),
        test_cache(),
        test_pool(),
    )
}

/// Thành viên cuối cùng rời nhóm thì nhóm phải bị xoá, không để lại nhóm
/// 0 thành viên / 0 admin mà invite code vẫn còn sống.
#[tokio::test]
async fn test_last_member_leaving_deletes_group() {
    let group_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id,
        full_name: "Solo".to_string(),
        role: GroupRole::MEMBER,
        left_at: None,
        joined_at: Utc::now(),
    });

    let service = test_group_service(&group_repo);
    service.leave_group(group_id, user_id).await.unwrap();

    assert!(group_repo.groups.lock().await.iter().all(|g| g.id != group_id));
}

/// Admin duy nhất không được rời khi còn thành viên khác (nhóm sẽ mất admin).
#[tokio::test]
async fn test_only_admin_cannot_leave_while_others_remain() {
    let group_id = Uuid::now_v7();
    let admin_id = Uuid::now_v7();
    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    for (user_id, role) in [(admin_id, GroupRole::ADMIN), (Uuid::now_v7(), GroupRole::MEMBER)] {
        group_repo.members.lock().await.push(GroupMemberWithUser {
            group_id,
            user_id,
            full_name: "Member".to_string(),
            role,
            left_at: None,
            joined_at: Utc::now(),
        });
    }

    let service = test_group_service(&group_repo);
    let result = service.leave_group(group_id, admin_id).await;
    assert!(matches!(result, Err(dsa::errors::AppError::Business(dsa::errors::BusinessError::ValidationError(_)))));
}

/// Người đã rời nhóm join lại bằng invite code phải được kích hoạt lại (trước đây
/// unique violation bị nuốt và API trả Ok trong khi membership vẫn `left_at`).
#[tokio::test]
async fn test_left_member_can_rejoin_by_invite_code() {
    let group_id = Uuid::now_v7();
    let user_id = Uuid::now_v7();
    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    group_repo.groups.lock().await[0].invite_code = Some("ABCD1234".to_string());
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id,
        full_name: "Returning".to_string(),
        role: GroupRole::MEMBER,
        left_at: Some(Utc::now()),
        joined_at: Utc::now(),
    });

    let service = test_group_service(&group_repo);
    service.join_by_invite_code("ABCD1234", user_id).await.unwrap();

    let members = group_repo.members.lock().await;
    let member = members.iter().find(|m| m.user_id == user_id).unwrap();
    assert!(member.left_at.is_none());
    assert_eq!(members.iter().filter(|m| m.user_id == user_id).count(), 1);
}

/// Invite code cũ không được hạ quyền admin đang active.
#[tokio::test]
async fn test_join_by_invite_code_keeps_existing_admin_role() {
    let group_id = Uuid::now_v7();
    let admin_id = Uuid::now_v7();
    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    group_repo.groups.lock().await[0].invite_code = Some("ABCD5678".to_string());
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: admin_id,
        full_name: "Admin".to_string(),
        role: GroupRole::ADMIN,
        left_at: None,
        joined_at: Utc::now(),
    });

    let service = test_group_service(&group_repo);
    service.join_by_invite_code("ABCD5678", admin_id).await.unwrap();

    let members = group_repo.members.lock().await;
    assert_eq!(members.iter().find(|m| m.user_id == admin_id).unwrap().role, GroupRole::ADMIN);
}
