use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::{book::scaler::Scaler, engine::state::MarketSnapshot};

pub fn render(frame: &mut Frame, app_data: &super::App) {
    let snapshot = app_data.state.load();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, chunks[0], &app_data.state.symbol, &snapshot, app_data.frozen, app_data.start_time.elapsed());
    render_main(frame, chunks[1], &app_data.state.scaler, &snapshot);
    render_footer(frame, chunks[2], app_data.update_interval_ms);
}

fn render_header(
    frame: &mut Frame,
    area: Rect,
    symbol: &str,
    snapshot: &MarketSnapshot,
    frozen: bool,
    uptime: std::time::Duration,
) {
    let metrics = &snapshot.metrics;

    let status = if frozen {
        Span::styled("FROZEN", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))
    } else if snapshot.is_syncing {
        Span::styled("SYNCING", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("LIVE", Style::default().fg(Color::Green))
    };

    let format_symbol = Span::styled(symbol, Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    let format_lag = |net_lag_ms: Option<u64>, total_lag_ms: Option<u64>| -> Span {
        match (net_lag_ms, total_lag_ms) {
            (Some(net), Some(total)) => {
                let color = if total < 50 {
                    Color::Green
                } else if total < 200 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                Span::styled(format!("{}/{} ms", net, total), Style::default().fg(color))
            }
            (None, Some(total)) => {
                let color = if total < 50 {
                    Color::Green
                } else if total < 200 {
                    Color::Yellow
                } else {
                    Color::Red
                };
                Span::styled(format!("--/{} ms", total), Style::default().fg(color))
            }
            (Some(net), None) => Span::styled(format!("{}/-- ms", net), Style::default().fg(Color::DarkGray)),
            (None, None) => Span::styled("--/-- ms", Style::default().fg(Color::DarkGray)),
        }
    };

    let left_header_text = vec![
        Line::from(vec![
            format_symbol,
            Span::raw(" | "),
            status,
            Span::raw(" | "),
            Span::raw("Book (net/total): "),
            format_lag(metrics.orderbook_network_lag_ms, metrics.orderbook_lag_ms),
            Span::raw(" | "),
            Span::raw("Trade (net/total): "),
            format_lag(metrics.trade_network_lag_ms, metrics.trade_lag_ms),
            Span::raw(" | "),
            Span::raw(format!("{:.0}/s", metrics.updates_per_second)),
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
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(area);

    let left_header = Paragraph::new(left_header_text)
        .block(Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .title("Market Data Engine"));

    let right_header = Paragraph::new(right_header_text)
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM));

    frame.render_widget(left_header, header_chunks[0]);
    frame.render_widget(right_header, header_chunks[1]);
}

fn render_main(frame: &mut Frame, area: Rect, scaler: &Scaler, snapshot: &MarketSnapshot) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_orderbook(frame, chunks[0], scaler, snapshot);
    render_trade_flow(frame, chunks[1], snapshot);
}

fn render_orderbook(frame: &mut Frame, area: Rect, scaler: &Scaler, snapshot: &MarketSnapshot) {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled("  ASKS", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" ".repeat(7)),
        Span::styled("Price", Style::default().add_modifier(Modifier::UNDERLINED)),
        Span::raw(" ".repeat(7)),
        Span::styled("Quantity", Style::default().add_modifier(Modifier::UNDERLINED)),
    ]));

    let (bids, asks) = snapshot.top_n_depth(5, scaler);

    for (price, qty) in asks.iter().rev() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Red)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 2), Style::default().fg(Color::DarkGray)),
    ]));

    for (price, qty) in bids.iter() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Green)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("  BIDS", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 4), Style::default().fg(Color::DarkGray)),
    ]));

    let metrics = &snapshot.metrics;

    lines.push(Line::from(vec![
        Span::raw("  Mid Price:  "),
        Span::styled(
            format_opt_decimal(metrics.mid_price, 2),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Spread:     "),
        Span::styled(
            format_opt_decimal(metrics.spread, 4),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
    ]));

    let imbalance_color = metrics.imbalance_ratio.map(|ratio| {
        if ratio > rust_decimal::Decimal::ZERO { Color::Green }
        else if ratio < rust_decimal::Decimal::ZERO { Color::Red }
        else { Color::White }
    }).unwrap_or(Color::White);

    lines.push(Line::from(vec![
        Span::raw("  Imbalance:  "),
        Span::styled(
            format_opt_decimal(metrics.imbalance_ratio, 3),
            Style::default().fg(imbalance_color).add_modifier(Modifier::BOLD),
        ),
    ]));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Order Book (L2)"));

    frame.render_widget(paragraph, area);
}

fn render_trade_flow(frame: &mut Frame, area: Rect, snapshot: &MarketSnapshot) {
    const MAX_TRADE_HISTORY_DISPLAY: u16 = 15;

    let metrics = &snapshot.metrics;
    let recent_trades = &snapshot.recent_trades;

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
    
    // Significant trades section
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Significant Trades", Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)),
    ]));

    let significant_trades = &snapshot.significant_trades;
    if significant_trades.is_empty() {
        lines.push(Line::from(vec![
            Span::raw("  No significant trades"),
        ]));
    } else {
        for sig_trade in significant_trades.iter().rev().take(5) {
            let (side_text, side_color) = match sig_trade.side() {
                crate::binance::types::Side::Buy => ("BUY ", Color::Green),
                crate::binance::types::Side::Sell => ("SELL", Color::Red),
            };
            
            let reason_text = sig_trade.significance_reason.display();
            
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(side_text, Style::default().fg(side_color).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(format!("{:>10}", sig_trade.trade.price), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Span::raw("  │  "),
                Span::styled(format!("{:>10}", sig_trade.trade.quantity), Style::default().add_modifier(Modifier::BOLD)),
            ]));
            
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("↳ ", Style::default().fg(Color::DarkGray)),
                Span::styled(reason_text, Style::default().fg(Color::Yellow)),
                Span::raw("  Notional: "),
                Span::styled(format!("{:.2}", sig_trade.notional_value), Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    // Add separator and total trades at bottom
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("─".repeat(area.width as usize - 4), Style::default().fg(Color::DarkGray)),
    ]));
    
    // Trade metrics
    let buy_percent = metrics.buy_ratio_1m.map(|a| (a * 100.0).round() as u32);
    let sell_percent = buy_percent.map(|a| 100 - a);
    
    let volume_str = if metrics.volume_1m >= rust_decimal::Decimal::from(1000) {
        format!("{:.2}", metrics.volume_1m)
    } else {
        format!("{:.4}", metrics.volume_1m)
    };
    
    lines.push(Line::from(vec![
        Span::raw("  Volume (1m): "),
        Span::styled(volume_str, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  │  VWAP (1m): "),
        Span::styled(format_opt_decimal(metrics.vwap_1m, 2), Style::default().fg(Color::Yellow)),
    ]));
    
    lines.push(Line::from(vec![
        Span::raw("  Trades (1m): "),
        Span::styled(format!("{}", metrics.trade_count_1m), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  │  Buy/Sell: "),
        Span::styled(format!("{}%", format_opt_int(buy_percent)), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" │ "),
        Span::styled(format!("{}%", format_opt_int(sell_percent)), Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
    ]));
    
    lines.push(Line::from(vec![
        Span::raw("  Total Trades: "),
        Span::styled(format!("{}", metrics.total_trades), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
    ]));
    
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Trade Flow"));

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
    let left_footer = Paragraph::new("'q' or 'Esc' to quit | 'f' to freeze/unfreeze | '↑/↓' to adjust display speed ");

    let right_footer = Paragraph::new(format!("Display update interval: ({}ms)", update_interval_ms))
        .alignment(ratatui::layout::Alignment::Right);

    frame.render_widget(left_footer, footer_chunks[0]);
    frame.render_widget(right_footer, footer_chunks[1]);
}

fn duration_to_string(dur: std::time::Duration) -> String {
    let total_secs = dur.as_secs();
    let seconds = total_secs % 60;
    let minutes = (total_secs / 60) % 60;
    let hours = total_secs / 3600;

    if hours > 0 {
        format!("{}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{}:{:02}", minutes, seconds)
    }
}

fn format_opt_decimal(opt: Option<rust_decimal::Decimal>, precision: u32) -> String {
    opt.map(|d| format!("{:.1$}", d, precision as usize))
        .unwrap_or_else(|| "--".to_string())
}

fn format_opt_int<T: std::fmt::Display>(opt: Option<T>) -> String {
    opt.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string())
}
