use anyhow::Result;
use futures_util::StreamExt;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{self, Duration};
use tokio::sync::mpsc;

use rust_decimal::Decimal;

use crate::binance::types::{
    DepthSnapshot, MarketEvent, ReceivedDepthUpdate, ReceivedTrade, SignificanceReason,
    SignificantTrade, Trade,
};
use crate::binance::{snapshot, stream};
use crate::book::orderbook::OrderBook;
use crate::book::scaler::Scaler;
use crate::book::sync::{SyncOutcome, SyncState};
use crate::config;
use crate::engine::metrics::{MarketMetrics, RunningTradeTotals};
use crate::engine::state::{MarketSnapshot, MarketState};

pub enum EngineCommand {
    NewSnapshot(DepthSnapshot),
    RequestSnapshot,
    Shutdown,
}

pub struct MarketDataEngine {
    state: Arc<MarketState>,
    metrics: MarketMetrics,
    recent_trades: VecDeque<Trade>,
    significant_trades: VecDeque<SignificantTrade>,

    conf: Arc<config::Config>,

    sync_state: SyncState,
    book: OrderBook,
    scaler: Scaler,
    symbol: String,

    is_syncing: bool,

    command_tx: mpsc::Sender<EngineCommand>,
    command_rx: mpsc::Receiver<EngineCommand>,

    running_trade_totals: RunningTradeTotals,

    update_counter: u64,
    last_rate_calc_time: std::time::Instant,
    last_publish_time: std::time::Instant,
    updates_per_second: f64,
    total_trades: u64,
}

impl MarketDataEngine {
    pub fn prepare(
        symbol: String,
        initial_snapshot: DepthSnapshot,
        scaler: Scaler,
        conf: Arc<config::Config>,
    ) -> Result<(Self, mpsc::Sender<EngineCommand>, Arc<MarketState>)> {
        let (command_tx, command_rx) = mpsc::channel(32);

        let mut sync_state = SyncState::default();
        sync_state.set_last_update_id(initial_snapshot.last_update_id);
        let book = OrderBook::from_snapshot(initial_snapshot.clone(), &scaler)?;
        let (top_n_bids, top_n_asks) = book.top_n_depth(conf.orderbook_depth_display_count);
        let state = Arc::new(MarketState::new(
            top_n_bids,
            top_n_asks,
            symbol.clone(),
            scaler.clone(),
        ));

        let engine = MarketDataEngine {
            state: state.clone(),
            metrics: MarketMetrics::new(conf.orderbook_imbalance_depth_levels),
            recent_trades: VecDeque::with_capacity(conf.recent_trades_starting_capacity),
            significant_trades: VecDeque::with_capacity(conf.significant_trades_display_count),

            conf,

            sync_state,
            book,
            scaler,
            symbol,

            is_syncing: true,

            command_tx: command_tx.clone(),
            command_rx,

            running_trade_totals: RunningTradeTotals::default(),

            update_counter: 0,
            last_rate_calc_time: std::time::Instant::now(),
            last_publish_time: std::time::Instant::now(),
            updates_per_second: 0.0,
            total_trades: 0,
        };

        Ok((engine, command_tx, state))
    }

    #[tracing::instrument(level = "trace", skip_all, name = "engine::publish_snapshot")]
    fn publish_snapshot(&mut self, force: bool) {
        // TODO: make time configurable
        // throttle snapshot updates to at most once every 100ms, unless it's a forced update (e.g. after applying a significant trade or receiving a new snapshot)
        let now = std::time::Instant::now();
        if !force && now.duration_since(self.last_publish_time).as_millis() < 100 {
            return;
        }


        let (top_n_bids, top_n_asks) = self.book.top_n_depth(self.conf.orderbook_depth_display_count);
        let snapshot = MarketSnapshot {
            top_n_bids,
            top_n_asks,
            metrics: self.metrics.clone(),
            recent_trades: self.recent_trades.clone(),
            significant_trades: self.significant_trades.clone(),
            is_syncing: self.is_syncing,
        };

        self.state.snapshot.store(Arc::new(snapshot));

        self.last_publish_time = std::time::Instant::now();
    }

    fn spawn_snapshot_fetch(&self) {
        let symbol = self.symbol.clone();
        let tx = self.command_tx.clone();
        let conf = self.conf.clone();

        tokio::spawn(async move {
            match snapshot::fetch_snapshot(&symbol, conf.orderbook_initial_snapshot_depth).await {
                Ok(snapshot) => {
                    if tx.send(EngineCommand::NewSnapshot(snapshot)).await.is_err() {
                        tracing::error!("Failed to send snapshot to engine - channel closed")
                    };
                }
                Err(e) => {
                    tracing::error!("Fatal error, failed to fetch snapshot: {}", e);
                }
            }
        });
    }

    fn update_rate_counter(&mut self) {
        self.update_counter += 1;
        let now = time::Instant::now();
        let elapsed_secs = now.duration_since(self.last_rate_calc_time).as_secs_f64();

        if elapsed_secs >= 1.0 {
            self.updates_per_second = self.update_counter as f64 / elapsed_secs;
            self.last_rate_calc_time = now;
            self.update_counter = 0;
        }
    }

    fn detect_significant_trade(&mut self, trade: &Trade, event_time: u64) {
        let total_volume = self.running_trade_totals.total_volume;
        let enough_trades = self.running_trade_totals.trade_count >= self.conf.min_trades_for_significance as u64;

        if !enough_trades || total_volume.is_zero() {
            return;
        }

        let volume_ratio = trade.quantity / total_volume;
        if volume_ratio < self.conf.significant_trade_volume_pct {
            return;
        }

        let notional_value = trade.price * trade.quantity;
        let volume_pct = volume_ratio * Decimal::ONE_HUNDRED;
        let reason = SignificanceReason::HighVolumePercent(volume_pct);

        self.significant_trades
            .push_back(SignificantTrade::new(trade.clone(), notional_value, reason));

        // Prune significant trades older than retention window
        let cutoff = event_time.saturating_sub(self.conf.significant_trades_retention_secs * 1000);
        while self
            .significant_trades
            .front()
            .is_some_and(|t| t.trade.trade_time < cutoff)
        {
            self.significant_trades.pop_front();
        }

        // Keep only the most recent N significant trades for display
        while self.significant_trades.len() > self.conf.significant_trades_display_count {
            self.significant_trades.pop_front();
        }
    }

    #[tracing::instrument(level = "trace", skip_all, name = "engine::handle_ws_trade")]
    fn handle_ws_trade(&mut self, received: ReceivedTrade) {
        self.total_trades += 1;
        self.update_rate_counter();

        let event_time = received.trade.trade_time;
        let received_at = received.received_at;
        let cutoff_time = event_time.saturating_sub(60_000);

        self.running_trade_totals.add(&received.trade);
        self.recent_trades.push_back(received.trade.clone());

        while let Some(oldest) = self.recent_trades.front() {
            if oldest.trade_time < cutoff_time {
                self.running_trade_totals.remove(oldest);
                self.recent_trades.pop_front();
            } else {
                break;
            }

            // Important!!: might seem like you would want to prune trades that won't be displayed, but we need the full 1-minute window to accurately calculate significance of other trades and also metrics!
        }

        self.detect_significant_trade(&received.trade, event_time);

        self.metrics.compute_trade_metrics(
            &self.running_trade_totals,
            self.recent_trades.back(),
            self.total_trades,
            event_time,
            received_at,
        );

        self.metrics
            .update_performance_metrics(self.updates_per_second);

        self.publish_snapshot(false);
    }

    #[tracing::instrument(level = "trace", skip_all, name = "engine::handle_ws_depth_update")]
    async fn handle_ws_depth_update(&mut self, received: ReceivedDepthUpdate) -> Result<()> {
        self.update_rate_counter();
        let event_time = received.update.event_time;
        let received_at = received.received_at;

        match self.sync_state.process_delta(received.update) {
            SyncOutcome::Updates(updates) => {
                for update in updates {
                    self.book.apply_update(&update, &self.scaler)?;
                }
                self.is_syncing = false;
            }
            SyncOutcome::GapBetweenUpdates => {
                self.command_tx.send(EngineCommand::RequestSnapshot).await?;
                self.sync_state = SyncState::default();
                self.is_syncing = true;
            }
            SyncOutcome::NoUpdates => {}
        }

        self.metrics
            .compute_book_metrics(&self.book, &self.scaler, event_time, received_at);

        self.publish_snapshot(false);

        Ok(())
    }

    async fn handle_command(&mut self, cmd: EngineCommand) -> Result<bool> {
        match cmd {
            EngineCommand::NewSnapshot(snapshot) => {
                tracing::info!(
                    "Received new snapshot, lastUpdateId: {}",
                    snapshot.last_update_id
                );

                self.sync_state.set_last_update_id(snapshot.last_update_id);
                self.book = OrderBook::from_snapshot(snapshot, &self.scaler)?;
                self.publish_snapshot(true);

                self.is_syncing = false;
                Ok(false)
            }
            EngineCommand::RequestSnapshot => {
                tracing::warn!("Gap detected, requesting new snapshot...");
                self.spawn_snapshot_fetch();
                Ok(false)
            }
            EngineCommand::Shutdown => {
                tracing::info!("Shutting down engine...");
                Ok(true)
            }
        }
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let backoff_ms = self.conf.initial_backoff_ms * 2u64.saturating_pow(attempt);
        Duration::from_millis(backoff_ms.min(self.conf.max_backoff_ms))
    }

    async fn connect_with_retry<T, F, Fut>(&self, connect_fn: F, stream_name: &str) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut attempt = 0;

        loop {
            match connect_fn().await {
                Ok(stream) => {
                    if attempt > 0 {
                        tracing::info!("{} reconnected after {} attempts", stream_name, attempt);
                    }
                    return Ok(stream);
                }
                Err(e) => {
                    attempt += 1;
                    if attempt >= self.conf.max_reconnect_attempts {
                        tracing::error!(
                            "{} failed to reconnect after {} attempts: {}",
                            stream_name,
                            attempt,
                            e
                        );
                        return Err(e);
                    }

                    let backoff = self.calculate_backoff(attempt);
                    tracing::warn!(
                        "{} connection failed (attempt {}/{}): {}. Retrying in {:?}",
                        stream_name,
                        attempt,
                        self.conf.max_reconnect_attempts,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let symbol = self.symbol.clone();

        tracing::info!("Engine running for symbol: {}", self.symbol);

        let mut market_stream = Box::pin(
            self.connect_with_retry(|| stream::connect_market_stream(&symbol), "Market stream")
                .await?,
        );

        let stream_timeout = Duration::from_millis(self.conf.message_timeout_ms);
        let mut last_message_time = tokio::time::Instant::now();

        loop {
            tokio::select! {
                Some(cmd) = self.command_rx.recv() => {
                    let should_shutdown = self.handle_command(cmd).await?;
                    if should_shutdown {
                        break;
                    }
                }

                Some(result) = market_stream.next() => {
                    last_message_time = tokio::time::Instant::now();

                    match result {
                        Ok(event) => {
                            match event {
                                MarketEvent::Trade(trade) => self.handle_ws_trade(trade),
                                MarketEvent::Depth(update) => self.handle_ws_depth_update(update).await?,
                            }
                        }
                        Err(e) => {
                            tracing::error!("Market websocket stream error: {}", e);
                            self.is_syncing = true;
                            self.publish_snapshot(true);

                            // reset sync state - we need a fresh snapshot after reconnect
                            self.sync_state = SyncState::default();
                            self.spawn_snapshot_fetch();

                            market_stream = Box::pin(self.connect_with_retry(
                                || stream::connect_market_stream(&symbol),
                                "Market stream",
                            ).await?);
                        }
                    }
                }

                _ = tokio::time::sleep_until(last_message_time + stream_timeout) => {
                    tracing::warn!("No message received for {:?}, attempting reconnect...", stream_timeout);
                    self.is_syncing = true;
                    self.publish_snapshot(true);

                    self.sync_state = SyncState::default();
                    self.spawn_snapshot_fetch();

                    market_stream = Box::pin(self.connect_with_retry(
                        || stream::connect_market_stream(&symbol),
                        "Market stream",
                    ).await?);

                    last_message_time = tokio::time::Instant::now();
                }

                else => break
            }
        }

        Ok(())
    }
}
