use chrono::Utc;
use uuid::Uuid;

use crate::common::{MockExpenseRepository, MockGroupRepository, seed_group, test_cache, test_pool};
use dsa::domain::{
    Currency, GroupRole, SplitType,
    expenses::{
        request::{CreateExpenseRequest, ShareRequest},
        service::ExpenseService,
    },
    groups::entity::GroupMemberWithUser,
    shared::PaginationQuery,
};
use dsa::errors::{AppError, BusinessError};

#[tokio::test]
#[ignore = "requires a live Postgres test database (set TEST_DATABASE_URL) because ExpenseService::create opens a real transaction"]
async fn test_expense_creation_and_authorization() {
    let group_id = Uuid::now_v7();
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();
    let outsider = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: u1,
        full_name: "Alice".to_string(),
        role: GroupRole::ADMIN,
        left_at: None,
        joined_at: Utc::now(),
    });
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: u2,
        full_name: "Bob".to_string(),
        role: GroupRole::MEMBER,
        left_at: None,
        joined_at: Utc::now(),
    });

    let expense_repo = MockExpenseRepository::default();
    let cache = test_cache();

    let service = ExpenseService::new(expense_repo, group_repo, cache, test_pool());

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

/// `validate_creation` chạy trước transaction nên test được với mock:
/// currency lệch nhóm và split PERCENTAGE khai man số tiền đều phải bị chặn.
#[tokio::test]
async fn test_expense_create_rejects_currency_mismatch_and_percentage_amount_mismatch() {
    let group_id = Uuid::now_v7();
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    for (user_id, name) in [(u1, "Alice"), (u2, "Bob")] {
        group_repo.members.lock().await.push(GroupMemberWithUser {
            group_id,
            user_id,
            full_name: name.to_string(),
            role: GroupRole::MEMBER,
            left_at: None,
            joined_at: Utc::now(),
        });
    }

    let service = ExpenseService::new(MockExpenseRepository::default(), group_repo, test_cache(), test_pool());

    let base = |currency, split_type, shares| CreateExpenseRequest {
        group_id,
        payer_id: u1,
        amount: 100,
        currency,
        description: "Coffee".to_string(),
        expense_date: None,
        split_type: Some(split_type),
        shares,
    };

    // Nhóm VND, expense USD → BadRequest.
    let wrong_currency = base(
        Currency::USD,
        SplitType::EXACT,
        vec![
            ShareRequest { user_id: u1, share_amount: 50, share_percentage: None },
            ShareRequest { user_id: u2, share_amount: 50, share_percentage: None },
        ],
    );
    assert!(matches!(service.create(wrong_currency, u1).await, Err(AppError::Business(BusinessError::BadRequest(_)))));

    // PERCENTAGE đúng 100% nhưng share_amount = 0 hết → phải chặn (trước đây lọt).
    let percent_ledger_gap = base(
        Currency::VND,
        SplitType::PERCENTAGE,
        vec![
            ShareRequest { user_id: u1, share_amount: 0, share_percentage: Some(6000) },
            ShareRequest { user_id: u2, share_amount: 0, share_percentage: Some(4000) },
        ],
    );
    assert!(matches!(
        service.create(percent_ledger_gap, u1).await,
        Err(AppError::Business(BusinessError::BadRequest(_)))
    ));
}
