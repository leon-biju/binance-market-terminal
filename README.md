<div align="center">

# Binance Market Terminal

**A high-performance, real-time L2 orderbook and trade stream ingestor built in Rust**

[![Rust](https://img.shields.io/badge/rust-1.82%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

<img width="2524" height="1381" alt="screenshot-2026-01-17_18-29-29" src="https://github.com/user-attachments/assets/5621a234-4ff5-473e-aa09-12c03c328ea1" />

</div>

---

## Overview

A low-latency market data processing system that ingests and displays live L2 orderbook data and trade streams for any symbol on Binance using websockets, with automatic gap recovery and lock-free snapshot publishing.

### Non-Goals

- **Trading execution**: This is a read-only data ingestor, not a trading bot
- **Historical data storage**: No persistence layer. Data is ephemeral
- **Multi-exchange support**: Binance-specific implementation. Although adding support shouldn't be too hard if the exchange API is similar
- **Guaranteed Sub-millisecond latency**: Optimized for correctness over raw speed

### Disclaimer

> This project is not affiliated with or endorsed by Binance. It is a toy project intended for **educational and research purposes only**.

---

## Quick Start

```bash
# Clone and build
git clone https://github.com/leon-biju/binance-market-terminal.git
cd binance-market-terminal
cargo build --release

# Run for any Binance trading pair
./target/release/binance-market-terminal BTCUSDT
```

<details>
<summary><strong>System Dependencies (Linux)</strong></summary>

```bash
# Debian/Ubuntu
sudo apt-get install pkg-config libssl-dev

# Fedora/RHEL
sudo dnf install openssl-devel
```

</details>

---

## Architecture

### System Design

<!-- TODO: Add architecture diagram here -->
<!-- ![Architecture Diagram](docs/assets/architecture.png) -->

The engine employs a **single-writer, multiple-reader** pattern optimized for high-frequency updates:

```
┌─────────────────────────────────────────────────────────────────┐
│                        Market Data Engine                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐   │
│  │   Binance    │    │     Sync     │    │    Orderbook     │   │
│  │  WebSocket   │───▶│    Layer     │───▶│   (Workspace)    │   │
│  └──────────────┘    └──────────────┘    └───────┬──────────┘   │
│         │                   │                    │              │
│         │                   │              ┌─────▼─────────┐    │
│         │                   │              │  ArcSwap      │    │
│  ┌──────▼──────┐     ┌──────▼──────┐       │ (Published)   │    │
│  │   Binance   │     │    Gap      │       └─────┬─────────┘    │
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

### Correctness Guarantees

| Guarantee | Mechanism |
|-----------|-----------|
| **No missed updates** | Sequence ID validation with gap detection |
| **No stale reads** | Atomic snapshot publishing via `ArcSwap` |
| **No data races** | Single-writer pattern; readers get immutable snapshots |
| **Automatic recovery** | Transparent re-sync on sequence gaps or disconnects |

---

## Performance

### Benchmark Results

Benchmarks run on release builds using [Criterion](https://github.com/bheisler/criterion.rs):

| Operation | Latency |
|-----------|---------|
| Snapshot construction (10k levels) | ~3.25 ms|
| Update batch (100 × 100 levels) | ~1.29 ms|
| High-churn updates (1000 × 10 levels) | ~1.38 ms|
| Top-of-book query | ~23 ns|


### Latency Considerations

| Factor | Impact |
|--------|--------|
| **Network** | Primary bottleneck; Binance streams from Asia |
| **UK Deployment** | Expect 100-250ms network latency |
| **Processing** | Sub-millisecond for most operations |

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

## Configuration

Configuration is managed via `config.toml` in the project root. If the file is missing, defaults are used.

---

## Controls

| Key | Action |
|-----|--------|
| `q` / `Esc` | Quit application |
| `f` | Freeze/Pause the interface* |
| `↑` / `↓` | Increase/decrease time between TUI frame updates |

*Note: Only the interface is paused; the engine thread continues running.

---

## Project Structure

```
src/
├── main.rs, lib.rs, config.rs     # Entry point & configuration
├── binance/                       # WebSocket stream, REST snapshots, data structures for received messages
├── book/                          # Orderbook data structure & sync layer
├── engine/                        # Runtime event loop & state management
├── tui/                           # TUI rendering
└── benches/                       # Criterion benchmarks
```

---

## Logging & Debugging

Logs are written to `logs/` with daily rotation:

```bash
# View latest logs
tail -f logs/ingestor.log.$(date +%Y-%m-%d)

# Run with debug logging
RUST_LOG=debug ./target/release/binance-market-terminal BTCUSDT

# Run with trace logging (very verbose)
RUST_LOG=trace cargo run -- BTCUSDT
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| TLS/SSL errors | Install OpenSSL dev libraries (see Quick Start) |
| Connection timeouts | Check internet; Binance may be rate-limiting |
| High latency | Expected for non-Asian deployments (~100-250ms from UK deployment) |

---

## Contributing

Contributions are welcome. Please follow these steps:

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

- [Binance API](https://developers.binance.com/docs/binance-spot-api-docs) for market data
- [Ratatui](https://ratatui.rs/) for the terminal UI framework
- [Criterion](https://github.com/criterion-rs/criterion.rs) for benchmarking

---

<div align="center">

**[⬆ Back to Top](#binance-market-terminal)**

</div>
