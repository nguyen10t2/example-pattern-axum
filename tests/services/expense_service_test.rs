use chrono::Utc;
use uuid::Uuid;

use crate::common::{MockExpenseRepository, MockGroupRepository, test_cache};
use dsa::domain::{
    Currency, GroupRole, SplitType,
    expenses::{
        request::{CreateExpenseRequest, ShareRequest},
        service::ExpenseService,
    },
    groups::entity::GroupMemberWithUser,
    shared::PaginationQuery,
};

#[tokio::test]
async fn test_expense_creation_and_authorization() {
    let group_id = Uuid::now_v7();
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();
    let outsider = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: u1,
        full_name: "Alice".to_string(),
        role: GroupRole::ADMIN,
        joined_at: Utc::now(),
    });
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: u2,
        full_name: "Bob".to_string(),
        role: GroupRole::MEMBER,
        joined_at: Utc::now(),
    });

    let expense_repo = MockExpenseRepository::default();
    let cache = test_cache();

    let service = ExpenseService::new(expense_repo, group_repo, cache);

    // Outsider cannot create expense
    let outsider_attempt = service
        .create(
            CreateExpenseRequest {
                group_id,
                payer_id: u1,
                amount: 100,
                currency: Currency::VND,
                description: "Coffee".to_string(),
                expense_date: None,
                split_type: Some(SplitType::EQUAL),
                shares: vec![
                    ShareRequest { user_id: u1, share_amount: 50, share_percentage: None },
                    ShareRequest { user_id: u2, share_amount: 50, share_percentage: None },
                ],
            },
            outsider,
        )
        .await;
    assert!(outsider_attempt.is_err());

    // Valid creation by u1
    let created = service
        .create(
            CreateExpenseRequest {
                group_id,
                payer_id: u1,
                amount: 100,
                currency: Currency::VND,
                description: "Coffee".to_string(),
                expense_date: None,
                split_type: Some(SplitType::EQUAL),
                shares: vec![
                    ShareRequest { user_id: u1, share_amount: 50, share_percentage: None },
                    ShareRequest { user_id: u2, share_amount: 50, share_percentage: None },
                ],
            },
            u1,
        )
        .await
        .unwrap();

    assert_eq!(created.amount, 100);
    assert_eq!(created.shares.as_ref().unwrap().len(), 2);

    // Fetch by group paginated
    let list = service.find_by_group(group_id, u2, PaginationQuery { page: Some(1), limit: Some(10) }).await.unwrap();
    assert_eq!(list.items.len(), 1);

    // Delete by non-creator member (u2) should fail if not admin
    let u2_delete = service.delete_expense(created.id, u2).await;
    assert!(u2_delete.is_err());

    // Delete by creator (u1) succeeds
    service.delete_expense(created.id, u1).await.unwrap();
}
