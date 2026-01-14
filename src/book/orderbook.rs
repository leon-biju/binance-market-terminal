//dont touch this file
use std::collections::BTreeMap;
use num_traits::Zero;
use anyhow::Result;

use crate::{binance::{types::{DepthSnapshot, DepthUpdate}}};
use crate::book::scaler;

pub type RawDepthLevel = (u64, u64);


#[derive(Debug, Clone)]
pub struct OrderBook {
    bids: BTreeMap<u64, u64>,
    asks: BTreeMap<u64, u64>,
}

impl OrderBook {
    pub fn from_snapshot(snapshot: DepthSnapshot, scaler: &scaler::Scaler) -> Result<Self> {
        let mut bids = BTreeMap::new();
        let mut asks = BTreeMap::new();
        
        for [price, qty] in snapshot.bids {
            let pt = scaler.price_to_ticks(&price).ok_or_else(|| anyhow::anyhow!("Failed to convert price ({}) to ticks", &price))?;
            let qt = scaler.qty_to_ticks(&qty).ok_or_else(|| anyhow::anyhow!("Failed to convert qty ({}) to ticks", &price))?;
            bids.insert(pt, qt);
        }
        
        for [price, qty] in snapshot.asks {
            let pt = scaler.price_to_ticks(&price).ok_or_else(|| anyhow::anyhow!("Failed to convert price ({}) to ticks", &price))?;
            let qt = scaler.qty_to_ticks(&qty).ok_or_else(|| anyhow::anyhow!("Failed to convert qty ({}) to ticks", &price))?;
            asks.insert(pt, qt);
        }
        

        Ok(Self {
            bids,
            asks
        })
    }
    
    pub fn apply_update(&mut self, update: &DepthUpdate, scaler: &scaler::Scaler) -> Result<()> {
        for [price, qty] in &update.b {
            let pt = scaler.price_to_ticks(&price).ok_or_else(|| anyhow::anyhow!("Failed to convert price ({}) to ticks", &price))?;
            let qt = scaler.qty_to_ticks(&qty).ok_or_else(|| anyhow::anyhow!("Failed to convert qty ({}) to ticks", &qty))?;
            if qt.is_zero() {
                self.bids.remove(&pt);
            } else {
                self.bids.insert(pt, qt);
            }
        }
        
        for [price, qty] in &update.a {
            let pt = scaler.price_to_ticks(&price).ok_or_else(|| anyhow::anyhow!("Failed to convert price ({}) to ticks", &price))?;
            let qt = scaler.qty_to_ticks(&qty).ok_or_else(|| anyhow::anyhow!("Failed to convert qty ({}) to ticks", &qty))?;
            if qt.is_zero() {
                self.asks.remove(&pt);
            } else {
                self.asks.insert(pt, qt);
            }
        }
        Ok(())
    }
    
    
    pub fn best_bid(&self) -> Option<(&u64, &u64)> {
        self.bids.iter().next_back() // highest price
    }

    pub fn best_ask(&self) -> Option<(&u64, &u64)> {
        self.asks.iter().next() // lowest price
    }

    pub fn spread(&self) -> Option<u64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some(ask - bid),
            _ => None,
        }
    }

    pub fn mid_price(&self) -> Option<u64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid, _)), Some((ask, _))) => Some((ask + bid) / 2),
            _ => None,
        }
    }

    pub fn top_n_depth(&self, n: usize) -> (Vec<RawDepthLevel>, Vec<RawDepthLevel>) {
        let best_n_bids: Vec<RawDepthLevel> = self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(price, qty)| (*price, *qty))
            .collect();

        let best_n_asks: Vec<RawDepthLevel> = self.asks
            .iter()
            .take(n)
            .map(|(price, qty)| (*price, *qty))
            .collect();

        (best_n_bids, best_n_asks)
    }

    pub fn imbalance_ratio(&self, levels: usize) -> Option<f64> {
        let (bids, asks) = self.top_n_depth(levels);

        if bids.is_empty() || asks.is_empty() {
            return None;
        }
        
        let bid_volume: u64 = bids.iter()
            .map(|(_, qty)| *qty)
            .sum();

        let ask_volume: u64 = asks.iter()
            .map(|(_, qty)| *qty)
            .sum();

        let total_volume = bid_volume + ask_volume;

        if total_volume == 0 {
            return None;
        }

        Some(bid_volume as f64 / total_volume as f64)
    } 
}
