//! Baseline bench cho split validation: equal/exact/percentage, cả input hợp lệ
//! và không hợp lệ.
//!
//! Chạy: `cargo bench --bench split_validation`.

use criterion::{Criterion, criterion_group, criterion_main};
use dsa::domain::{
    SplitType,
    expenses::strategy::{SplitContext, SplitShareInput, SplitStrategyFactory},
};
use uuid::Uuid;

fn shares(amounts: &[i64]) -> Vec<SplitShareInput> {
    amounts
        .iter()
        .map(|a| SplitShareInput { user_id: Uuid::now_v7(), share_amount: *a, share_percentage: None })
        .collect()
}

fn bench_valid(c: &mut Criterion) {
    let mut group = c.benchmark_group("split_validate_ok");
    let equal = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);
    let exact = SplitStrategyFactory::get_strategy(&SplitType::EXACT);
    let percentage = SplitStrategyFactory::get_strategy(&SplitType::PERCENTAGE);

    group.bench_function("equal_3", |b| {
        let ctx = SplitContext { total_amount: 100, shares: shares(&[33, 33, 34]) };
        b.iter(|| equal.validate(&ctx));
    });
    group.bench_function("exact_2", |b| {
        let ctx = SplitContext { total_amount: 500, shares: shares(&[200, 300]) };
        b.iter(|| exact.validate(&ctx));
    });
    group.bench_function("percentage_2", |b| {
        let mut ctx_shares = shares(&[500, 500]);
        ctx_shares[0].share_percentage = Some(5000);
        ctx_shares[1].share_percentage = Some(5000);
        let ctx = SplitContext { total_amount: 1000, shares: ctx_shares };
        b.iter(|| percentage.validate(&ctx));
    });
    group.finish();
}

fn bench_invalid(c: &mut Criterion) {
    let mut group = c.benchmark_group("split_validate_err");
    let equal = SplitStrategyFactory::get_strategy(&SplitType::EQUAL);

    group.bench_function("equal_uneven_3", |b| {
        let ctx = SplitContext { total_amount: 100, shares: shares(&[90, 5, 5]) };
        b.iter(|| equal.validate(&ctx));
    });
    group.finish();
}

criterion_group!(benches, bench_valid, bench_invalid);
criterion_main!(benches);
