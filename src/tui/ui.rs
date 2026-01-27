use crate::{book::scaler::Scaler, config::Config, engine::state::MarketSnapshot};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

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

    render_header(
        frame,
        chunks[0],
        &app_data.state.symbol,
        &snapshot,
        app_data.frozen,
        app_data.start_time.elapsed(),
    );
    render_main(
        frame,
        chunks[1],
        &app_data.state.scaler,
        &snapshot,
        &app_data.config,
    );
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
        Span::styled(
            "FROZEN",
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
    } else if snapshot.is_syncing {
        Span::styled("SYNCING", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("LIVE", Style::default().fg(Color::Green))
    };

    let format_symbol = Span::styled(
        symbol,
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    );

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
            (Some(net), None) => Span::styled(
                format!("{}/-- ms", net),
                Style::default().fg(Color::DarkGray),
            ),
            (None, None) => Span::styled("--/-- ms", Style::default().fg(Color::DarkGray)),
        }
    };

    let left_header_text = vec![Line::from(vec![
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
    ])];

    let right_header_text = vec![Line::from(vec![
        Span::styled("Uptime: ", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(duration_to_string(uptime)),
    ])];

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(area);

    let left_header = Paragraph::new(left_header_text).block(
        Block::default()
            .borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM)
            .title("Binance Market Terminal"),
    );

    let right_header = Paragraph::new(right_header_text)
        .alignment(ratatui::layout::Alignment::Right)
        .block(Block::default().borders(Borders::RIGHT | Borders::TOP | Borders::BOTTOM));

    frame.render_widget(left_header, header_chunks[0]);
    frame.render_widget(right_header, header_chunks[1]);
}

fn render_main(
    frame: &mut Frame,
    area: Rect,
    scaler: &Scaler,
    snapshot: &MarketSnapshot,
    config: &Config,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    render_orderbook(
        frame,
        chunks[0],
        scaler,
        snapshot,
        config.orderbook_depth_display_count,
    );
    render_trade_flow(frame, chunks[1], snapshot, config);
}

fn render_orderbook(
    frame: &mut Frame,
    area: Rect,
    scaler: &Scaler,
    snapshot: &MarketSnapshot,
    orderbook_depth_display_count: usize,
) {
    let mut lines = vec![];

    lines.push(Line::from(vec![
        Span::styled(
            "  ASKS",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ".repeat(7)),
        Span::styled("Price", Style::default().add_modifier(Modifier::UNDERLINED)),
        Span::raw(" ".repeat(7)),
        Span::styled(
            "Quantity",
            Style::default().add_modifier(Modifier::UNDERLINED),
        ),
    ]));

    let (bids, asks) = snapshot.top_n_depth(orderbook_depth_display_count, scaler);

    for (price, qty) in asks.iter().rev() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Red)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }

    lines.push(Line::from(vec![Span::styled(
        "─".repeat(area.width as usize - 2),
        Style::default().fg(Color::DarkGray),
    )]));

    for (price, qty) in bids.iter() {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{:>12}", price), Style::default().fg(Color::Green)),
            Span::raw("  │  "),
            Span::raw(format!("{:>12}", qty)),
        ]));
    }

    lines.push(Line::from(vec![Span::styled(
        "  BIDS",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )]));

    lines.push(Line::from(""));
    lines.push(Line::from(vec![Span::styled(
        "─".repeat(area.width as usize - 4),
        Style::default().fg(Color::DarkGray),
    )]));

    let metrics = &snapshot.metrics;

    lines.push(Line::from(vec![
        Span::raw("  Mid Price:  "),
        Span::styled(
            format_opt_decimal(metrics.mid_price, 2),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::raw("  Spread:     "),
        Span::styled(
            format_opt_decimal(metrics.spread, 4),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let imbalance_color = metrics
        .imbalance_ratio
        .map(|ratio| {
            if ratio > rust_decimal::Decimal::ZERO {
                Color::Green
            } else if ratio < rust_decimal::Decimal::ZERO {
                Color::Red
            } else {
                Color::White
            }
        })
        .unwrap_or(Color::White);

    lines.push(Line::from(vec![
        Span::raw("  Imbalance:  "),
        Span::styled(
            format_opt_decimal(metrics.imbalance_ratio, 3),
            Style::default()
                .fg(imbalance_color)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Order Book (L2)"),
    );

    frame.render_widget(paragraph, area);
}

fn render_trade_flow(frame: &mut Frame, area: Rect, snapshot: &MarketSnapshot, config: &Config) {
    let metrics = &snapshot.metrics;
    let recent_trades = &snapshot.recent_trades;
    let significant_trades = &snapshot.significant_trades;

    // Calculate available space for tables
    let recent_trades_count = recent_trades.len().min(config.recent_trades_display_count);
    let sig_trades_count = significant_trades
        .len()
        .min(config.significant_trades_display_count);

    // Split area into sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((recent_trades_count + 2) as u16), // Recent trades table + header
            Constraint::Length(1),                                // Gap
            Constraint::Length((sig_trades_count + 2) as u16), // Significant trades table + header
            Constraint::Length(5),                             // Metrics section
            Constraint::Min(0),                                // Spacer
        ])
        .split(
            Block::default()
                .borders(Borders::ALL)
                .title("Trade Flow")
                .inner(area),
        );

    // Render the outer block
    frame.render_widget(
        Block::default().borders(Borders::ALL).title("Trade Flow"),
        area,
    );

    // Recent Trades Table
    let recent_header = Row::new(vec![
        Cell::from("Side").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Price").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Quantity").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let recent_rows: Vec<Row> = recent_trades
        .iter()
        .rev()
        .take(recent_trades_count)
        .map(|trade| {
            let (side_text, side_color) = match trade.side() {
                crate::binance::types::Side::Buy => ("BUY", Color::Green),
                crate::binance::types::Side::Sell => ("SELL", Color::Red),
            };
            Row::new(vec![
                Cell::from(side_text).style(Style::default().fg(side_color)),
                Cell::from(format!("{}", trade.price)).style(Style::default().fg(Color::Cyan)),
                Cell::from(format!("{}", trade.quantity)),
            ])
        })
        .collect();

    let recent_table = Table::new(
        recent_rows,
        [
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Length(14),
        ],
    )
    .header(recent_header)
    .block(Block::default().title(Span::styled(
        format!("Recent Trades ({})", recent_trades_count),
        Style::default().add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(recent_table, chunks[0]);

    // Gap between tables is at chunks[1]

    // Significant Trades Table
    let sig_header = Row::new(vec![
        Cell::from("Side").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Price").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Quantity").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Reason").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Notional").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().add_modifier(Modifier::UNDERLINED));

    let sig_rows: Vec<Row> = significant_trades
        .iter()
        .rev()
        .take(config.significant_trades_display_count)
        .map(|sig_trade| {
            let (side_text, side_color) = match sig_trade.side() {
                crate::binance::types::Side::Buy => ("BUY", Color::Green),
                crate::binance::types::Side::Sell => ("SELL", Color::Red),
            };
            Row::new(vec![
                Cell::from(side_text)
                    .style(Style::default().fg(side_color).add_modifier(Modifier::BOLD)),
                Cell::from(format!("{}", sig_trade.trade.price)).style(
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(format!("{}", sig_trade.trade.quantity))
                    .style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(sig_trade.significance_reason.display())
                    .style(Style::default().fg(Color::Yellow)),
                Cell::from(format!("{:.2}", sig_trade.notional_value))
                    .style(Style::default().fg(Color::Cyan)),
            ])
        })
        .collect();

    let sig_table = Table::new(
        sig_rows,
        [
            Constraint::Length(6),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Length(14),
        ],
    )
    .header(sig_header)
    .block(Block::default().title(Span::styled(
        format!("Significant Trades ({})", sig_trades_count),
        Style::default().add_modifier(Modifier::BOLD),
    )));

    frame.render_widget(sig_table, chunks[2]);

    // Trade Metrics Section
    let buy_percent = metrics.buy_ratio_1m.map(|a| (a * 100.0).round() as u32);
    let sell_percent = buy_percent.map(|a| 100 - a);

    let volume_str = if metrics.volume_1m >= rust_decimal::Decimal::from(1000) {
        format!("{:.2}", metrics.volume_1m)
    } else {
        format!("{:.4}", metrics.volume_1m)
    };

    let metrics_rows = vec![
        Row::new(vec![
            Cell::from("Volume (1m)"),
            Cell::from(volume_str).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("VWAP (1m)"),
            Cell::from(format_opt_decimal(metrics.vwap_1m, 2))
                .style(Style::default().fg(Color::Yellow)),
        ]),
        Row::new(vec![
            Cell::from("Trades (1m)"),
            Cell::from(format!("{}", metrics.trade_count_1m)).style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from("Buy / Sell"),
            Cell::from(Line::from(vec![
                Span::styled(
                    format!("{}%", format_opt_int(buy_percent)),
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" / "),
                Span::styled(
                    format!("{}%", format_opt_int(sell_percent)),
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
            ])),
        ]),
        Row::new(vec![
            Cell::from("Total Trades"),
            Cell::from(format!("{}", metrics.total_trades)).style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Cell::from(""),
            Cell::from(""),
        ]),
    ];

    let metrics_table = Table::new(
        metrics_rows,
        [
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(12),
            Constraint::Length(14),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(metrics_table, chunks[3]);
}

fn render_footer(frame: &mut Frame, area: Rect, update_interval_ms: u64) {
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
        .split(area);
    let left_footer = Paragraph::new(
        "'q' or 'Esc' to quit | 'f' to freeze/unfreeze | '↑/↓' to adjust display speed ",
    );

    let right_footer = Paragraph::new(format!(
        "Display update interval: ({}ms)",
        update_interval_ms
    ))
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
    opt.map(|v| v.to_string())
        .unwrap_or_else(|| "N/A".to_string())
}
