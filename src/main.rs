mod binance;
mod book;
mod config;
mod engine;
mod tui;

use std::sync::Arc;

use anyhow::Result;
use tracing::info;
use tracing_appender::rolling;
use tracing_subscriber::{EnvFilter, fmt};

use crate::binance::snapshot;
use crate::book::scaler;
use crate::engine::runtime::{EngineCommand, MarketDataEngine};
use crate::tui::App;

fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = rolling::daily("logs", "ingestor.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = EnvFilter::from_default_env().add_directive("info".parse().unwrap());

    fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(false)
        .init();
    guard
}

#[tokio::main]
async fn main() -> Result<()> {
    let _log_guard = init_logging();

    // Install default crypto provider for rustls before any TLS connections
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let symbol = std::env::args().nth(1).ok_or_else(|| {
        anyhow::anyhow!("Usage: orderbook-engine <symbol>")
    })?;
    // Add visual separator in logs
    info!("");
    info!("================================================");
    info!("");
    info!("[PROGRAM START]");

    let conf = Arc::new(config::load_config());
    info!("{:?}", conf);

    let snapshot = snapshot::fetch_snapshot(&symbol, conf.orderbook_initial_snapshot_depth).await?;
    info!(
        "[DEPTH SNAPSHOT_INFO] lastUpdateId: {}",
        snapshot.last_update_id
    );

    let (tick_size, step_size) = binance::exchange_info::fetch_tick_and_step_sizes(&symbol).await?;
    let scaler = scaler::Scaler::new(tick_size, step_size);

    let (engine, command_tx, state) =
        MarketDataEngine::new(symbol, snapshot, scaler, conf.clone())?;

    // Spawn the engine in the background
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = engine.run().await {
            tracing::error!("Engine error: {}", e);
        }
    });

    // Run the TUI in the main task
    let mut app = App::new(state, conf);
    app.run().await?;

    // TUI exited, engine will continue running until dropped
    command_tx.send(EngineCommand::Shutdown).await?;

    if let Err(e) = engine_handle.await {
        tracing::error!("Engine task panicked: {}", e);
    }

    info!("[PROGRAM END]");
    Ok(())
}
