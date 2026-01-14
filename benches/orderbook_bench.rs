use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use orderbook_engine::{
    binance::types::{DepthSnapshot, DepthUpdate},
    book::orderbook::OrderBook,
    book::scaler::Scaler,
};
use rust_decimal::Decimal;
use std::hint::black_box;
use std::str::FromStr;

const SNAPSHOT_LEVELS: usize = 10000;
const UPDATES_PER_BATCH: usize = 100;
const LEVELS_PER_UPDATE: usize = 100;

fn bench_from_snapshot(c: &mut Criterion) {
    // simulate a snapshot
    let snapshot = DepthSnapshot::_fake_snapshot(SNAPSHOT_LEVELS);
    let scaler = Scaler::new(
        Decimal::from_str("0.01").unwrap(),
        Decimal::from_str("0.01").unwrap(),
    );

    c.bench_function(&format!("from_snapshot_{}", SNAPSHOT_LEVELS), |b| {
        b.iter(|| {
            let _book = black_box(OrderBook::from_snapshot(
                black_box(snapshot.clone()),
                &scaler,
            ));
        })
    });
}

fn bench_apply_updates(c: &mut Criterion) {
    let snapshot = DepthSnapshot::_fake_snapshot(SNAPSHOT_LEVELS);
    let scaler = Scaler::new(
        Decimal::from_str("0.01").unwrap(),
        Decimal::from_str("0.01").unwrap(),
    );

    let updates: Vec<DepthUpdate> = (0..UPDATES_PER_BATCH)
        .map(|i| DepthUpdate::_fake_update(i as u64, LEVELS_PER_UPDATE))
        .collect();

    c.bench_function(
        &format!("apply_updates_{}x{}", UPDATES_PER_BATCH, LEVELS_PER_UPDATE),
        |b| {
            b.iter_batched_ref(
                || OrderBook::from_snapshot(snapshot.clone(), &scaler).unwrap(),
                |book| {
                    for up in &updates {
                        let _ = book.apply_update(black_box(up), &scaler);
                    }
                },
                BatchSize::SmallInput,
            )
        },
    );
}

fn bench_query_functions(c: &mut Criterion) {
    let snapshot = DepthSnapshot::_fake_snapshot(SNAPSHOT_LEVELS);
    let scaler = Scaler::new(
        Decimal::from_str("0.01").unwrap(),
        Decimal::from_str("0.01").unwrap(),
    );
    let book = OrderBook::from_snapshot(snapshot, &scaler).unwrap();

    c.bench_function("query_best_spread_mid", |b| {
        b.iter(|| {
            let _bid = black_box(book.best_bid());
            let _ask = black_box(book.best_ask());
            let _spread = black_box(book.spread());
            let _mid = black_box(book.mid_price());
        })
    });
}

fn bench_high_churn(c: &mut Criterion) {
    // Simulate high-frequency trading: many small updates
    let snapshot = DepthSnapshot::_fake_snapshot(SNAPSHOT_LEVELS);
    let scaler = Scaler::new(
        Decimal::from_str("0.01").unwrap(),
        Decimal::from_str("0.01").unwrap(),
    );

    let updates: Vec<DepthUpdate> = (0..1000)
        .map(|i| DepthUpdate::_fake_update(i as u64, 10)) // small updates
        .collect();

    c.bench_function("apply_updates_high_churn_1000x10", |b| {
        b.iter_batched_ref(
            || OrderBook::from_snapshot(snapshot.clone(), &scaler).unwrap(),
            |book| {
                for up in &updates {
                    let _ = book.apply_update(black_box(up), &scaler);
                }
            },
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_from_snapshot,
    bench_apply_updates,
    bench_query_functions,
    bench_high_churn
);
criterion_main!(benches);
