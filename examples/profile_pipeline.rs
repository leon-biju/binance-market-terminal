/// Profile the full data pipeline: WebSocket → Engine → TUI render
///
/// Produces:
///   - CPU flamegraph (SVG + protobuf) via pprof sampling
///   - Wall-clock span trace (JSON) via tracing-chrome, viewable in chrome://tracing or https://ui.perfetto.dev
///
/// Usage:
///   cargo run --example profile_pipeline --release -- BTCUSDT [duration_secs]
///
/// Defaults: symbol=BTCUSDT (its the most popular stream i think deal with it), duration=30s
use anyhow::Result;
use binance_market_terminal::{
    binance::{exchange_info, snapshot},
    book::scaler::Scaler,
    config,
    engine::runtime::{EngineCommand, MarketDataEngine},
    tui::App,
};
use pprof::ProfilerGuardBuilder;
use pprof::protos::Message;
use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing_chrome::ChromeLayerBuilder;
use tracing_subscriber::prelude::*;

const DEFAULT_DURATION_SECS: u64 = 30;
const RENDER_INTERVAL_MS: u64 = 500;

#[tokio::main]
async fn main() -> Result<()> {
    let out_dir = "target/flamegraphs";
    fs::create_dir_all(out_dir)?;

    // Parse args
    let symbol = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "BTCUSDT".to_string());
    let duration_secs: u64 = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_DURATION_SECS);

    // ── Tracing: wall-clock span timing via chrome trace ──
    let (chrome_layer, _chrome_guard) = ChromeLayerBuilder::new()
        .file(format!("{}/pipeline_trace.json", out_dir))
        .include_args(true)
        .build();

    tracing_subscriber::registry()
        .with(chrome_layer)
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .init();

    // ── TLS provider ──
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");

    println!(
        "⏱  Profiling pipeline for {} ({} seconds)...",
        symbol, duration_secs
    );

    // ── Startup: real snapshot + exchange info ──
    let conf = Arc::new(config::load_config());

    let startup = Instant::now();
    let snapshot_data =
        snapshot::fetch_snapshot(&symbol, conf.orderbook_initial_snapshot_depth).await?;
    println!(
        "   Snapshot fetched in {:.1}ms (lastUpdateId: {})",
        startup.elapsed().as_secs_f64() * 1000.0,
        snapshot_data.last_update_id
    );

    let (tick_size, step_size) = exchange_info::fetch_tick_and_step_sizes(&symbol).await?;
    println!(
        "   Exchange info fetched in {:.1}ms",
        startup.elapsed().as_secs_f64() * 1000.0
    );

    let scaler = Scaler::new(tick_size, step_size);

    let (engine, command_tx, state) =
        MarketDataEngine::prepare(symbol.clone(), snapshot_data, scaler.clone(), conf.clone())?;

    // ── CPU profiler: pprof sampling ──
    let guard = ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .unwrap();

    // ── Spawn engine (real WebSocket connection) ──
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = engine.run().await {
            tracing::error!("Engine error: {}", e);
        }
    });

    println!(
        "   Engine started in {:.1}ms",
        startup.elapsed().as_secs_f64() * 1000.0
    );

    // ── Headless TUI render loop ──
    // Exercises the exact same render path as the real TUI,
    // but using a TestBackend (no terminal required).
    let app = App::new(state, conf);
    let profile_start = Instant::now();
    let mut render_count: u64 = 0;
    let mut first_live_render: Option<Duration> = None;

    while profile_start.elapsed() < Duration::from_secs(duration_secs) {
        let _render_span = tracing::trace_span!("headless_render_cycle").entered();
        app._render_headless()?;
        render_count += 1;

        // Detect first render with live (non-syncing) data
        if first_live_render.is_none() {
            let snap = app.state.load();
            if !snap.is_syncing && snap.top_n_depth(&scaler).0.len() > 0 {
                first_live_render = Some(profile_start.elapsed());
                println!(
                    "   ✓ First live data rendered at {:.1}ms",
                    first_live_render.unwrap().as_secs_f64() * 1000.0
                );
            }
        }

        tokio::time::sleep(Duration::from_millis(RENDER_INTERVAL_MS)).await;
    }

    println!(
        "\n   {} headless renders in {} seconds",
        render_count, duration_secs
    );

    // ── Shutdown ──
    let _ = command_tx.send(EngineCommand::Shutdown).await;
    let _ = engine_handle.await;

    // ── Write CPU flamegraph ──
    if let Ok(report) = guard.report().build() {
        let mut file = File::create(format!("{}/pipeline_flamegraph.svg", out_dir))?;
        report.flamegraph(&mut file)?;
        println!(
            "   CPU flamegraph  → {}/pipeline_flamegraph.svg",
            out_dir
        );

        let pprof_profile = report.pprof().unwrap();
        let mut buf = Vec::new();
        pprof_profile.encode(&mut buf)?;
        let mut proto_file = File::create(format!("{}/pipeline_profile.pb", out_dir))?;
        proto_file.write_all(&buf)?;
        println!(
            "   Protobuf profile → {}/pipeline_profile.pb",
            out_dir
        );
    }

    // Chrome trace is flushed when _chrome_guard drops
    drop(_chrome_guard);
    println!(
        "   Chrome trace     → {}/pipeline_trace.json",
        out_dir
    );
    println!("   Open trace in: chrome://tracing or https://ui.perfetto.dev");

    Ok(())
}
