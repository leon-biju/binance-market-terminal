# Orderbook Engine

A high-performance L2 orderbook built in Rust. It ingests Binance depth snapshots and incremental Websocket updates while maintaining a queryable in-memory book.

## Features
 - L2 order book (price -> aggregated quantity)
 - Lock-free reads via atomic pointer swapping
 - Handles gaps between updates and applies them in order
 - Constant time top-of-book queries
 - Non-blocking snapshot recovery
 - Fully benchmarked using Criterion

---

## Architecture Overview

### Engine Design
The engine uses a single-writer, multiple-readers pattern optimized for high-frequency updates. It maintains two separate orderbook instances:

- **Hot Path** - A mutable workspace where updates are applied rapidly without any locks
- **Published Snapshot** - An immutable book exposed via `ArcSwap` for lock-free TUI reads

This design means the engine can process updates at full speed while consumers read the orderbook with zero synchronization overhead. Updates are applied in-place to the workspace, then cloned and atomically swapped into the shared pointer when ready.

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
    - Gives you constant-time lookups for top-of-book

### Gap Recovery
When the sync layer detects a gap in update IDs, it triggers an async snapshot fetch via a command channel. The HTTP request happens in a background task so the engine can continue processing WebSocket messages. Once the new snapshot arrives, it resets the sync state and swaps in a fresh book. The command channel is prioritized using biased selection to ensure snapshot updates apply immediately.


## Performance Results Summary

Ran benchmarks with Criterion in release mode to see how well it scales.

- **Snapshot construction (10,000 levels):** ~3.25 ms  
- **Update application (100 updates × 100 levels):** ~1.39 ms  
    Approx. 130 ns per price-level mutation.

- **High-churn updates (1,000 updates × 10 levels):** ~1.38 ms  
    Similar performance under frequent small updates.

- **Top-of-book queries (best bid/ask, spread, mid-price):** ~29 ns  
    Constant-time. No meaningful variation between runs.
