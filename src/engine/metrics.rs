use rust_decimal::Decimal;
use std::time;

use crate::{
    binance::types::Trade,
    book::{orderbook::OrderBook, scaler::Scaler},
};

// running totals for the 1-minute trade window.
// Updated incrementally as trades get added/pruned

#[derive(Clone, Debug, Default)]
pub struct RunningTradeTotals {
    pub total_volume: Decimal,
    pub total_notional: Decimal,
    pub trade_count: u64,
    pub buy_count: u64,
}

impl RunningTradeTotals {
    /// use when new trade enters window.
    #[inline]
    pub fn add(&mut self, trade: &Trade) {
        self.total_volume += trade.quantity;
        self.total_notional += trade.quantity * trade.price;
        self.trade_count += 1;
        if !trade.is_buyer_maker {
            self.buy_count += 1;
        }
    }

    // use when a trade ages out of the 1-minute window.
    #[inline]
    pub fn remove(&mut self, trade: &Trade) {
        self.total_volume -= trade.quantity;
        self.total_notional -= trade.quantity * trade.price;
        self.trade_count -= 1;
        if !trade.is_buyer_maker {
            self.buy_count -= 1;
        }
    }
}

fn compute_latencies(event_time: u64, received_at: time::Instant) -> (u64, u64) {
    let now_ms = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let total_lag_ms = now_ms.saturating_sub(event_time);
    let processing_ms = received_at.elapsed().as_millis() as u64;
    let network_lag_ms = total_lag_ms.saturating_sub(processing_ms);

    (total_lag_ms, network_lag_ms)
}

#[derive(Clone)]
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

    imbalance_depth_levels: usize,
}

impl MarketMetrics {
    pub fn new(imbalance_depth_levels: usize) -> Self {
        Self {
            imbalance_depth_levels,
            ..Default::default()
        }
    }
    // Compute only orderbook-related metrics
    #[tracing::instrument(level = "trace", skip_all, name = "metrics::compute_book_metrics")]
    pub fn compute_book_metrics(
        &mut self,
        book: &OrderBook,
        scaler: &Scaler,
        event_time: u64,
        received_at: std::time::Instant,
    ) {
        self.spread = book
            .spread()
            .map(|spread_ticks| scaler.ticks_to_price(spread_ticks));

        self.mid_price = book.mid_price().map(|price| scaler.ticks_to_price(price));

        self.imbalance_ratio = book
            .imbalance_ratio(self.imbalance_depth_levels)
            .and_then(Decimal::from_f64_retain);

        let (total_lag, network_lag) = compute_latencies(event_time, received_at);
        self.orderbook_lag_ms = Some(total_lag);
        self.orderbook_network_lag_ms = Some(network_lag);
    }

    #[tracing::instrument(level = "trace", skip_all, name = "metrics::compute_trade_metrics")]
    pub fn compute_trade_metrics(
        &mut self,
        running_total: &RunningTradeTotals,
        last_trade: Option<&Trade>,
        total_trades: u64,
        event_time: u64,
        received_at: std::time::Instant,
    ) {
        self.last_price = last_trade.map(|t| t.price);
        self.last_qty = last_trade.map(|t| t.quantity);

        self.trade_count_1m = running_total.trade_count;
        self.volume_1m = running_total.total_volume;

        self.buy_ratio_1m = if running_total.trade_count > 0 {
            Some(running_total.buy_count as f64 / running_total.trade_count as f64)
        } else {
            None
        };

        self.vwap_1m = if running_total.total_volume > Decimal::ZERO {
            Some(running_total.total_notional / running_total.total_volume)
        } else {
            None
        };

        self.total_trades = total_trades;

        let (total_lag, network_lag) = compute_latencies(event_time, received_at);
        self.trade_lag_ms = Some(total_lag);
        self.trade_network_lag_ms = Some(network_lag);
    }

    pub fn update_performance_metrics(&mut self, updates_per_second: f64) {
        self.updates_per_second = updates_per_second;
    }
}

impl Default for MarketMetrics {
    fn default() -> Self {
        MarketMetrics {
            spread: None,
            mid_price: None,
            imbalance_ratio: None,
            last_price: None,
            last_qty: None,
            volume_1m: rust_decimal::Decimal::ZERO,
            trade_count_1m: 0,
            total_trades: 0,
            buy_ratio_1m: None,
            vwap_1m: None,
            updates_per_second: 0.0,
            orderbook_lag_ms: None,
            orderbook_network_lag_ms: None,
            trade_lag_ms: None,
            trade_network_lag_ms: None,
            imbalance_depth_levels: 10,
        }
    }
}
