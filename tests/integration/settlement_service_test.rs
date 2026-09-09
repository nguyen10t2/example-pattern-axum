use chrono::Utc;
use uuid::Uuid;

use crate::common::{MockGroupRepository, MockSettlementRepository, test_cache, test_pool};
use dsa::domain::{
    Currency, GroupRole,
    groups::entity::GroupMemberWithUser,
    settlements::{request::CreateSettlementRequest, service::SettlementService},
    shared::PaginationQuery,
};

#[tokio::test]
async fn test_settlement_lifecycle_and_permissions() {
    let group_id = Uuid::now_v7();
    let sender = Uuid::now_v7();
    let receiver = Uuid::now_v7();
    let outsider = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: sender,
        full_name: "Sender".to_string(),
        role: GroupRole::MEMBER,
        joined_at: Utc::now(),
    });
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: receiver,
        full_name: "Receiver".to_string(),
        role: GroupRole::MEMBER,
        joined_at: Utc::now(),
    });

    let settlement_repo = MockSettlementRepository::default();
    let cache = test_cache();

    let service = SettlementService::new(settlement_repo, group_repo, cache, test_pool());

    // Outsider creation fails
    let outsider_res = service
        .create(
            CreateSettlementRequest {
                group_id,
                sender_id: sender,
                receiver_id: receiver,
                amount: 75,
                currency: Currency::VND,
                settled_at: None,
            },
            outsider,
        )
        .await;
    assert!(outsider_res.is_err());

    // Valid settlement by sender
    let settlement = service
        .create(
            CreateSettlementRequest {
                group_id,
                sender_id: sender,
                receiver_id: receiver,
                amount: 75,
                currency: Currency::VND,
                settled_at: None,
            },
            sender,
        )
        .await
        .unwrap();

    assert_eq!(settlement.amount, 75);

    // Fetch paginated
    let list =
        service.find_by_group(group_id, receiver, PaginationQuery { page: Some(1), limit: Some(10) }).await.unwrap();
    assert_eq!(list.items.len(), 1);

    // Cancel settlement
    service.cancel_settlement(settlement.id, sender).await.unwrap();

    // Verify cancelled
    let after_cancel = service.find_by_id(settlement.id, sender).await;
    assert!(after_cancel.is_err());
}
