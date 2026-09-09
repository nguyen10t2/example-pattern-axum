use crate::domain::expenses::{
    entity::{ExpenseEntity, ExpenseShareEntity, ExpenseShareWithUser, ExpenseWithPayer},
    response::{ExpenseResponse, ExpenseShareResponse},
};

pub struct ExpenseMapper;

impl ExpenseMapper {
    #[must_use]
    /// Map expense kèm payer sang response.
    pub fn to_response_with_payer(
        entity: ExpenseWithPayer,
        shares: Option<Vec<ExpenseShareResponse>>,
    ) -> ExpenseResponse {
        ExpenseResponse {
            id: entity.id,
            group_id: entity.group_id,
            created_by_id: entity.created_by_id,
            payer_id: entity.payer_id,
            payer_name: Some(entity.payer_name),
            amount: entity.amount,
            currency: entity.currency,
            description: entity.description,
            split_type: entity.split_type,
            expense_date: entity.expense_date,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            shares,
        }
    }

    #[must_use]
    /// Map expense entity sang response (tên payer và shares truyền rời).
    pub fn to_response_from_entity(
        entity: ExpenseEntity,
        payer_name: Option<String>,
        shares: Option<Vec<ExpenseShareResponse>>,
    ) -> ExpenseResponse {
        ExpenseResponse {
            id: entity.id,
            group_id: entity.group_id,
            created_by_id: entity.created_by_id,
            payer_id: entity.payer_id,
            payer_name,
            amount: entity.amount,
            currency: entity.currency,
            description: entity.description,
            split_type: entity.split_type,
            expense_date: entity.expense_date,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
            shares,
        }
    }

    /// Map share entity sang response (tên user truyền rời).
    #[must_use]
    pub const fn to_share_response(entity: &ExpenseShareEntity, user_name: Option<String>) -> ExpenseShareResponse {
        ExpenseShareResponse {
            id: entity.id,
            expense_id: entity.expense_id,
            user_id: entity.user_id,
            user_name,
            share_amount: entity.share_amount,
            share_percentage: entity.share_percentage,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }

    #[must_use]
    /// Map share kèm user sang response.
    pub fn to_share_response_with_user(entity: ExpenseShareWithUser) -> ExpenseShareResponse {
        ExpenseShareResponse {
            id: entity.id,
            expense_id: entity.expense_id,
            user_id: entity.user_id,
            user_name: Some(entity.user_name),
            share_amount: entity.share_amount,
            share_percentage: entity.share_percentage,
            created_at: entity.created_at,
            updated_at: entity.updated_at,
        }
    }
}
