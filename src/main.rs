mod binance;
mod book;
mod engine;

use anyhow::Result;
use futures_util::StreamExt;
use rust_decimal::Decimal;
use crate::binance::snapshot;
use crate::book::scaler;
use crate::engine::engine::MarketDataEngine;

use crate::binance::stream::connect_trade_stream;



#[tokio::main]
async fn main() -> Result<()> {
    // Install default crypto provider for rustls before any TLS connections
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");


    //this is temporary just while developing other stuff
    let symbol = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: orderbook-engine <symbol>");
        std::process::exit(1);
    });
    let ws_stream = connect_trade_stream(&symbol).await?;
    tokio::pin!(ws_stream);
    while let Some(msg) = ws_stream.next().await {
        let m = msg.unwrap();
        println!("{:?}", m);

    }


    //run_depth_book_stuff().await?;

    Ok(())
}

async fn run_depth_book_stuff() -> Result<()> {
    
    let symbol = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: orderbook-engine <symbol>");
        std::process::exit(1);
    });

    println!("Fetching initial snapshot...");
    let snapshot = snapshot::fetch_snapshot(&symbol, 1000).await?;
    println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
    
    let (tick_size, step_size) = binance::exchange_info::fetch_tick_and_step_sizes(&symbol).await?;
    let scaler = scaler::Scaler::new(tick_size, step_size);
    let scaler_clone = scaler.clone();

    let (engine, _command_tx, state) = MarketDataEngine::new(symbol, snapshot, scaler);
    
    // spawn a task to periodically read and display the orderbook
    let state_clone = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            
            let book_arc = state_clone.current_book.load();
            let is_syncing = *state_clone.is_syncing.read().await;
            
            if is_syncing {
                println!("Status: Syncing...");
            } else {
                let (bids, asks) = book_arc.top_n_depth(2);

                let bids_decimal: Vec<(Decimal, Decimal)> = bids.iter()
                    .map(|(p, q)| (scaler_clone.ticks_to_price(*p), scaler_clone.ticks_to_price(*q)))
                    .collect();

                let asks_decimal: Vec<(Decimal, Decimal)> = asks.iter()
                    .map(|(p, q)| (scaler_clone.ticks_to_price(*p), scaler_clone.ticks_to_price(*q)))
                    .collect();

                println!("Top 2 levels - Bids: {:?}, Asks: {:?}", bids_decimal, asks_decimal);
                
                if let (Some((bid_price, _)), Some((ask_price, _))) = (book_arc.best_bid(), book_arc.best_ask()) {
                    println!("Best Bid: {}, Best Ask: {}, Spread: {}", scaler_clone.ticks_to_price(*bid_price), scaler_clone.ticks_to_price(*ask_price), scaler_clone.ticks_to_price(*ask_price - *bid_price));
                }
            }
        }
    });
    
    // Run the engine (this blocks until shutdown)
    engine.run().await?;
    
    Ok(())
}