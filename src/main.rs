mod binance;
mod snapshot;
mod stream;
mod sync;
mod orderbook;

use anyhow::Result;
use futures_util::StreamExt;

use crate::binance::DepthUpdate;


#[tokio::main]
async fn main() -> Result<()>{
    // Install default crypto provider for rustls before any TLS connections
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    
    let symbol = "BTCUSDT";


    println!("Connecting to WebSocket...");
    let ws_stream = stream::connect_depth_stream(symbol).await?;
    
    println!("Fetching snapshot...");
    let snapshot = snapshot::fetch_snapshot(symbol, 1000).await?;
    println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
    
    let mut sync = sync::SyncState::new();
    sync.set_last_update_id(snapshot.last_update_id);

    let mut book = orderbook::OrderBook::from_snapshot(snapshot);
    
    println!("Processing deltas!");
    tokio::pin!(ws_stream);

    while let Some(result) = ws_stream.next().await {
        let update = result?;
        handle_message(update, &mut sync, &mut book)?;
        

        break; //TEMPORARY DEBUG STATEMENT to only listen to one message
    }
    

    println!("{:?}", book);
    
    
    Ok(())
}



fn handle_message(update: DepthUpdate, sync: &mut sync::SyncState, book: &mut orderbook::OrderBook) -> Result<()>{
    let first_id = update.first_update_id;
    let final_id = update.final_update_id;
    if let Some(updates) = sync.process_delta(update)? {
        for update in updates {
            println!("Applying update! U={}, u={}", first_id, final_id);

            book.apply_update(&update);
        }
    }
    Ok(())
}