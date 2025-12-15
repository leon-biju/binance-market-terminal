use std::collections::VecDeque;

use rust_decimal::Decimal;

use crate::{binance::types::Trade, book::{orderbook::OrderBook, scaler::Scaler}};

pub struct MarketMetrics {
    // Orderbook metrics
    pub best_bid: Option<Decimal>,
    pub best_ask: Option<Decimal>,
    pub spread: Option<Decimal>,
    pub mid_price: Option<Decimal>,
    pub imbalance_ratio: Option<Decimal>,

    // Trade metrics (todo)
    pub last_price: Option<Decimal>,
    pub last_qty: Option<Decimal>,
    pub volume_1m: Decimal,
    pub trade_count_1m: u64,
    pub vwap_1m: Option<Decimal>,

    // System metrics
    pub last_update_time: std::time::Instant,
    pub updates_per_second: f64,
    pub is_syncing: bool,
}


impl MarketMetrics {
    pub fn compute(book: &OrderBook, recent_trades: &VecDeque<Trade>, scaler: &Scaler, is_syncing: bool) -> Self {
        let best_bid: Option<Decimal> = book.best_bid()
            .map(|(price, _)| scaler.ticks_to_price(*price));
        let best_ask: Option<Decimal> = book.best_ask()
            .map(|(price, _)| scaler.ticks_to_price(*price));

        let spread = book.spread()
            .map(|spread_ticks| scaler.ticks_to_price(spread_ticks));

        let mid_price = book.mid_price()
            .map(|price| scaler.ticks_to_price(price));

        // magic 10 value here todo: replace this
        let imbalance_ratio = book.imbalance_ratio(10).map(Decimal::from_f64_retain).flatten();
        
        //holy temp values btw
        let last_price = None;
        let last_qty = None;
        let volume_1m = Decimal::ZERO;
        let trade_count_1m = 0;
        let vwap_1m = None;

        
        
        Self { 
            best_bid,
            best_ask,
            spread,
            mid_price,
            imbalance_ratio,
            last_price,
            last_qty,
            volume_1m,
            trade_count_1m,
            vwap_1m,
            last_update_time: std::time::Instant::now(),
            updates_per_second: 0.0, //todo: track in engine,
            is_syncing
        }

    }
}