use chrono::Utc;
use uuid::Uuid;

use crate::common::{MockGroupRepository, MockSettlementRepository, seed_group, test_cache, test_pool};
use dsa::domain::{
    Currency, GroupRole,
    groups::entity::GroupMemberWithUser,
    settlements::{request::CreateSettlementRequest, service::SettlementService},
    shared::PaginationQuery,
};
use dsa::errors::{AppError, BusinessError};

#[tokio::test]
async fn test_settlement_lifecycle_and_permissions() {
    let group_id = Uuid::now_v7();
    let sender = Uuid::now_v7();
    let receiver = Uuid::now_v7();
    let outsider = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: sender,
        full_name: "Sender".to_string(),
        role: GroupRole::MEMBER,
        left_at: None,
        joined_at: Utc::now(),
    });
    group_repo.members.lock().await.push(GroupMemberWithUser {
        group_id,
        user_id: receiver,
        full_name: "Receiver".to_string(),
        role: GroupRole::MEMBER,
        left_at: None,
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

#[tokio::test]
async fn test_settlement_create_rejects_non_party_member() {
    let group_id = Uuid::now_v7();
    let sender = Uuid::now_v7();
    let receiver = Uuid::now_v7();
    let third_member = Uuid::now_v7();

    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    for (user_id, name) in [(sender, "Sender"), (receiver, "Receiver"), (third_member, "Third")] {
        group_repo.members.lock().await.push(GroupMemberWithUser {
            group_id,
            user_id,
            full_name: name.to_string(),
            role: GroupRole::MEMBER,
            left_at: None,
            joined_at: Utc::now(),
        });
    }

    let service = SettlementService::new(MockSettlementRepository::default(), group_repo, test_cache(), test_pool());

    let input = || CreateSettlementRequest {
        group_id,
        sender_id: sender,
        receiver_id: receiver,
        amount: 50,
        currency: Currency::VND,
        settled_at: None,
    };

    // Member thứ 3 (không phải sender/receiver) ghi hộ → 403, chống giả mạo.
    assert!(matches!(
        service.create(input(), third_member).await,
        Err(AppError::Business(BusinessError::NotSettlementParty))
    ));

    // Receiver tự ghi → OK.
    service.create(input(), receiver).await.unwrap();
}

#[tokio::test]
async fn test_settlement_rejects_self_transfer_and_currency_mismatch() {
    let group_id = Uuid::now_v7();
    let user = Uuid::now_v7();
    let other = Uuid::now_v7();
    let group_repo = MockGroupRepository::default();
    seed_group(&group_repo, group_id, Currency::VND).await;
    for (user_id, name) in [(user, "User"), (other, "Other")] {
        group_repo.members.lock().await.push(GroupMemberWithUser {
            group_id,
            user_id,
            full_name: name.to_string(),
            role: GroupRole::MEMBER,
            left_at: None,
            joined_at: Utc::now(),
        });
    }

    let service = SettlementService::new(MockSettlementRepository::default(), group_repo, test_cache(), test_pool());

    // sender == receiver: no-op record, không được ghi vào sổ.
    let self_transfer = CreateSettlementRequest {
        group_id,
        sender_id: user,
        receiver_id: user,
        amount: 50,
        currency: Currency::VND,
        settled_at: None,
    };
    assert!(matches!(service.create(self_transfer, user).await, Err(AppError::Business(BusinessError::BadRequest(_)))));

    // Nhóm VND mà ghi settlement USD → lệch sổ nợ (engine không quy đổi).
    let wrong_currency = CreateSettlementRequest {
        group_id,
        sender_id: user,
        receiver_id: other,
        amount: 50,
        currency: Currency::USD,
        settled_at: None,
    };
    assert!(matches!(
        service.create(wrong_currency, user).await,
        Err(AppError::Business(BusinessError::BadRequest(_)))
    ));
}
