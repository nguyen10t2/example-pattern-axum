use chrono::Utc;
use uuid::Uuid;

use crate::common::{
    MockExpenseRepository, MockGroupRepository, MockSettlementRepository, MockUserRepository, test_cache, test_pool,
};
use dsa::domain::{
    Currency, SplitType,
    expenses::entity::{ExpenseEntity, ExpenseShareEntity, ExpenseWithSharesEntity},
    groups::{request::CreateGroupRequest, service::GroupService},
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
