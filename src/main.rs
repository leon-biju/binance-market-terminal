mod binance;
mod snapshot;
mod stream;
mod sync;

use anyhow::Result;
use futures_util::StreamExt;


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
    let snapshot = snapshot::fetch_snapshot(symbol, 1).await?;
    println!("Snapshot lastUpdateId: {}", snapshot.last_update_id);
    
    let mut sync = sync::SyncState::new();
    sync.set_last_update_id(snapshot.last_update_id);

    println!("Processing deltas!");
    tokio::pin!(ws_stream);
    while let Some(result) = ws_stream.next().await {
        match result {
            Ok(update) => {
                let first_id = update.first_update_id;
                let final_id = update.final_update_id;
                match sync.process_delta(update) {
                    Ok(true) => {
                        println!("successfully going to update! U={}, u={}. update count is {}", first_id, final_id, final_id-first_id);
                        sync.set_last_update_id(final_id);

                    }
                    Ok(false) => {
                        //buffered or dropped just keep going
                    }
                    Err(e) => {
                        //todo: resync logic for now just crash and burn
                        eprintln!("DESYNC DETECTED!!!: {}", e);
                        break;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                break;
            }
        }
    }
    
    
    
    Ok(())
}
