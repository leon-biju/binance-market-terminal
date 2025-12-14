use std::sync::Arc;
use tokio::sync::mpsc;
use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::types::DepthSnapshot;
use crate::binance::{snapshot, stream};
use crate::book::sync::{SyncState, SyncOutcome};
use crate::book::orderbook::OrderBook;
use crate::book::scaler::Scaler;
use crate::engine::state::MarketState;

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

    command_tx: mpsc::Sender<EngineCommand>,
    command_rx: mpsc::Receiver<EngineCommand>,
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
            command_tx: command_tx.clone(),
            command_rx,
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
                    eprintln!("Fatal error, failed to fetch snapshot: {}", e);
                }
            }
        });
    }

    async fn handle_command(&mut self, cmd: EngineCommand) -> Result<bool> {
        match cmd {
            EngineCommand::NewSnapshot(snapshot) => {
                println!("Received new snapshot, lastUpdateId: {}", snapshot.last_update_id);
                self.sync_state.set_last_update_id(snapshot.last_update_id);
                self.book = OrderBook::from_snapshot(snapshot, &self.scaler);
                self.state.current_book.store(Arc::new(self.book.clone()));
                *self.state.is_syncing.write().await = false;
                Ok(false)
            }
            EngineCommand::RequestSnapshot => {
                println!("Gap detected, requesting new snapshot...");
                self.spawn_snapshot_fetch();
                Ok(false)
            }
            EngineCommand::Shutdown => {
                println!("Shutting down engine...");
                Ok(true)
            }
        }
    }

    async fn handle_ws_update(&mut self, update: crate::binance::types::DepthUpdate) -> Result<()> {
        match self.sync_state.process_delta(update) {
            SyncOutcome::Updates(updates) => {
                for update in updates {
                    self.book.apply_update(&update, &self.scaler);
                }
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
        Ok(())
    }
    
    pub async fn run(mut self) -> Result<()> {
        let symbol = self.symbol.clone();
        let ws_stream = stream::connect_depth_stream(&symbol).await?;
        tokio::pin!(ws_stream);
        
        println!("Engine running for symbol: {}", self.symbol);
        
        loop {
            tokio::select! {
                biased;
                
                Some(cmd) = self.command_rx.recv() => {
                    let should_shutdown = self.handle_command(cmd).await?;
                    if should_shutdown {
                        break;
                    }
                }
                
                Some(result) = ws_stream.next() => {
                    match result {
                        Ok(update) => {
                            self.handle_ws_update(update).await?;
                        }
                        Err(e) => {
                            eprintln!("WebSocket error: {}", e);
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