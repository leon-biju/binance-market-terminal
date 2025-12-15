use anyhow::Result;
use futures_util::StreamExt;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use crate::binance::types::{DepthUpdate, Trade};

pub async fn connect_depth_stream(symbol: &str) -> Result<impl StreamExt<Item = Result<DepthUpdate>>> {
    let url = format!("wss://stream.binance.com:9443/ws/{}@depth@100ms", symbol.to_lowercase());
    let (ws_stream, _) = connect_async(url).await?;
    let (_, read) = ws_stream.split();

    Ok(read.filter_map(|msg| async move {
        match msg {
            Ok(Message::Text(text)) => {
                Some(serde_json::from_str::<DepthUpdate>(&text).map_err(Into::into))
            }
            _ => None,
        }
    }))
}

pub async fn connect_trade_stream(symbol: &str) -> Result<impl StreamExt<Item = Result<Trade>>> {
    let url = format!("wss://stream.binance.com:9443/ws/{}@trade", symbol.to_lowercase());
    let (ws_stream, _) = connect_async(url).await?;
    let (_, read) = ws_stream.split();

    Ok(read.filter_map(|msg| async move {
        match msg {
            Ok(Message::Text(text)) => {
                Some(serde_json::from_str::<Trade>(&text).map_err(Into::into))
            }
            _ => None,
        }
    }))
}