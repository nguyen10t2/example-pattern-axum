use dsa::domain::debt_engine::{DebtEngine, ExpenseForEngine, SettlementForEngine, ShareForEngine};
use uuid::Uuid;

#[tokio::test]
async fn test_zero_balances_for_equal_split() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();
    let user_ids = vec![u1, u2];

    let expenses = vec![ExpenseForEngine {
        payer_id: u1,
        amount: 100,
        shares: vec![ShareForEngine { user_id: u1, amount: 50 }, ShareForEngine { user_id: u2, amount: 50 }],
    }];
    let settlements = vec![];

    let balances = DebtEngine::calculate_net_balances(&user_ids, &expenses, &settlements).await.unwrap();
    let b1 = balances.iter().find(|b| b.user_id == u1).unwrap();
    let b2 = balances.iter().find(|b| b.user_id == u2).unwrap();

    assert_eq!(b1.net_amount, 50);
    assert_eq!(b2.net_amount, -50);

    let suggestions = DebtEngine::simplify_debts(&balances).await.unwrap();
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].from_user_id, u2);
    assert_eq!(suggestions[0].to_user_id, u1);
    assert_eq!(suggestions[0].amount, 50);
}

#[tokio::test]
async fn test_neutralizing_with_settlements() {
    let u1 = Uuid::now_v7();
    let u2 = Uuid::now_v7();
    let user_ids = vec![u1, u2];

    let expenses = vec![ExpenseForEngine {
        payer_id: u1,
        amount: 100,
        shares: vec![ShareForEngine { user_id: u1, amount: 50 }, ShareForEngine { user_id: u2, amount: 50 }],
    }];
    let settlements = vec![SettlementForEngine { sender_id: u2, receiver_id: u1, amount: 50 }];

    let balances = DebtEngine::calculate_net_balances(&user_ids, &expenses, &settlements).await.unwrap();
    let b1 = balances.iter().find(|b| b.user_id == u1).unwrap();
    let b2 = balances.iter().find(|b| b.user_id == u2).unwrap();

    assert_eq!(b1.net_amount, 0);
    assert_eq!(b2.net_amount, 0);

    let suggestions = DebtEngine::simplify_debts(&balances).await.unwrap();
    assert_eq!(suggestions.len(), 0);
}

#[tokio::test]
async fn test_empty_inputs() {
    let balances = DebtEngine::calculate_net_balances(&[], &[], &[]).await.unwrap();
    assert_eq!(balances.len(), 0);
    let suggestions = DebtEngine::simplify_debts(&[]).await.unwrap();
    assert_eq!(suggestions.len(), 0);
}

#[tokio::test]
async fn test_complex_circular_debts_simplification() {
    let a = Uuid::now_v7();
    let b = Uuid::now_v7();
    let c = Uuid::now_v7();
    let d = Uuid::now_v7();

    let expenses = vec![
        ExpenseForEngine {
            payer_id: a,
            amount: 400,
            shares: vec![
                ShareForEngine { user_id: a, amount: 100 },
                ShareForEngine { user_id: b, amount: 100 },
                ShareForEngine { user_id: c, amount: 100 },
                ShareForEngine { user_id: d, amount: 100 },
            ],
        },
        ExpenseForEngine {
            payer_id: b,
            amount: 200,
            shares: vec![ShareForEngine { user_id: b, amount: 100 }, ShareForEngine { user_id: c, amount: 100 }],
        },
    ];

    let balances = DebtEngine::calculate_net_balances(&[a, b, c, d], &expenses, &[]).await.unwrap();
    assert_eq!(balances.iter().find(|x| x.user_id == a).unwrap().net_amount, 300);
    assert_eq!(balances.iter().find(|x| x.user_id == b).unwrap().net_amount, 0);
    assert_eq!(balances.iter().find(|x| x.user_id == c).unwrap().net_amount, -200);
    assert_eq!(balances.iter().find(|x| x.user_id == d).unwrap().net_amount, -100);

    let suggestions = DebtEngine::simplify_debts(&balances).await.unwrap();
    assert_eq!(suggestions.len(), 2);
    let c_to_a = suggestions.iter().find(|s| s.from_user_id == c && s.to_user_id == a).unwrap();
    assert_eq!(c_to_a.amount, 200);
    let d_to_a = suggestions.iter().find(|s| s.from_user_id == d && s.to_user_id == a).unwrap();
    assert_eq!(d_to_a.amount, 100);
}
