mod binance;
mod snapshot;

use anyhow::Result;


#[tokio::main]
async fn main() -> Result<()>{
    println!("Hello, world!");
    
    let snapshot = snapshot::fetch_snapshot("BTCUSDT", 1).await?;
    println!("{:?}", snapshot);
    Ok(())
}
