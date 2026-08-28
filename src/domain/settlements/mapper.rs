use crate::domain::settlements::{
    entity::{SettlementEntity, SettlementWithUsers},
    response::SettlementResponse,
};

pub struct SettlementMapper;

impl SettlementMapper {
    pub fn to_response(entity: &SettlementEntity) -> SettlementResponse {
        SettlementResponse {
            id: entity.id,
            group_id: entity.group_id,
            sender_id: entity.sender_id,
            sender_name: None,
            receiver_id: entity.receiver_id,
            receiver_name: None,
            amount: entity.amount,
            currency: entity.currency,
            settled_at: entity.settled_at,
            created_at: entity.created_at,
        }
    }

    pub fn to_response_with_users(entity: &SettlementWithUsers) -> SettlementResponse {
        SettlementResponse {
            id: entity.id,
            group_id: entity.group_id,
            sender_id: entity.sender_id,
            sender_name: Some(entity.sender_name.clone()),
            receiver_id: entity.receiver_id,
            receiver_name: Some(entity.receiver_name.clone()),
            amount: entity.amount,
            currency: entity.currency,
            settled_at: entity.settled_at,
            created_at: entity.created_at,
        }
    }

    pub fn to_response_list_with_users(entities: &[SettlementWithUsers]) -> Vec<SettlementResponse> {
        entities.iter().map(Self::to_response_with_users).collect()
    }
}
