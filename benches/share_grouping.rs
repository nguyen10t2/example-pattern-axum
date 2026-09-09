//! So sánh gom shares theo expense: bản `filter` O(E×S) cũ vs helper `HashMap` O(E+S).
//!
//! Chạy: `cargo bench --bench share_grouping`. Bản cũ copy logic đã thay thế để
//! đối chiếu công bằng trên cùng input.

use chrono::Utc;
use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use dsa::domain::{
    Currency, SplitType,
    expenses::{
        entity::{ExpenseEntity, ExpenseShareEntity, ExpenseWithSharesEntity},
        pg::assemble_expenses_with_shares,
    },
};
use uuid::Uuid;

fn fixture(expense_count: usize, shares_per_expense: usize) -> (Vec<ExpenseEntity>, Vec<ExpenseShareEntity>) {
    let expenses: Vec<ExpenseEntity> = (0..expense_count)
        .map(|_| ExpenseEntity {
            id: Uuid::now_v7(),
            group_id: Uuid::now_v7(),
            created_by_id: Uuid::now_v7(),
            payer_id: Uuid::now_v7(),
            amount: 100,
            currency: Currency::VND,
            description: String::new(),
            split_type: SplitType::EQUAL,
            expense_date: Utc::now(),
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .collect();
    let shares: Vec<ExpenseShareEntity> = expenses
        .iter()
        .flat_map(|e| {
            (0..shares_per_expense).map(|_| ExpenseShareEntity {
                id: Uuid::now_v7(),
                expense_id: e.id,
                user_id: Uuid::now_v7(),
                share_amount: 10,
                share_percentage: None,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            })
        })
        .collect();
    (expenses, shares)
}

fn baseline_filter_join(
    expenses: Vec<ExpenseEntity>,
    all_shares: &[ExpenseShareEntity],
) -> Vec<ExpenseWithSharesEntity> {
    let mut results = Vec::with_capacity(expenses.len());
    for expense in expenses {
        let shares = all_shares.iter().filter(|s| s.expense_id == expense.id).cloned().collect();
        results.push(ExpenseWithSharesEntity { expense, shares });
    }
    results
}

fn bench_grouping(c: &mut Criterion) {
    let mut group = c.benchmark_group("share_grouping");
    for (expenses, shares_each) in [(20, 5), (100, 10)] {
        let param = format!("{expenses}e_{shares_each}s");
        group.bench_with_input(BenchmarkId::from_parameter(&param), &(expenses, shares_each), |b, &(e, s)| {
            b.iter_batched(
                || fixture(e, s),
                |(expenses, shares)| assemble_expenses_with_shares(expenses, shares),
                BatchSize::SmallInput,
            );
        });
        group.bench_with_input(BenchmarkId::new("baseline_filter", &param), &(expenses, shares_each), |b, &(e, s)| {
            b.iter_batched(
                || fixture(e, s),
                |(expenses, shares)| baseline_filter_join(expenses, &shares),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_grouping);
criterion_main!(benches);
