use std::collections::BTreeMap;
use rust_decimal::Decimal;
use crate::binance::{DepthSnapshot, DepthUpdate};

#[derive(Debug)]
pub struct OrderBook {
    bids: BTreeMap<Decimal, Decimal>, // price -> quantity
    asks: BTreeMap<Decimal, Decimal>,
}

impl OrderBook {
    pub fn from_snapshot(snapshot: DepthSnapshot) -> Self {
        let mut book = Self {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
        };
        
        for [price, qty] in snapshot.bids {
            if let (Ok(p), Ok(q)) = (price.parse(), qty.parse()) {
                book.bids.insert(p, q);
            }
        }
        
        for [price, qty] in snapshot.asks {
            if let (Ok(p), Ok(q)) = (price.parse(), qty.parse()) {
                book.asks.insert(p, q);
            }
        }
        
        book
    }
    
    pub fn apply_update(&mut self, update: &DepthUpdate) {
        for [price, qty] in &update.b {
            if let (Ok(p), Ok(q)) = (price.parse::<Decimal>(), qty.parse::<Decimal>()) {
                if q.is_zero() {
                    self.bids.remove(&p);
                } else {
                    self.bids.insert(p, q);
                }
            }
        }
        
        for [price, qty] in &update.a {
            if let (Ok(p), Ok(q)) = (price.parse::<Decimal>(), qty.parse::<Decimal>()) {
                if q.is_zero() {
                    self.asks.remove(&p);
                } else {
                    self.asks.insert(p, q);
                }
            }
        }
    }
    
    pub fn highest_bid(&self) -> Option<(&Decimal, &Decimal)> {
        self.bids.iter().next_back()
    }
    
    pub fn lowest_ask(&self) -> Option<(&Decimal, &Decimal)> {
        self.asks.iter().next()
    }
}