use dsa::domain::{
    SplitType,
    expenses::strategy::{SplitContext, SplitShareInput, SplitStrategyFactory},
};
use uuid::Uuid;

#[test]
fn test_equal_split_strategy_success() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
    let ctx = SplitContext {
        total_amount: 100,
        shares: vec![
            SplitShareInput { user_id: u1, share_amount: 50, share_percentage: None },
            SplitShareInput { user_id: u2, share_amount: 50, share_percentage: None },
        ],
    };

    assert!(strategy.validate(&ctx).is_ok());
}

#[test]
fn test_equal_split_strategy_mismatch() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let strategy = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
    let ctx = SplitContext {
        total_amount: 100,
        shares: vec![
            SplitShareInput { user_id: u1, share_amount: 40, share_percentage: None },
            SplitShareInput { user_id: u2, share_amount: 50, share_percentage: None },
        ],
    };

    assert!(strategy.validate(&ctx).is_err());
}

#[test]
fn test_percentage_split_strategy_success() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let strategy = SplitStrategyFactory::get_strategy(&SplitType::PERCENTAGE);
    let ctx = SplitContext {
        total_amount: 200,
        shares: vec![
            SplitShareInput {
                user_id: u1,
                share_amount: 120,
                share_percentage: Some(6000), // 60.00%
            },
            SplitShareInput {
                user_id: u2,
                share_amount: 80,
                share_percentage: Some(4000), // 40.00%
            },
        ],
    };

    assert!(strategy.validate(&ctx).is_ok());
}

#[test]
fn test_percentage_split_strategy_not_100_percent() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let strategy = SplitStrategyFactory::get_strategy(&SplitType::PERCENTAGE);
    let ctx = SplitContext {
        total_amount: 200,
        shares: vec![
            SplitShareInput { user_id: u1, share_amount: 120, share_percentage: Some(6000) },
            SplitShareInput {
                user_id: u2,
                share_amount: 80,
                share_percentage: Some(3000), // Only 90% total
            },
        ],
    };

    assert!(strategy.validate(&ctx).is_err());
}

#[test]
fn test_exact_split_strategy_success() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();

    let strategy = SplitStrategyFactory::get_strategy(&SplitType::EXACT);
    let ctx = SplitContext {
        total_amount: 150,
        shares: vec![
            SplitShareInput { user_id: u1, share_amount: 70, share_percentage: None },
            SplitShareInput { user_id: u2, share_amount: 80, share_percentage: None },
        ],
    };

    assert!(strategy.validate(&ctx).is_ok());
}
