use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>, // [price, qty]
    pub asks: Vec<[String; 2]>,
}

#[derive(Debug, Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "E")]
    pub event_time: u64,
    pub s: String, // symbol
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    pub b: Vec<[String; 2]>, // bids
    pub a: Vec<[String; 2]>, // asks
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum StreamMessage {
    DepthUpdate { data: DepthUpdate },
}