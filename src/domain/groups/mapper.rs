use crate::domain::groups::{
    entity::{GroupEntity, GroupMemberWithUser, GroupWithBalanceEntity},
    response::{GroupMemberResponse, GroupResponse},
};

pub struct GroupMapper;

impl GroupMapper {
    #[must_use]
    /// Map group entity sang response.
    pub fn to_response(entity: GroupEntity) -> GroupResponse {
        GroupResponse {
            id: entity.id,
            name: entity.name,
            description: entity.description,
            invite_code: entity.invite_code,
            default_currency: entity.default_currency,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            user_balance: None,
        }
    }

    #[must_use]
    /// Map group kèm số dư sang response.
    pub fn to_response_with_balance(entity: GroupWithBalanceEntity) -> GroupResponse {
        GroupResponse {
            id: entity.id,
            name: entity.name,
            description: entity.description,
            invite_code: entity.invite_code,
            default_currency: entity.default_currency,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            user_balance: Some(entity.user_balance.unwrap_or(0)),
        }
    }

    #[must_use]
    /// Map thành viên kèm user sang response.
    pub fn to_member_response(entity: GroupMemberWithUser) -> GroupMemberResponse {
        GroupMemberResponse {
            group_id: entity.group_id,
            user_id: entity.user_id,
            full_name: entity.full_name,
            role: entity.role,
            joined_at: entity.joined_at,
        }
    }
}
