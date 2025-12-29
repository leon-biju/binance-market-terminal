use std::{collections::VecDeque};
use std::sync::Arc;
use arc_swap::ArcSwap;
use rust_decimal::Decimal;
use crate::{binance::types::Trade, book::{orderbook::OrderBook, scaler::Scaler}, engine::metrics::MarketMetrics};

#[derive(Clone)]
pub struct MarketSnapshot {
    pub book: OrderBook,
    pub metrics: MarketMetrics,
    pub recent_trades: VecDeque<Trade>,
    pub is_syncing: bool
}

impl MarketSnapshot {
    pub fn top_n_depth(&self, n: usize, scaler: &Scaler) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let (bids, asks) = self. book.top_n_depth(n);

        let bids_decimal = bids.iter()
            .map(|(price, qty)| (scaler.ticks_to_price(*price), scaler.ticks_to_qty(*qty)))
            .collect();

        let asks_decimal = asks.iter()
            .map(|(price, qty)| (scaler.ticks_to_price(*price), scaler.ticks_to_qty(*qty)))
            .collect();

        (bids_decimal, asks_decimal)
    }
}

pub struct MarketState {
    // atomic pointer to the current, immutable OrderBook snapshot.
    // TUI can read this instantly and lock-free.
    pub snapshot: ArcSwap<MarketSnapshot>,
    pub symbol: String,
    pub scaler: Scaler,
}

impl MarketState {
    pub fn new(initial_book: OrderBook, symbol: String, scaler: Scaler) -> Self {
        let initial_snapshot = MarketSnapshot {
            book: initial_book,
            metrics: MarketMetrics::default(),
            recent_trades: VecDeque::new(),
            is_syncing: true,
        };

        MarketState {
            snapshot: ArcSwap::from_pointee(initial_snapshot),
            symbol,
            scaler,
        }
    }

    pub fn load(&self) -> Arc<MarketSnapshot>{
        self.snapshot.load_full()
    }
    
}