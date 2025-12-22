use std::{fmt::{Debug, format}, sync::Arc};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::{engine::state::MarketState, tui::app};

pub fn render(frame: &mut Frame, app_data: &super::App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(15),    // Order book and trades
            Constraint::Length(5),  // Market metrics
            Constraint::Length(1),  // Footer
        ])
        .split(frame.area());

    render_header(frame, chunks[0], &app_data.state, app_data.frozen, app_data.start_time.elapsed());
    render_main(frame, chunks[1], &app_data.state);
    render_metrics(frame, chunks[2], &app_data.state);
    render_footer(frame, chunks[3], app_data.update_interval_ms);
    
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
    let metrics = state.metrics.load();
    
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
            //Span::raw(format!(" ({}/{})", metrics.bid_depth, metrics.ask_depth)),
            Span::raw(" | "),
            Span::raw("Orderbook Lag: "),
            format_lag(metrics.orderbook_lag_ms),
            Span::raw(" | "),
            Span::raw("Trade Lag: "),
            format_lag(metrics.trade_lag_ms),
            Span::raw(" | "),
            Span::raw(format!("Updates/s: {:.1}", metrics.updates_per_second)),
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
            Constraint::Percentage(60),
            Constraint::Percentage(40),
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
        Span::styled("  ASK", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]));
    
    // Get top 5 level bids and asks as decimals
    let (bids, asks) = state.top_n_depth(5);

    // Display asks (top 5, reversed so highest ask is at top)
    for (price, qty) in asks.iter().rev() {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<12}", price), Style::default().fg(Color::Red)),
            Span::raw(" | "),
            Span::raw(format!("{:>10}", qty)),
        ]));
    }
    
    // Separator
    lines.push(Line::from(vec![
        Span::raw("─".repeat(area.width as usize - 2)),
    ]));
    
    // Display bids (top 5)
    for (price, qty) in bids.iter() {
        lines.push(Line::from(vec![
            Span::styled(format!(" {:<12}", price), Style::default().fg(Color::Green)),
            Span::raw(" | "),
            Span::raw(format!("{:>10}", qty)),
        ]));
    }
    
    // BID footer
    lines.push(Line::from(vec![
        Span::styled("  BID", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]));
    
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Order Book (Top 10)"));
    
    frame.render_widget(paragraph, area);
}

fn render_trade_flow(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let metrics = state.metrics.load();
    let recent_trades = state.recent_trades.load();
    
    // Calculate available lines: area height - 2 for borders - 1 for header - 1 for last trade section
    let available_lines = (area.height.saturating_sub(5)).min(15) as usize;
    
    let mut lines = vec![];
    
    // Last trade section
    lines.push(Line::from(vec![
        Span::styled("Last Trade:", Style::default().add_modifier(Modifier::BOLD)),
    ]));
    
    if let (Some(price), Some(qty)) = (metrics.last_price, metrics.last_qty) {
        let side = recent_trades.back().map(|t| t.side()).unwrap_or(crate::binance::types::Side::Buy);
        let (side_text, side_color) = match side {
            crate::binance::types::Side::Buy => ("BUY", Color::Green),
            crate::binance::types::Side::Sell => ("SELL", Color::Red),
        };
        
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(format!("{}", price), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::raw(format!("{}", qty)),
            Span::raw("  "),
            Span::styled(side_text, Style::default().fg(side_color)),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::raw("  No trades yet"),
        ]));
    }
    
    lines.push(Line::from(""));
    
    // Get the most recent trades that fit in available space
    let trades_to_display = recent_trades.len().min(available_lines);
    
    // Display trades from newest to oldest so most recent appears at top
    for trade in recent_trades.iter().rev().take(trades_to_display) {
        let (side_text, side_color) = match trade.side() {
            crate::binance::types::Side::Buy => ("BUY", Color::Green),
            crate::binance::types::Side::Sell => ("SELL", Color::Red),
        };
        
        lines.push(Line::from(vec![
            Span::raw(" "),
            Span::styled(format!("{:>9}", trade.price), Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::raw(format!("{:>8}", trade.quantity)),
            Span::raw("  "),
            Span::styled(side_text, Style::default().fg(side_color)),
        ]));
    }
    
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Trade Flow"));
    
    frame.render_widget(paragraph, area);
}

fn render_metrics(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let metrics = state.metrics.load();
    
    let line1 = Line::from(vec![
        Span::raw("Mid: "),
        Span::styled(format_opt_decimal(metrics.mid_price, 3), Style::default().fg(Color::Cyan)),
        Span::raw(" | Spread: "),
        Span::raw(format_opt_decimal(metrics.spread, 3)),
        Span::raw(" | VWAP: "),
        Span::raw(format_opt_decimal(metrics.vwap_1m, 3)),
        Span::raw(" | Imbalance: "),
        Span::raw(format_opt_decimal(metrics.imbalance_ratio, 3)),
        Span::raw(" | Vol: "),
        Span::raw(metrics.volume_1m.to_string()),
    ]);
    
    let buy_percent = metrics.buy_ratio_1m
        .map(|a| (a * 100.0).round() as u32);
    let sell_percent = buy_percent
        .map(|a| 100 - a);

    let line2 = Line::from(vec![
        Span::raw("Trades/s: "),
        Span::raw(format!("{:.1}", metrics.updates_per_second)),
        Span::raw(format!(" | Buy %: {}% | Sell %: {}%", format_opt_int(buy_percent), format_opt_int(sell_percent))),
        Span::raw(" | Last depth update: "),
        Span::raw(format_opt_int(metrics.orderbook_lag_ms)),
        Span::raw("ms ago"),
    ]);
    
    let paragraph = Paragraph::new(vec![line1, line2])
        .block(Block::default().borders(Borders::ALL).title("Market Metrics (1m window)"));
    
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
