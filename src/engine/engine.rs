use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::types::{DepthSnapshot, Trade};
use crate::binance::{snapshot, stream};
use crate::book::sync::{SyncState, SyncOutcome};
use crate::book::orderbook::OrderBook;
use crate::book::scaler::Scaler;
use crate::engine::metrics::MarketMetrics;
use crate::engine::state::MarketState;

// max trades that can be stored
pub const MAX_TRADES: usize = 1000;

pub enum EngineCommand {
    NewSnapshot(DepthSnapshot),
    RequestSnapshot,
    Shutdown,
}
pub struct MarketDataEngine {
    pub state: Arc<MarketState>,
    pub sync_state: SyncState,
    pub book: OrderBook,
    pub scaler: Scaler,
    pub symbol: String,
    pub recent_trades: VecDeque<Trade>,

    command_tx: mpsc::Sender<EngineCommand>,
    command_rx: mpsc::Receiver<EngineCommand>,

    update_counter : u64,
    last_rate_calc_time: std::time::Instant,
    updates_per_second: f64,

}

impl MarketDataEngine {
    pub fn new(symbol: String, initial_snapshot: DepthSnapshot, scaler: Scaler) -> (Self, mpsc::Sender<EngineCommand>, Arc<MarketState>) {
        let (command_tx, command_rx) = mpsc::channel(32);
        
        let mut sync_state = SyncState::new();
        sync_state.set_last_update_id(initial_snapshot.last_update_id);
        let book = OrderBook::from_snapshot(initial_snapshot.clone(), &scaler);
        let state = Arc::new(MarketState::new(book.clone()));
        
        let engine = MarketDataEngine {
            state: state.clone(),
            sync_state,
            book,
            scaler,
            symbol,
            recent_trades: VecDeque::with_capacity(MAX_TRADES),
            command_tx: command_tx.clone(),
            command_rx,
            update_counter: 0,
            last_rate_calc_time: std::time::Instant::now(),
            updates_per_second: 0.0,
        };
        
        (engine, command_tx, state) 
    }

    fn spawn_snapshot_fetch(&self) {
        let symbol = self.symbol.clone();
        let tx = self.command_tx.clone();
        
        tokio::spawn(async move {
            match snapshot::fetch_snapshot(&symbol, 1000).await {
                Ok(snapshot) => {
                    let _ = tx.send(EngineCommand::NewSnapshot(snapshot)).await;
                }
                Err(e) => {
                    tracing::error!("Fatal error, failed to fetch snapshot: {}", e);
                }
            }
        });
    }

    fn update_metrics(&mut self) {
        self.update_counter += 1;

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(self.last_rate_calc_time).as_secs_f64();


        if elapsed >= 1.0 {
            self.updates_per_second = self.update_counter as f64 / elapsed;
            

            self.last_rate_calc_time = now;
            self.update_counter = 0;
        }
            
        let metrics = MarketMetrics::compute(
            &self.book,
            &self.recent_trades,
            &self.scaler,
            self.updates_per_second,
        );
        
        self.state.metrics.store(Arc::new(metrics));
    }

    async fn handle_command(&mut self, cmd: EngineCommand) -> Result<bool> {
        match cmd {
            EngineCommand::NewSnapshot(snapshot) => {
                tracing::info!("Received new snapshot, lastUpdateId: {}", snapshot.last_update_id);
                self.sync_state.set_last_update_id(snapshot.last_update_id);
                self.book = OrderBook::from_snapshot(snapshot, &self.scaler);
                self.state.current_book.store(Arc::new(self.book.clone()));
                *self.state.is_syncing.write().await = false;
                self.update_metrics();
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

    async fn handle_ws_trade(&mut self, trade: Trade) {
        
        self.recent_trades.push_back(trade);
        if self.recent_trades.len() > MAX_TRADES {
            self.recent_trades.pop_front();
        }

        //the ole switcheroo
        self.state.recent_trades.store(Arc::new(self.recent_trades.clone()));
        self.update_metrics();
    }

    async fn handle_ws_update(&mut self, update: crate::binance::types::DepthUpdate) -> Result<()> {
        match self.sync_state.process_delta(update) {
            SyncOutcome::Updates(updates) => {
                for update in updates {
                    self.book.apply_update(&update, &self.scaler)?;
                }
                // the ole switcheroo
                self.state.current_book.store(Arc::new(self.book.clone()));
                *self.state.is_syncing.write().await = false;
            }
            SyncOutcome::GapBetweenUpdates => {
                self.command_tx.send(EngineCommand::RequestSnapshot).await?;
                self.sync_state = SyncState::new();
                *self.state.is_syncing.write().await = true;
            }
            SyncOutcome::NoUpdates => {}
        }

        self.update_metrics();

        Ok(())
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
                        Ok(trade) => self.handle_ws_trade(trade).await,
                        Err(e) => {
                            tracing::error!("Trade websocket stream error: {}", e);
                            break;
                        }
                    }
                }
                
                Some(result) = depth_stream.next() => {
                    match result {
                        Ok(update) => self.handle_ws_update(update).await?,
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