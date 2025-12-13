mod binance;
mod book;

use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::{stream, snapshot};
use crate::book::{orderbook, scaler, sync};


#[tokio::main]
async fn main() -> Result<()>{
    // Install default crypto provider for rustls before any TLS connections
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    let symbol = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: orderbook-engine <symbol>");
        std::process::exit(1);
    });

    println!("Connecting to WebSocket...");
    let ws_stream = stream::connect_depth_stream(&symbol).await?;
    
    println!("Fetching snapshot...");
    let snapshot = snapshot::fetch_snapshot(&symbol, 1000).await?;
    println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
    
    let mut sync = sync::SyncState::new();
    sync.set_last_update_id(snapshot.last_update_id);

    // get tick size and step size
    let (tick_size, step_size) = binance::exchange_info::fetch_tick_and_step_sizes(&symbol).await?;
    let scaler = scaler::Scaler::new(tick_size, step_size);

    let mut book = orderbook::OrderBook::from_snapshot(snapshot, &scaler);
    
    println!("Processing deltas!");
    tokio::pin!(ws_stream);

    // main listening loop   
    while let Some(result) = ws_stream.next().await {

        let update = result?;

        match sync.process_delta(update) {
            sync::SyncOutcome::Updates(updates) => {
                for update in updates {
                    //println!("Applying update! U={}, u={}", update.first_update_id, update.final_update_id);
                    book.apply_update(&update, &scaler);
                }
            }
            sync::SyncOutcome::GapBetweenUpdates => {
                println!("Gap detected; refetching snapshot and resetting state");
                let snapshot = snapshot::fetch_snapshot(&symbol, 1000).await?;
                println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
                sync = sync::SyncState::new();
                sync.set_last_update_id(snapshot.last_update_id);
                book = orderbook::OrderBook::from_snapshot(snapshot, &scaler);
            }
            sync::SyncOutcome::NoUpdates => {
                println!("No updates!");
            }
        }

        let (bids, asks) = book.top_n_depth(2);
        let bids_scaled: Vec<_> = bids.iter().map(|(price, qty)| (scaler.ticks_to_price(*price), scaler.ticks_to_qty(*qty))).collect();
        let asks_scaled: Vec<_> = asks.iter().map(|(price, qty)| (scaler.ticks_to_price(*price), scaler.ticks_to_qty(*qty))).collect();
        println!("Bids: {:?}, Asks: {:?}", bids_scaled, asks_scaled);
        //break; //TEMPORARY DEBUG STATEMENT to only listen to one message
    }
    


    
    
    
    Ok(())
}