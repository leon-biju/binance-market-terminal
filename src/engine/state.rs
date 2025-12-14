use arc_swap::ArcSwap;
use crate::book::orderbook::OrderBook;

pub struct MarketState {
    // atomic pointer to the current, immutable OrderBook snapshot.
    // TUI can read this instantly and lock-free.
    pub current_book: ArcSwap<OrderBook>, 
    pub is_syncing: tokio::sync::RwLock<bool>,

}

impl MarketState {
    pub fn new(initial_book: OrderBook) -> Self {
        MarketState {
            current_book: ArcSwap::from_pointee(initial_book),
            is_syncing: tokio::sync::RwLock::new(true), // Start in syncing state
        }
    }
}