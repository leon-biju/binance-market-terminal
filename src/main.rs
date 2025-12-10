mod binance;
mod snapshot;
mod stream;
mod sync;
mod orderbook;

use anyhow::Result;
use futures_util::StreamExt;


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

    let mut book = orderbook::OrderBook::from_snapshot(snapshot);
    
    println!("Processing deltas!");
    tokio::pin!(ws_stream);

    // main listening loop   
    while let Some(result) = ws_stream.next().await {

        let update = result?;

        match sync.process_delta(update) {
            sync::SyncOutcome::Updates(updates) => {
                for update in updates {
                    println!("Applying update! U={}, u={}", update.first_update_id, update.final_update_id);
                    book.apply_update(&update);
                }
            }
            sync::SyncOutcome::GapBetweenUpdates => {
                println!("Gap detected; refetching snapshot and resetting state");
                let snapshot = snapshot::fetch_snapshot(&symbol, 1000).await?;
                println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
                sync = sync::SyncState::new();
                sync.set_last_update_id(snapshot.last_update_id);
                book = orderbook::OrderBook::from_snapshot(snapshot);
            }
            sync::SyncOutcome::NoUpdates => {
                println!("No updates!");
            }
        }

        println!("{:?}", book.top_n_depth(2));
        //break; //TEMPORARY DEBUG STATEMENT to only listen to one message
    }
    


    
    
    
    Ok(())
}