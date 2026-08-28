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

pub trait SplitStrategy: Send + Sync {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError>;
}

pub struct EqualSplitStrategy;

impl SplitStrategy for EqualSplitStrategy {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError> {
        let sum: i64 = context.shares.iter().map(|s| s.share_amount).sum();
        if sum != context.total_amount {
            return Err(AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string())));
        }
        Ok(())
    }
}

pub struct ExactSplitStrategy;

impl SplitStrategy for ExactSplitStrategy {
    fn validate(&self, context: &SplitContext) -> Result<(), AppError> {
        let sum: i64 = context.shares.iter().map(|s| s.share_amount).sum();
        if sum != context.total_amount {
            return Err(AppError::Business(BusinessError::BadRequest("BAD_REQUEST".to_string())));
        }
        Ok(())
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
