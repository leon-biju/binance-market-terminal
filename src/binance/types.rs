use rand::Rng;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct DepthSnapshot {
    #[serde(rename = "lastUpdateId")]
    pub last_update_id: u64,
    pub bids: Vec<[String; 2]>, // [price, qty]
    pub asks: Vec<[String; 2]>,
}

impl DepthSnapshot {
    // generate fake snapshot for testing
    pub fn fake_snapshot(n_levels: usize) -> Self {
        let mut rng = rand::rng();
        
        // mid price around 50000 (like BTC), tick size 0.01
        let mid_price = Decimal::from(50000);
        let tick_size = Decimal::new(1, 2); // 0.01
        
        let bids = (0..n_levels)
            .map(|i| {
                // best bid is slightly below mid, then descending by tick size
                let price = (mid_price - tick_size * Decimal::from(i + 1)).to_string();
                let qty = Decimal::from(rng.random_range(1..100)).to_string();
                [price, qty]
            })
            .collect::<Vec<_>>();

        let asks = (0..n_levels)
            .map(|i| {
                // best ask is slightly above mid, then ascending by tick size
                let price = (mid_price + tick_size * Decimal::from(i + 1)).to_string();
                let qty = Decimal::from(rng.random_range(1..100)).to_string();
                [price, qty]
            })
            .collect::<Vec<_>>();

        Self {
            last_update_id: 0,
            bids,
            asks,
        }
    }
}


#[derive(Debug, Deserialize)]
pub struct DepthUpdate {
    #[serde(rename = "E")]
    pub event_time: u64,
    #[serde(rename = "U")]
    pub first_update_id: u64,
    #[serde(rename = "u")]
    pub final_update_id: u64,
    pub b: Vec<[String; 2]>, // bids
    pub a: Vec<[String; 2]>, // asks

    #[serde(skip, default = "std::time::Instant::now")]
    pub received_at: std::time::Instant,
}

impl DepthUpdate {
    // generate fake update for testing
    pub fn fake_update(last_update_id: u64, n_levels: usize) -> Self {
        let mut rng = rand::rng();
        let mut bids = Vec::new();
        let mut asks = Vec::new();

        let mid_price = Decimal::from(50000);
        let tick_size = Decimal::new(1, 2); // 0.01

        for _ in 0..n_levels {
            let is_bid = rng.random_bool(0.5);
            
            let price = if rng.random_bool(0.85) { // 85% update existing prices
                let offset = rng.random_range(1..1000);
                if is_bid {
                    mid_price - tick_size * Decimal::from(offset)
                } else {
                    mid_price + tick_size * Decimal::from(offset)
                }
            } else {
                let offset = rng.random_range(1000..5000);
                if is_bid {
                    mid_price - tick_size * Decimal::from(offset)
                } else {
                    mid_price + tick_size * Decimal::from(offset)
                }
            };
            
            // we'll simulate 10% deletions
            let qty = if rng.random_bool(0.1) {
                "0".to_string() // Level deletion
            } else {
                Decimal::from(rng.random_range(1..100)).to_string()
            };
            
            if is_bid {
                bids.push([price.to_string(), qty]);
            } else {
                asks.push([price.to_string(), qty]);
            }
        }

        Self {
            event_time: 0,
            first_update_id: last_update_id + 1,
            final_update_id: last_update_id + n_levels as u64 - 1,
            received_at: std::time::Instant::now(),
            b: bids,
            a: asks
        }
    }
}


pub enum Side {
    Sell, 
    Buy,
}
impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
         match self {
            Side::Buy => write!(f, "BUY"),
            Side::Sell => write!(f, "SELL"),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Trade {
    #[serde(rename = "p")]
    pub price: Decimal,
    #[serde(rename = "q")]
    pub quantity: Decimal,
    #[serde(rename = "T")]
    pub trade_time: u64,
    #[serde(rename = "m")]
    pub is_buyer_maker: bool,

    #[serde(skip, default = "std::time::Instant::now")]
    pub received_at: std::time::Instant,
}

impl Trade {
    pub fn side(&self) -> Side{
        if self.is_buyer_maker {
            Side::Sell
        } else {
            Side::Buy
        }
    }
}