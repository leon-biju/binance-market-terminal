use std::{collections::VecDeque};

use arc_swap::ArcSwap;
use rust_decimal::Decimal;
use crate::{binance::types::Trade, book::{orderbook::OrderBook, scaler::Scaler}, engine::metrics::MarketMetrics};

// This struct is all the tui will see
// Anything the tui needs goes here

pub struct MarketState {
    // atomic pointer to the current, immutable OrderBook snapshot.
    // TUI can read this instantly and lock-free.
    pub current_book: ArcSwap<OrderBook>, 
    pub is_syncing: tokio::sync::RwLock<bool>,
    pub metrics: ArcSwap<MarketMetrics>,
    pub recent_trades: ArcSwap<VecDeque<Trade>>,
    pub symbol: String,
    pub scaler: Scaler,
}

impl MarketState {
    pub fn new(initial_book: OrderBook, symbol: String, scaler: Scaler) -> Self {
        let initial_metrics = MarketMetrics {
            spread: None,
            mid_price: None,
            imbalance_ratio: None,
            last_price: None,
            last_qty: None,
            volume_1m: rust_decimal::Decimal::ZERO,
            trade_count_1m: 0,
            vwap_1m: None,
            updates_per_second: 0.0,
            orderbook_lag_ms: None,
            trade_lag_ms: None,
        };

        MarketState {
            current_book: ArcSwap::from_pointee(initial_book),
            is_syncing: tokio::sync::RwLock::new(true), // Start in syncing state
            metrics: ArcSwap::from_pointee(initial_metrics),
            recent_trades: ArcSwap::from_pointee(VecDeque::with_capacity(super::engine::MAX_TRADES)),
            symbol,
            scaler,
        }
    }

    pub fn top_n_depth(&self, n: usize) -> (Vec<(Decimal, Decimal)>, Vec<(Decimal, Decimal)>) {
        let (bids, asks) = self.current_book.load().top_n_depth(n);

        let bids_decimal = bids.iter()
            .map(|(price, qty)| (self.scaler.ticks_to_price(*price), self.scaler.ticks_to_qty(*qty)))
            .collect();

        let asks_decimal = asks.iter()
            .map(|(price, qty)| (self.scaler.ticks_to_price(*price), self.scaler.ticks_to_qty(*qty)))
            .collect();

        (bids_decimal, asks_decimal)
    }
}