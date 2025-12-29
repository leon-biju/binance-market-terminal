use std::collections::VecDeque;
use std::sync::Arc;
use std::time;
use tokio::sync::mpsc;
use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::types::{DepthSnapshot, Trade, ReceivedTrade, ReceivedDepthUpdate};
use crate::binance::{snapshot, stream};
use crate::book::sync::{SyncState, SyncOutcome};
use crate::book::orderbook::OrderBook;
use crate::book::scaler::Scaler;
use crate::engine::metrics::MarketMetrics;
use crate::engine::state::{MarketSnapshot, MarketState};

// max trades that can be stored
pub const INITIAL_STARTING_CAPACITY: usize = 1000;

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
        scaler: Scaler
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
            recent_trades: VecDeque::with_capacity(INITIAL_STARTING_CAPACITY),
            metrics: MarketMetrics::default(),
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

    pub async fn run(mut self) -> Result<()> {
        let symbol = self.symbol.clone();
        let depth_stream = stream::connect_depth_stream(&symbol).await?;
        let trade_stream = stream::connect_trade_stream(&symbol).await?;
        
        
        tracing::info!("Engine running for symbol: {}", self.symbol);

        tokio::pin!(depth_stream);
        tokio::pin!(trade_stream);
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
                            break;
                        }
                    }
                }
                
                Some(result) = depth_stream.next() => {
                    match result {
                        Ok(update) => self.handle_ws_depth_update(update).await?,
                        Err(e) => {
                            tracing::error!("Depth websocket stream error: {}", e);
                            break;
                        }
                    }
                }
                
                else => break
            }
        }
        
        Ok(())
    }
}