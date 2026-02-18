use crate::binance::types::SignificantTrade;
use crate::book::orderbook::RawDepthLevel;
use crate::{
    binance::types::Trade,
    book::{scaler::Scaler},
    engine::metrics::MarketMetrics,
};
use arc_swap::ArcSwap;
use rust_decimal::Decimal;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone)]
pub struct MarketSnapshot {
    pub top_n_bids: Vec<RawDepthLevel>,
    pub top_n_asks: Vec<RawDepthLevel>,
    //pub book: OrderBook,
    pub metrics: MarketMetrics,
    pub recent_trades: VecDeque<Trade>,
    pub significant_trades: VecDeque<SignificantTrade>,
    pub is_syncing: bool,
}

pub type DisplayDepthLevel = (Decimal, Decimal);

impl MarketSnapshot {
    pub fn top_n_depth(
        &self,
        scaler: &Scaler,
    ) -> (Vec<DisplayDepthLevel>, Vec<DisplayDepthLevel>) {

        let bids_decimal: Vec<DisplayDepthLevel> = self.top_n_bids
            .iter()
            .map(|(price, qty)| (scaler.ticks_to_price(*price), scaler.ticks_to_qty(*qty)))
            .collect();

        let asks_decimal: Vec<DisplayDepthLevel> = self.top_n_asks
            .iter()
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
    pub fn new(top_n_bids: Vec<RawDepthLevel>, top_n_asks: Vec<RawDepthLevel>, symbol: String, scaler: Scaler) -> Self {
        let initial_snapshot = MarketSnapshot {
            top_n_bids,
            top_n_asks,
            metrics: MarketMetrics::default(),
            recent_trades: VecDeque::new(),
            significant_trades: VecDeque::new(),
            is_syncing: true,
        };

        MarketState {
            snapshot: ArcSwap::from_pointee(initial_snapshot),
            symbol,
            scaler,
        }
    }

    pub fn load(&self) -> Arc<MarketSnapshot> {
        self.snapshot.load_full()
    }
}
