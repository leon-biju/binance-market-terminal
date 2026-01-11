use anyhow::Result;
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::binance::types::{
    CombinedStreamMessage, DepthUpdate, MarketEvent, ReceivedDepthUpdate, ReceivedTrade, Trade,
};

// connect to a combined stream that delivrs both depth updates and trades
pub async fn connect_market_stream(symbol: &str) -> Result<impl StreamExt<Item = Result<MarketEvent>>> {
    let symbol_lower = symbol.to_lowercase();
    let url = format!(
        "wss://stream.binance.com:9443/stream?streams={}@depth@100ms/{}@trade",
        symbol_lower, symbol_lower
    );
    let (ws_stream, _) = connect_async(url).await?;
    let (_, read) = ws_stream.split();

    Ok(read.filter_map(|msg| async move {
        match msg {
            Ok(Message::Text(text)) => {
                let received_at = std::time::Instant::now();
                let combined: CombinedStreamMessage = match serde_json::from_str(&text) {
                    Ok(c) => c,
                    Err(e) => return Some(Err(e.into())),
                };

                if combined.stream.ends_with("@depth@100ms") {
                    match serde_json::from_value::<DepthUpdate>(combined.data) {
                        Ok(update) => Some(Ok(MarketEvent::Depth(ReceivedDepthUpdate {
                            update,
                            received_at,
                        }))),
                        Err(e) => Some(Err(e.into())),
                    }
                } else if combined.stream.ends_with("@trade") {
                    match serde_json::from_value::<Trade>(combined.data) {
                        Ok(trade) => Some(Ok(MarketEvent::Trade(ReceivedTrade {
                            trade,
                            received_at,
                        }))),
                        Err(e) => Some(Err(e.into())),
                    }
                } else {
                    tracing::warn!("Unknown stream type: {}", combined.stream);
                    None
                }
            }
            _ => None,
        }
    }))
}