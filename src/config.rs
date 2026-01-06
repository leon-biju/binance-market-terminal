use serde::Deserialize;
use std::fs;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub orderbook_initial_snapshot_depth: u16,
    pub orderbook_imbalance_depth_levels: usize,

    pub recent_trades_starting_capacity: usize,
    pub significant_trades_retention_secs: u64,
    pub significant_trade_volume_pct: f64,
    pub min_trades_for_significance: usize,

    pub max_reconnect_attempts: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,

    pub orderbook_depth_display_count: usize,
    pub recent_trades_display_count: usize,
    pub significant_trades_display_count: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            orderbook_initial_snapshot_depth: 1000,
            orderbook_imbalance_depth_levels: 10,
            
            recent_trades_starting_capacity: 1000,
            significant_trades_retention_secs: 120,
            
            significant_trade_volume_pct: 0.05,
            min_trades_for_significance: 50,
            
            max_reconnect_attempts: 10,
            initial_backoff_ms: 100,
            max_backoff_ms: 30000,
            
            orderbook_depth_display_count: 5,
            recent_trades_display_count: 10,
            significant_trades_display_count: 20,
        }
    }
}

pub fn load_config() -> Config {
    fs::read_to_string("config.toml")
        .ok()
        .and_then(|content| toml::from_str(&content).ok())
        .unwrap_or_default()
}