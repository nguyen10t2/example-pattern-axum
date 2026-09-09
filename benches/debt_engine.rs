//! Baseline bench cho debt engine: số dư + gợi ý trả nợ với nhóm n thành viên.
//!
//! Chạy: `cargo bench --bench debt_engine`. Mỗi member trả 1 expense chia đều
//! cho cả nhóm (dữ liệu deterministic, dựng ngoài vòng đo).

use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use dsa::domain::debt_engine::{DebtEngine, ExpenseForEngine, SettlementForEngine, ShareForEngine, UserBalance};
use uuid::Uuid;

fn fixture(member_count: usize) -> (Vec<Uuid>, Vec<ExpenseForEngine>, Vec<SettlementForEngine>) {
    let users: Vec<Uuid> = (0..member_count).map(|_| Uuid::now_v7()).collect();
    let per_share = 10_000_i64;
    let total = per_share * i64::try_from(member_count).unwrap();
    let mut expenses: Vec<ExpenseForEngine> = users
        .iter()
        .map(|payer| ExpenseForEngine {
            payer_id: *payer,
            amount: total,
            shares: users.iter().map(|u| ShareForEngine { user_id: *u, amount: per_share }).collect(),
        })
        .collect();
    // Thêm 1 expense lớn do user đầu trả để số dư khác 0 và nhánh greedy chạy thật.
    expenses.push(ExpenseForEngine {
        payer_id: users[0],
        amount: total * i64::try_from(member_count).unwrap(),
        shares: users.iter().map(|u| ShareForEngine { user_id: *u, amount: per_share }).collect(),
    });
    (users, expenses, Vec::new())
}

fn bench_net_balances(c: &mut Criterion) {
    let mut group = c.benchmark_group("net_balances");
    for n in [5, 20, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || fixture(n),
                |(users, expenses, settlements)| DebtEngine::calculate_net_balances(&users, &expenses, &settlements),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_simplify(c: &mut Criterion) {
    let mut group = c.benchmark_group("simplify_debts");
    for n in [5, 20, 100] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (users, expenses, settlements) = fixture(n);
                    DebtEngine::calculate_net_balances(&users, &expenses, &settlements)
                },
                |balances: Vec<UserBalance>| DebtEngine::simplify_debts(&balances),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_net_balances, bench_simplify);
criterion_main!(benches);
