use std::{sync::Arc};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::{engine::state::MarketState};

pub fn render(frame: &mut Frame, app_data: &super::App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(15),    // Order book and trades
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    render_header(frame, chunks[0], &app_data.state, app_data.frozen, app_data.start_time.elapsed());
    render_main(frame, chunks[1], &app_data.state);
    render_footer(frame, chunks[2], app_data.update_interval_ms);
    
}

fn duration_to_string(dur: std::time::Duration) -> String {
    let total_secs = dur.as_secs();


    let seconds = total_secs % 60;
    let minutes = (total_secs / 60) % 60;
    let hours   = total_secs / 3600;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }

}

fn render_header(frame: &mut Frame, area: Rect, state: &Arc<MarketState>, frozen: bool, uptime: std::time::Duration) {
    // Try to read metrics, fallback to defaults if lock is held
    let (orderbook_lag_ms, trade_lag_ms, updates_per_second) = state.metrics.try_read()
        .map(|m| (m.orderbook_lag_ms, m.trade_lag_ms, m.updates_per_second))
        .unwrap_or((None, None, 0.0));
    
    let is_syncing = state.is_syncing.try_read()
        .map(|guard| *guard)
        .unwrap_or(true);
    
    let status = if frozen {
        Span::styled("FROZEN", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
    } else if is_syncing {
        Span::styled("SYNCING", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("LIVE", Style::default().fg(Color::Green))
    };
    
    let format_symbol = Span::styled(&state.symbol, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    // Format depth status
    let depth_text = Span::raw("Ok");
    
    // Format lag with color coding
    let format_lag = |lag_ms: Option<u64>| -> Span {
        match lag_ms {
            None => Span::raw("N/A"),
            Some(ms) if ms < 100 => Span::styled(format!("{}ms", ms), Style::default().fg(Color::Green)),
            Some(ms) if ms < 500 => Span::styled(format!("{}ms", ms), Style::default().fg(Color::Yellow)),
            Some(ms) => Span::styled(format!("{}ms", ms), Style::default().fg(Color::Red)),
        }
    };
    
    let left_header_text = vec![
        Line::from(vec![
            format_symbol,
            Span::raw(" | "),
            status,
            Span::raw(" | "),
            Span::raw("Depth: "),
            depth_text,
            Span::raw(" | "),
            Span::raw("Orderbook Lag: "),
            format_lag(orderbook_lag_ms),
            Span::raw(" | "),
            Span::raw("Trade Lag: "),
            format_lag(trade_lag_ms),
            Span::raw(" | "),
            Span::raw(format!("Updates/s: {:.1}", updates_per_second)),
        ]),
    ];
    
    let right_header_text = vec![
        Line::from(vec![
            Span::styled("Uptime: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(duration_to_string(uptime)),
        ]),
    ];
    
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80),
            Constraint::Percentage(20),
        ])
        .split(area);

    let left_header = Paragraph::new(left_header_text)
        .block(Block::default()
        .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
        .title("Market Data Engine"));

     let right_header = Paragraph::new(right_header_text)
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::default()
        .borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM));

    
    frame.render_widget(left_header, header_chunks[0]);
    frame.render_widget(right_header, header_chunks[1]);
}

fn render_main(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);
    
    // Left panel: Order Book
    render_orderbook(frame, chunks[0], state);
    
    // Right panel: Trade Flow
    render_trade_flow(frame, chunks[1], state);
}

fn render_orderbook(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let mut lines = vec![];
    
    // ASK header
    lines.push(Line::from(vec![
        Span::styled("  ASKS", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(7)),
        Span::styled("Price", Style::default().add_modifier(Modifier::UNDERLINED)),
        Span::raw(" ".repeat(7)),
        Span::styled("Quantity", Style::default().add_modifier(Modifier::UNDERLINED)),
    ]));
    
    // Get top 5 level bids and asks as decimals
    let (bids, asks) = state.top_n_depth(5);

    // Display asks (top 5, reversed so highest ask is at top)
    for (price, qty) in asks.iter().rev() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Red)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }
    
    // Separator
    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 2), Style::default().fg(Color::DarkGray)),
    ]));
    
    // Display bids (top 5)
    for (price, qty) in bids.iter() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Green)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }
    
    // BID footer
    lines.push(Line::from(vec![
        Span::styled("  BIDS", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]));
    
    // Add separator and metrics
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 4), Style::default().fg(Color::DarkGray)),
    ]));
    
    // Try read, use defaults otherwise
    let (mid_price, spread, imbalance_ratio) = state.metrics.try_read()
        .map(|m| (m.mid_price, m.spread, m.imbalance_ratio))
        .unwrap_or((None, None, None));
    
    lines.push(Line::from(vec![
        Span::raw("  Mid Price:  "),
        Span::styled(format_opt_decimal(mid_price, 2), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ]));
    
    lines.push(Line::from(vec![
        Span::raw("  Spread:     "),
        Span::styled(format_opt_decimal(spread, 4), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    ]));
    
    let imbalance_color = imbalance_ratio.map(|ratio| {
        if ratio > rust_decimal::Decimal::from(0) {
            Color::Green
        } else if ratio < rust_decimal::Decimal::from(0) {
            Color::Red
        } else {
            Color::White
        }
    }).unwrap_or(Color::White);
    
    lines.push(Line::from(vec![
        Span::raw("  Imbalance:  "),
        Span::styled(format_opt_decimal(imbalance_ratio, 3), Style::default().fg(imbalance_color).add_modifier(Modifier::BOLD)),
    ]));
    
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Order Book (L2)"));
    
    frame.render_widget(paragraph, area);
}

fn render_trade_flow(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    const MAX_TRADE_HISTORY_DISPLAY: u16 = 15;
    
    // Try to read metrics, use defaults if lock is held
    let (volume_1m, vwap_1m, trade_count_1m, buy_ratio_1m, total_trades) = state.metrics.try_read()
        .map(|m| (m.volume_1m, m.vwap_1m, m.trade_count_1m, m.buy_ratio_1m, m.total_trades))
        .unwrap_or((rust_decimal::Decimal::ZERO, None, 0, None, 0));
    
    let recent_trades = state.recent_trades.load();
    
    // Calculate available lines: area height - 2 for borders - 1 for header - 1 for last trade section
    let available_lines = (area.height.saturating_sub(5)).min(MAX_TRADE_HISTORY_DISPLAY) as usize;
    
    let mut lines = vec![];
    
    // Last trade section with better formatting
    lines.push(Line::from(vec![
        Span::styled("Last Trade", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
    ]));

    let last_trade = recent_trades.iter().last();
    
    if let Some(trade) = last_trade {
        let side = recent_trades.back().map(|t| t.side()).unwrap_or(crate::binance::types::Side::Buy);
        let (side_text, side_color) = match side {
            crate::binance::types::Side::Buy => ("BUY ", Color::Green),
            crate::binance::types::Side::Sell => ("SELL", Color::Red),
        };
        
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(side_text, Style::default().fg(side_color).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(format!("{}", trade.price), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  │  "),
            Span::raw(format!("{}", trade.quantity)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  No trades yet"),
        ]));
    }
    
    lines.push(Line::from(""));
    
    // Get the most recent trades that fit in available space
    let trades_to_display = recent_trades.len().min(available_lines);
    
    // Display trades from newest to oldest, skipping the most recent one (already shown above)
    for trade in recent_trades.iter().rev().skip(1).take(trades_to_display) {
        let (side_text, side_color) = match trade.side() {
            crate::binance::types::Side::Buy => ("BUY ", Color::Green),
            crate::binance::types::Side::Sell => ("SELL", Color::Red),
        };
        
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(side_text, Style::default().fg(side_color)),
            Span::raw(" "),
            Span::styled(format!("{:>10}", trade.price), Style::default().fg(Color::Cyan)),
            Span::raw("  │  "),
            Span::raw(format!("{:>10}", trade.quantity)),
        ]));
    }
    
    // Add separator and total trades at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 4), Style::default().fg(Color::DarkGray)),
    ]));
    
    // Trade metrics
    let buy_percent = buy_ratio_1m.map(|a| (a * 100.0).round() as u32);
    let sell_percent = buy_percent.map(|a| 100 - a);
    
    let volume_str = if volume_1m >= rust_decimal::Decimal::from(1000) {
        format!("{:.2}", volume_1m)
    } else {
        format!("{:.4}", volume_1m)
    };
    
    lines.push(Line::from(vec![
        Span::raw("  Volume (1m): "),
        Span::styled(volume_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  │  VWAP (1m): "),
        Span::styled(format_opt_decimal(vwap_1m, 2), Style::default().fg(Color::Yellow)),
    ]));
    
    lines.push(Line::from(vec![
        Span::raw("  Trades (1m): "),
        Span::styled(format!("{}", trade_count_1m), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  │  Buy/Sell: "),
        Span::styled(format!("{}%", format_opt_int(buy_percent)), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(format!("{}%", format_opt_int(sell_percent)), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]));
    
    lines.push(Line::from(vec![
        Span::raw("  Total Trades: "),
        Span::styled(format!("{}", total_trades), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Recent Trades"));
    
    frame.render_widget(paragraph, area);
}

fn render_footer(frame: &mut Frame, area: Rect, update_interval_ms: u64) {
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(80),
            Constraint::Percentage(20),
        ])
        .split(area);
    let left_footer = Paragraph::new("Press 'q' or 'Esc' to quit | Press 'f' to freeze/unfreeze | Press '↑/↓' to adjust display speed ");

    let right_footer = Paragraph::new(format!("Display update interval: ({}ms)", update_interval_ms))
        .alignment(ratatui::layout::Alignment::Right);

    frame.render_widget(left_footer, footer_chunks[0]);
    frame.render_widget(right_footer, footer_chunks[1]);
}

fn format_opt_int<T: std::fmt::Display>(opt: Option<T>) -> String {
    opt.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string())
}

fn format_opt_decimal(opt: Option<rust_decimal::Decimal>, precision: u32) -> String {
    opt.map(|d| {
        let rounded = d.round_dp(precision);
        rounded.normalize().to_string()
    }).unwrap_or_else(|| "N/A".to_string())
}
