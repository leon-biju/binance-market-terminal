use std::collections::VecDeque;
use std::time;
use rust_decimal::Decimal;

use crate::{binance::types::Trade, book::{orderbook::OrderBook, scaler::Scaler}};

pub struct MarketMetrics {
    // Orderbook metrics
    pub spread: Option<Decimal>,
    pub mid_price: Option<Decimal>,
    pub imbalance_ratio: Option<Decimal>,

    // Trade metrics
    pub last_price: Option<Decimal>,
    pub last_qty: Option<Decimal>,
    pub volume_1m: Decimal,
    pub trade_count_1m: u64,
    pub buy_ratio_1m: Option<f64>,
    pub vwap_1m: Option<Decimal>,
    pub total_trades: u64,

    // System metrics
    pub updates_per_second: f64,

    // latency tracking
    pub orderbook_lag_ms: Option<u64>,
    pub orderbook_network_lag_ms: Option<u64>,
    pub trade_lag_ms: Option<u64>,
    pub trade_network_lag_ms: Option<u64>,
}


impl MarketMetrics {
    // Compute only orderbook-related metrics
    pub fn compute_book_metrics(
        &mut self,
        book: &OrderBook,
        scaler: &Scaler,
        event_time: u64,
        received_at: std::time::Instant,
    ) {
        self.spread = book.spread()
            .map(|spread_ticks| scaler.ticks_to_price(spread_ticks));

        self.mid_price = book.mid_price()
            .map(|price| scaler.ticks_to_price(price));

        // magic 10 value here todo: replace this
        self.imbalance_ratio = book.imbalance_ratio(10).map(Decimal::from_f64_retain).flatten();
        
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let orderbook_lag_ms = now_ms.saturating_sub(event_time);
        self.orderbook_lag_ms = Some(orderbook_lag_ms);
        let now_instant = std::time::Instant::now();
        
        //oh please make this nicer i beg. so messy
        self.orderbook_network_lag_ms = Some(
            orderbook_lag_ms -
            now_instant
                .duration_since(received_at)
                .as_millis() as u64
        );
    }

    pub fn compute_trade_metrics(
        &mut self,
        recent_trades: &VecDeque<Trade>,
        total_trades: u64,
        event_time: u64,
        received_at: std::time::Instant,
    ) {
        let last_trade = recent_trades.back();
        self.last_price = last_trade.map(|t| t.price);
        self.last_qty = last_trade.map(|t| t.quantity);
        
        self.trade_count_1m = recent_trades.iter().count() as u64;

        self.volume_1m = recent_trades.iter()
            .map(|t| t.quantity)
            .sum();

        let volume_price_sum_1m: Decimal = recent_trades.iter()
            .map(|t| t.quantity * t.price)
            .sum();

        let buy_count_1m = recent_trades.iter()
            .filter(|t| !t.is_buyer_maker)
            .count() as u64;
        
        self.buy_ratio_1m = if self.trade_count_1m > 0 {
            Some(buy_count_1m as f64 / self.trade_count_1m as f64)
        } else { 
            None
        };
        
        self.vwap_1m = if self.volume_1m > Decimal::ZERO {
            Some(volume_price_sum_1m / self.volume_1m)
        } else {
            None
        };

        self.total_trades = total_trades;

        let now_ms = time::SystemTime::now()
            .duration_since(time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let trade_lag_ms = now_ms.saturating_sub(event_time);
        self.trade_lag_ms = Some(trade_lag_ms);

        let now_instant = time::Instant::now();

        self.trade_network_lag_ms = Some(
            trade_lag_ms -
            now_instant
                .duration_since(received_at)
                .as_millis() as u64
        );
    }

    pub fn update_performance_metrics(&mut self, updates_per_second: f64) {
        self.updates_per_second = updates_per_second;
    }
}