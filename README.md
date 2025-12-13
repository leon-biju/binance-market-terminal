# Orderbook Engine

A high-performance L2 orderbook built in Rust. It ingests Binance depth snapshots and incremental Websocket updates while maintaining a queryable in-memory book.

## Features
 - L2 order book (price -> aggregated quantity)
 - Handles gaps between updates and applies them in order
 - Constant time top-of-book queries
 - Fully benchmarked using Criterion

---

## Architecture Overview

### Data Flow
1. **Initial Snapshot**
    - Grabs the initial state from Binance REST API
    - Parses it into a `DepthSnapshot` struct and uses that to build the OrderBook
    
2. **Websocket Updates**
    - Streams depth updates from Binance via websocket
    - Parses them into `DepthUpdate` messages
    - Runs them through a sync layer before applying to the book

3. **Synchronization Layer**
    - Makes sure updates get applied in the right order
    - Buffers any out-of-order updates
    - Detects gaps using Binance's update IDs
    - Only emits updates when it's safe to apply them

4. **Order Book**
    - Applies updates atomically
    - Keeps track of bid and ask price levels
    - Gives you constant-time lookups for top-of-book (verify this?)


## Performance Results Summary

Ran benchmarks with Criterion in release mode to see how well it scales.

- **Snapshot construction (10,000 levels):** ~3.25 ms  
- **Update application (100 updates × 100 levels):** ~1.39 ms  
    Approx. 130 ns per price-level mutation.

- **High-churn updates (1,000 updates × 10 levels):** ~1.38 ms  
    Similar performance under frequent small updates.

- **Top-of-book queries (best bid/ask, spread, mid-price):** ~29 ns  
    Constant-time. No meaningful variation between runs.
