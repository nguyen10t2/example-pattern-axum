use crate::{
    domain::SplitType,
    errors::{AppError, BusinessError},
};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct SplitShareInput {
    pub user_id: Uuid,
    pub share_amount: i64,
    pub share_percentage: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct SplitContext {
    pub total_amount: i64,
    pub shares: Vec<SplitShareInput>,
}

/// Kiểm tra một cách chia có hợp lệ với tổng tiền không.
pub trait SplitStrategy: Send + Sync {
    /// Validate context theo quy tắc của từng loại split.
    ///
    /// # Errors
    ///
    /// Trả `BadRequest` khi split sai quy tắc.
    fn validate(&self, context: &SplitContext) -> Result<(), AppError>;
}

/// Check chung cho strategy theo số tiền: tổng shares phải bằng total.
///
/// # Errors
///
/// Trả `BadRequest` khi tổng shares khác total.
fn validate_sum(context: &SplitContext) -> Result<(), AppError> {
    let sum: i64 = context.shares.iter().map(|s| s.share_amount).sum();
    if sum != context.total_amount {
        return Err(AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string())));
    }
    Ok(())
}

/// Ép chia đều: mỗi share phải là `floor(total / n)` hoặc `ceil(total / n)`.
///
/// Tổng nguyên hiếm khi chia hết (vd 100 / 3) nên không thể đòi bằng nhau tuyệt đối —
/// 33/33/34 là split đều hợp lệ, còn 90/5/5 thì không. Kết hợp với [`validate_sum`],
/// rule này chỉ nhận đúng các split rải phần dư mỗi share 1 đơn vị.
///
/// # Errors
///
/// Trả `BadRequest` khi có share lệch khỏi `floor`/`ceil`.
fn validate_equal_distribution(context: &SplitContext) -> Result<(), AppError> {
    let count = context.shares.len();
    if count == 0 {
        return Ok(());
    }
    let count =
        i64::try_from(count).map_err(|_| AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string())))?;
    let base = context.total_amount.div_euclid(count);
    let evenly_spread = context.shares.iter().all(|s| s.share_amount == base || s.share_amount == base + 1);
    if evenly_spread { Ok(()) } else { Err(AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string()))) }
}

pub struct EqualSplitStrategy;

impl SplitStrategy for EqualSplitStrategy {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError> {
        validate_sum(context)?;
        validate_equal_distribution(context)
    }
}

pub struct ExactSplitStrategy;

impl SplitStrategy for ExactSplitStrategy {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError> {
        validate_sum(context)
    }
}

pub struct PercentageSplitStrategy;

impl SplitStrategy for PercentageSplitStrategy {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError> {
        let total_pct: i32 = context.shares.iter().map(|s| s.share_percentage.unwrap_or(0)).sum();
        // Input is percentage * 100 (e.g. 10000 for 100%)
        if total_pct != 10000 {
            return Err(AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string())));
        }
        Ok(())
    }
}

pub struct SplitStrategyFactory;

impl SplitStrategyFactory {
    /// Chọn strategy theo loại split của expense.
    #[must_use]
    pub fn get_strategy(split_type: &SplitType) -> Box<dyn SplitStrategy> {
        match split_type {
            SplitType::EQUAL => Box::new(EqualSplitStrategy),
            SplitType::EXACT => Box::new(ExactSplitStrategy),
            SplitType::PERCENTAGE => Box::new(PercentageSplitStrategy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equal_split_strategy_passes_when_sum_matches() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
        let ctx = SplitContext {
            total_amount: 100,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 34, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&ctx).is_ok());
    }

    #[test]
    fn test_equal_split_strategy_fails_when_sum_mismatches() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
        let ctx = SplitContext {
            total_amount: 100,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&ctx).is_err());
    }

    #[test]
    fn test_equal_split_rejects_uneven_shares_with_matching_sum() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
        let ctx = SplitContext {
            total_amount: 100,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 90, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 5, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 5, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&ctx).is_err());
    }

    #[test]
    fn test_equal_split_rejects_shortfall_distribution() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
        let ctx = SplitContext {
            total_amount: 100,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 33, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&ctx).is_err());
    }

    #[test]
    fn test_equal_split_passes_on_exact_division_and_single_share() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
        let even_ctx = SplitContext {
            total_amount: 100,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 25, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 25, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 25, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 25, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&even_ctx).is_ok());

        let single_ctx = SplitContext {
            total_amount: 50,
            shares: vec![SplitShareInput { user_id: Uuid::now_v7(), share_amount: 50, share_percentage: None }],
        };
        assert!(strategy.validate(&single_ctx).is_ok());
    }

    #[test]
    fn test_exact_split_strategy() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::EXACT);
        let ctx = SplitContext {
            total_amount: 500,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 200, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 300, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&ctx).is_ok());

        let bad_ctx = SplitContext {
            total_amount: 500,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 200, share_percentage: None },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 200, share_percentage: None },
            ],
        };
        assert!(strategy.validate(&bad_ctx).is_err());
    }

    #[test]
    fn test_percentage_split_strategy() {
        let strategy = SplitStrategyFactory::get_strategy(&SplitType::PERCENTAGE);
        let ctx = SplitContext {
            total_amount: 1000,
            shares: vec![
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 500, share_percentage: Some(5000) },
                SplitShareInput { user_id: Uuid::now_v7(), share_amount: 500, share_percentage: Some(5000) },
            ],
        };
        assert!(strategy.validate(&ctx).is_ok());

        let bad_ctx = SplitContext {
            total_amount: 1000,
            shares: vec![SplitShareInput { user_id: Uuid::now_v7(), share_amount: 500, share_percentage: Some(4000) }],
        };
        assert!(strategy.validate(&bad_ctx).is_err());
    }
}
