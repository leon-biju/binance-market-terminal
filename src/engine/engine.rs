use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{self, Duration};
use tokio::sync::mpsc;
use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::types::{DepthSnapshot, Trade, ReceivedTrade, ReceivedDepthUpdate};
use crate::binance::{snapshot, stream};
use crate::book::sync::{SyncState, SyncOutcome};
use crate::book::orderbook::OrderBook;
use crate::book::scaler::Scaler;
use crate::config;
use crate::engine::metrics::MarketMetrics;
use crate::engine::state::{MarketSnapshot, MarketState};

pub enum EngineCommand {
    NewSnapshot(DepthSnapshot),
    RequestSnapshot,
    Shutdown,
}

pub struct MarketDataEngine {
    pub state: Arc<MarketState>,
    
    
    sync_state: SyncState,
    book: OrderBook,
    scaler: Scaler,
    symbol: String,
    recent_trades: VecDeque<Trade>,
    metrics: MarketMetrics,
    is_syncing: bool,

    command_tx: mpsc::Sender<EngineCommand>,
    command_rx: mpsc::Receiver<EngineCommand>,

    update_counter : u64,
    last_rate_calc_time: std::time::Instant,
    updates_per_second: f64,
    total_trades: u64,

}

impl MarketDataEngine {
    pub fn new(
        symbol: String,
        initial_snapshot: DepthSnapshot,
        scaler: Scaler,
        conf: &config::Config
    ) -> (Self, mpsc::Sender<EngineCommand>, Arc<MarketState>) {
        let (command_tx, command_rx) = mpsc::channel(32);
        
        let mut sync_state = SyncState::new();
        sync_state.set_last_update_id(initial_snapshot.last_update_id);
        let book = OrderBook::from_snapshot(initial_snapshot.clone(), &scaler);
        let state = Arc::new(MarketState::new(book.clone(), symbol.clone(), scaler.clone()));
        
        let engine = MarketDataEngine {
            state: state.clone(),

            sync_state,
            book,
            scaler,
            symbol,
            recent_trades: VecDeque::with_capacity(conf.initial_starting_capacity),
            metrics: MarketMetrics::new(conf.imbalance_depth_levels),
            is_syncing: true,

            command_tx: command_tx.clone(),
            command_rx,

            update_counter: 0,
            last_rate_calc_time: std::time::Instant::now(),
            updates_per_second: 0.0,
            total_trades: 0,
        };
        
        (engine, command_tx, state) 
    }

    fn publish_snapshot(&self) {
        let snapshot = MarketSnapshot {
            book: self.book.clone(),
            metrics: self.metrics.clone(),
            recent_trades: self.recent_trades.clone(),
            is_syncing: self.is_syncing,
        };

        self.state.snapshot.store(Arc::new(snapshot));
    }

    fn spawn_snapshot_fetch(&self) {
        let symbol = self.symbol.clone();
        let tx = self.command_tx.clone();
        
        tokio::spawn(async move {
            match snapshot::fetch_snapshot(&symbol, 1000).await {
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

    fn handle_ws_trade(&mut self, received: ReceivedTrade) {
        self.total_trades += 1;
        self.update_rate_counter();

        let event_time = received.trade.trade_time;
        let received_at = received.received_at;
        let cutoff_time = event_time.saturating_sub(60_000);
        
        self.recent_trades.push_back(received.trade);
        
        while let Some(oldest) = self.recent_trades.front() {
            if oldest.trade_time < cutoff_time {
                self.recent_trades.pop_front();
            } else {
                break;
            }
        }
        
        //update metrics in place
        self.metrics.compute_trade_metrics(
            &self.recent_trades,
            self.total_trades,
            event_time,
            received_at);

        self.metrics.update_performance_metrics(self.updates_per_second);

        self.publish_snapshot();
    }

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
                self.sync_state = SyncState::new();
                self.is_syncing = true;
            }
            SyncOutcome::NoUpdates => {}
        }

        self.metrics.compute_book_metrics(
            &self.book, 
            &self.scaler,
            event_time,
            received_at
        );

        self.publish_snapshot();

        Ok(())
    }

    async fn handle_command(&mut self, cmd: EngineCommand) -> Result<bool> {
        match cmd {
            EngineCommand::NewSnapshot(snapshot) => {
                tracing::info!("Received new snapshot, lastUpdateId: {}", snapshot.last_update_id);

                self.sync_state.set_last_update_id(snapshot.last_update_id);
                self.book = OrderBook::from_snapshot(snapshot, &self.scaler);
                self.publish_snapshot();

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

    fn calculate_backoff(attempt: u32, config: &config::Config) -> Duration {
        let backoff_ms = config.initial_backoff_ms * 2u64.saturating_pow(attempt);
        Duration::from_millis(backoff_ms.min(config.max_backoff_ms))
    }

    async fn connect_with_retry<T, F, Fut>(
        connect_fn: F,
        stream_name: &str,
        config: &config::Config,
    ) -> Result<T>
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
                    if attempt >= config.max_reconnect_attempts {
                        tracing::error!(
                            "{} failed to reconnect after {} attempts: {}",
                            stream_name,
                            attempt,
                            e
                        );
                        return Err(e);
                    }

                    let backoff = Self::calculate_backoff(attempt, config);
                    tracing::warn!(
                        "{} connection failed (attempt {}/{}): {}. Retrying in {:?}",
                        stream_name,
                        attempt,
                        config.max_reconnect_attempts,
                        e,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    pub async fn run(mut self, config: config::Config) -> Result<()> {
        let symbol = self.symbol.clone();
        
        tracing::info!("Engine running for symbol: {}", self.symbol);

        let mut depth_stream = Box::pin(Self::connect_with_retry(
            || stream::connect_depth_stream(&symbol),
            "Depth stream",
            &config,
        ).await?);

        let mut trade_stream = Box::pin(Self::connect_with_retry(
            || stream::connect_trade_stream(&symbol),
            "Trade stream",
            &config,
        ).await?);

        loop {
            tokio::select! {
                biased;
                
                Some(cmd) = self.command_rx.recv() => {
                    let should_shutdown = self.handle_command(cmd).await?;
                    if should_shutdown {
                        break;
                    }
                }

                Some(result) = trade_stream.next() => {
                    match result {
                        Ok(trade) => self.handle_ws_trade(trade),
                        Err(e) => {
                            tracing::error!("Trade websocket stream error: {}", e);
                            self.is_syncing = true;
                            self.publish_snapshot();
                            
                            trade_stream = Box::pin(Self::connect_with_retry(
                                || stream::connect_trade_stream(&symbol),
                                "Trade stream",
                                &config
                            ).await?);
                        }
                    }
                }
                
                Some(result) = depth_stream.next() => {
                    match result {
                        Ok(update) => self.handle_ws_depth_update(update).await?,
                        Err(e) => {
                            tracing::error!("Depth websocket stream error: {}", e);
                            self.is_syncing = true;
                            self.publish_snapshot();
                            
                            // reset sync state - we need a fresh snapshot after reconnect
                            self.sync_state = SyncState::new();
                            self.spawn_snapshot_fetch();
                            
                            depth_stream = Box::pin(Self::connect_with_retry(
                                || stream::connect_depth_stream(&symbol),
                                "Depth stream",
                                &config
                            ).await?);
                        }
                    }
                }
                
                else => break
            }
        }
        
        Ok(())
    }
}