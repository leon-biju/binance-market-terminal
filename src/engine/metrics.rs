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
    pub updates_per_second: f64,

    // latency tracking
    pub orderbook_lag_ms: Option<u64>,
    pub trade_lag_ms: Option<u64>
}


impl MarketMetrics {
    pub fn compute(
        book: &OrderBook,
        recent_trades: &VecDeque<Trade>,
        scaler: &Scaler,
        updates_per_second: f64,
        last_update_event_time: Option<u64>,
    ) -> Self {

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
        
        // Compute trade metrics from recent_trades
        let last_trade = recent_trades.back();
        let last_price = last_trade.map(|t| t.price);
        let last_qty = last_trade.map(|t| t.quantity);
        
        // Calculate metrics for last 1 minute
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        let one_minute_ago = now_ms - 60_000; // 60 seconds in milliseconds
        
        let mut volume_1m = Decimal::ZERO;
        let mut trade_count_1m = 0;
        let mut volume_price_sum = Decimal::ZERO;
        
        for trade in recent_trades.iter().rev() {
            if trade.event_time < one_minute_ago {
                break; // trades are ordered chronologically
            }
            volume_1m += trade.quantity;
            volume_price_sum += trade.quantity * trade.price;
            trade_count_1m += 1;
        }
        
        let vwap_1m = if volume_1m > Decimal::ZERO {
            Some(volume_price_sum / volume_1m)
        } else {
            None
        };

        
        // calculate lag
        let orderbook_lag_ms = last_update_event_time
            .map(|evt_time| now_ms.saturating_sub(evt_time));
        
        let trade_lag_ms = recent_trades.back()
            .map(|trade| now_ms.saturating_sub(trade.event_time));
        
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
            updates_per_second,
            orderbook_lag_ms,
            trade_lag_ms,
        }

    }
}