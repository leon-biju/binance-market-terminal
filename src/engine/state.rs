use std::{collections::VecDeque};

use arc_swap::ArcSwap;
use crate::{binance::types::Trade, book::orderbook::OrderBook, engine::metrics::MarketMetrics};

// This struct is all the tui will see
// Anything the tui needs goes here

pub struct MarketState {
    // atomic pointer to the current, immutable OrderBook snapshot.
    // TUI can read this instantly and lock-free.
    pub current_book: ArcSwap<OrderBook>, 
    pub is_syncing: tokio::sync::RwLock<bool>,
    pub metrics: ArcSwap<MarketMetrics>,
    pub recent_trades: ArcSwap<VecDeque<Trade>>,

}

impl MarketState {
    pub fn new(initial_book: OrderBook) -> Self {
        let initial_metrics = MarketMetrics {
            best_bid: None,
            best_ask: None,
            spread: None,
            mid_price: None,
            imbalance_ratio: None,
            last_price: None,
            last_qty: None,
            volume_1m: rust_decimal::Decimal::ZERO,
            trade_count_1m: 0,
            vwap_1m: None,
            updates_per_second: 0.0,
        };

        MarketState {
            current_book: ArcSwap::from_pointee(initial_book),
            is_syncing: tokio::sync::RwLock::new(true), // Start in syncing state
            metrics: ArcSwap::from_pointee(initial_metrics),
            recent_trades: ArcSwap::from_pointee(VecDeque::with_capacity(super::engine::MAX_TRADES))
        }
    }
}