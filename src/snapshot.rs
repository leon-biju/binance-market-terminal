use anyhow::Result;
use crate::binance::DepthSnapshot;

const DEPTH_SNAPSHOT_URL: &str = "https://api.binance.com/api/v3/depth";

pub async fn fetch_snapshot(symbol: &str, limit: u16) -> Result<DepthSnapshot> {
    let url = format!("{}?symbol={}&limit={}", DEPTH_SNAPSHOT_URL, symbol, limit);
    let response = reqwest::get(&url).await?;

    let json_value: serde_json::Value = response.json().await?;
    
    if json_value.get("code").is_some() {
        let msg = json_value.get("msg")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("Binance API error: {}", msg);
    }
    
    let snapshot: DepthSnapshot = serde_json::from_value(json_value)?;

    Ok(snapshot)
}