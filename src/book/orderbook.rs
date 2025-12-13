//dont touch this file
use std::collections::BTreeMap;
use num_traits::Zero;
use crate::{binance::{types::{DepthSnapshot, DepthUpdate}}};
use crate::book::scaler;


#[derive(Debug)]
pub struct OrderBook {
    bids: BTreeMap<u64, u64>,
    asks: BTreeMap<u64, u64>,
}

impl OrderBook {
    pub fn from_snapshot(snapshot: DepthSnapshot, scaler: &scaler::Scaler) -> Self {
        let mut book = Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        };
        
        for [price, qty] in snapshot.bids {
            let pt = scaler.price_to_ticks(&price).unwrap();
            let qt = scaler.qty_to_ticks(&qty).unwrap();
            book.bids.insert(pt, qt);
        }
        
        for [price, qty] in snapshot.asks {
            let pt = scaler.price_to_ticks(&price).unwrap();
            let qt = scaler.qty_to_ticks(&qty).unwrap();
            book.asks.insert(pt, qt);
        }
        
        book
    }
    
    pub fn apply_update(&mut self, update: &DepthUpdate, scaler: &scaler::Scaler) {
        for [price, qty] in &update.b {
            let pt = scaler.price_to_ticks(&price).unwrap();
            let qt = scaler.qty_to_ticks(&qty).unwrap();
            if qt.is_zero() {
                self.bids.remove(&pt);
            } else {
                self.bids.insert(pt, qt);
            }
        }
        
        for [price, qty] in &update.a {
            let pt = scaler.price_to_ticks(&price).unwrap();
            let qt = scaler.qty_to_ticks(&qty).unwrap();
            if qt.is_zero() {
                self.asks.remove(&pt);
            } else {
                self.asks.insert(pt, qt);
            }
        }
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

    pub fn top_n_depth(&self, n: usize) -> (Vec<(u64, u64)>, Vec<(u64, u64)>) {
        let best_n_bids: Vec<(u64, u64)> = self.bids
            .iter()
            .rev()
            .take(n)
            .map(|(price, qty)| (*price, *qty))
            .collect();

        let best_n_asks: Vec<(u64, u64)> = self.asks
            .iter()
            .take(n)
            .map(|(price, qty)| (*price, *qty))
            .collect();

        (best_n_bids, best_n_asks)
    }
}
