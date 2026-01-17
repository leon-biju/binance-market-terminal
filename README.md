<div align="center">

# Binance Market Terminal

**A high-performance, real-time L2 orderbook and trade stream ingestor built in Rust**

[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<!-- TODO: Add screenshot of TUI here -->
<!-- ![TUI Screenshot](docs/assets/tui-screenshot.png) -->

</div>

---

## Overview

This is an unofficial low-latency market data processing system that ingests and displays live trading data for a given symbol on Binance.


## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Architecture](#architecture)
- [Performance](#performance)
- [Benchmarks](#benchmarks)
- [Project Structure](#project-structure)
- [Contributing](#contributing)
- [License](#license)

---

## Requirements

- **Cargo** (included with Rust)
- Internet connection (for Binance API access)

### System Dependencies

On Linux, you may need to install the following for TLS support:

```bash
# Debian/Ubuntu
sudo apt-get install pkg-config libssl-dev

# Fedora/RHEL
sudo dnf install openssl-devel
```

---

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/orderbook-engine.git
cd orderbook-engine

# Build in release mode (recommended for performance)
cargo build --release
```

The compiled binary will be available at `target/release/orderbook-engine`.

### Development Build

```bash
# Build with debug symbols
cargo build

# Run tests
cargo test

# Run with cargo
cargo run -- BTCUSDT
```

---

## Quick Start

```bash
# Run the orderbook engine for a specific trading pair listed by Binance
./target/release/orderbook-engine BTCUSDT
```

The TUI will launch displaying:
- Real-time bid/ask levels
- Spread and mid-price
- Order imbalance metrics
- Recent and significant trades

### Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit application |
| `↑` / `↓` | Increase/decrease time between tui frame updates |

<!-- TODO: Add more keybindings if they exist -->

---

## Configuration

Configuration is managed via `config.toml` in the project root. If the file is missing, defaults are used.


## Architecture

### System Design

<!-- TODO: Add architecture diagram here -->
<!-- ![Architecture Diagram](docs/assets/architecture.png) -->

The engine employs a **single-writer, multiple-reader** pattern optimized for high-frequency updates:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Market Data Engine                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │
│  │   Binance    │    │     Sync     │    │    Orderbook     │   │
│  │  WebSocket   │───▶│    Layer     │───▶│   (Workspace)    │   │
│  └──────────────┘    └──────────────┘    └────────┬─────────┘   │
│         │                   │                     │              │
│         │                   │              ┌──────▼─────────┐    │
│         │                   │              │   ArcSwap      │    │
│  ┌──────▼──────┐     ┌──────▼──────┐      │  (Published)   │    │
│  │   Binance   │     │    Gap      │      └──────┬─────────┘    │
│  │  REST API   │◀────│  Recovery   │             │              │
│  └─────────────┘     └─────────────┘             │              │
│                                                  │              │
└──────────────────────────────────────────────────┼──────────────┘
                                                   │
                                            ┌──────▼──────┐
                                            │     TUI     │
                                            │  (Readers)  │
                                            └─────────────┘
```

### Core Components

| Component | Description |
|-----------|-------------|
| **Workspace Book** | Mutable orderbook where updates are applied without locks |
| **Published Snapshot** | Immutable copy exposed via `ArcSwap` for lock-free reads |
| **Sync Layer** | Ensures update ordering, buffers out-of-order messages, detects gaps |
| **Gap Recovery** | Async snapshot fetch triggered on sequence gaps |

### Data Flow

1. **Initial Snapshot** — Fetched from Binance REST API to bootstrap the orderbook
2. **WebSocket Stream** — Continuous depth updates parsed and queued
3. **Synchronization** — Updates validated against sequence IDs, gaps trigger recovery
4. **Application** — Valid updates applied to workspace, then atomically published
5. **Consumption** — TUI reads published snapshot with zero contention

### Gap Recovery Strategy

When the sync layer detects a gap in update IDs:

1. Recovery command sent via async channel
2. Background task fetches fresh snapshot (non-blocking)
3. Engine continues processing buffered WebSocket messages
4. New snapshot atomically replaces stale book
5. Biased `select!` ensures snapshot commands are prioritized

---

## Performance

### Benchmark Results

Benchmarks run on release builds using [Criterion](https://github.com/bheisler/criterion.rs):

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Snapshot construction (10k levels) | ~3.25 ms | — |
| Update batch (100 × 100 levels) | ~1.39 ms | ~7.2M levels/sec |
| High-churn updates (1000 × 10 levels) | ~1.38 ms | ~7.2M levels/sec |
| Top-of-book query | ~29 ns | ~34M queries/sec |

**Per-level mutation cost:** ~130 ns

### Latency Considerations

| Factor | Impact |
|--------|--------|
| **Network** | Primary bottleneck; Binance streams from Asia |
| **UK Deployment** | Expect 100-250ms network latency |
| **Processing** | Sub-microsecond for most operations |

---

## Benchmarks

Run the benchmark suite:

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench -- "from_snapshot"

# Generate HTML report (output in target/criterion/)
cargo bench -- --verbose
```

Benchmark results are saved to `target/criterion/` with detailed HTML reports.

---

## Project Structure

```
orderbook-engine/
├── Cargo.toml              # Package manifest and dependencies
├── config.toml             # Runtime configuration
├── README.md
│
├── src/
│   ├── main.rs             # Application entry point
│   ├── lib.rs              # Library exports
│   ├── config.rs           # Configuration loading
│   │
│   ├── binance/            # Binance API integration
│   │   ├── mod.rs
│   │   ├── exchange_info.rs    # Symbol metadata fetching
│   │   ├── snapshot.rs         # REST depth snapshot
│   │   ├── stream.rs           # WebSocket stream handling
│   │   └── types.rs            # API response types
│   │
│   ├── book/               # Orderbook implementation
│   │   ├── mod.rs
│   │   ├── orderbook.rs        # Core orderbook data structure
│   │   ├── scaler.rs           # Price/quantity scaling
│   │   └── sync.rs             # Update synchronization
│   │
│   ├── engine/             # Runtime engine
│   │   ├── mod.rs
│   │   ├── runtime.rs          # Main event loop
│   │   ├── state.rs            # Shared state management
│   │   └── metrics.rs          # Performance metrics
│   │
│   └── tui/                # Terminal UI
│       ├── mod.rs
│       ├── app.rs              # Application state
│       └── ui.rs               # Rendering logic
│
├── benches/
│   └── orderbook_bench.rs  # Criterion benchmarks
│
└── logs/                   # Runtime logs (daily rotation)
```

---

## Logging

Logs are written to the `logs/` directory with daily rotation:

```bash
# View latest logs
tail -f logs/ingestor.log.$(date +%Y-%m-%d)

# Search logs
grep "SNAPSHOT" logs/ingestor.log.*
```

Log level can be controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=debug ./target/release/orderbook-engine BTCUSDT
```

---

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| TLS/SSL errors | Ensure OpenSSL dev libraries are installed |
| Connection timeouts | Check internet connectivity; Binance may be rate-limiting |
| High latency | Expected for non-Asian deployments; consider co-location |

### Debug Mode

```bash
# Run with debug logging
RUST_LOG=debug cargo run -- BTCUSDT

# Run with trace logging (very verbose)
RUST_LOG=trace cargo run -- BTCUSDT
```

---

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

### Development Guidelines

- Run `cargo fmt` before committing
- Ensure `cargo clippy` passes without warnings
- Add tests for new functionality
- Update documentation as needed

---

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- [Binance API](https://binance-docs.github.io/apidocs/) for market data
- [Ratatui](https://github.com/ratatui-org/ratatui) for the terminal UI framework
- [Criterion](https://github.com/bheisler/criterion.rs) for benchmarking

---

<div align="center">

**[⬆ Back to Top](#orderbook-engine)**

</div>