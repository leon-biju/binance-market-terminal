use std::sync::Arc;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use crate::engine::state::MarketState;

pub fn render(frame: &mut Frame, state: &Arc<MarketState>) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Footer
        ])
        .split(frame.area());

    render_header(frame, chunks[0], state);
    render_main(frame, chunks[1], state);
    render_footer(frame, chunks[2]);
}

fn render_header(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let metrics = state.metrics.load();
    
    let is_syncing = state.is_syncing.try_read()
        .map(|guard| *guard)
        .unwrap_or(true);
    
    let status = if is_syncing {
        Span::styled("SYNCING", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("LIVE", Style::default().fg(Color::Green))
    };
    
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
    
    let header_text = vec![
        Line::from(vec![
            Span::from(&state.symbol),
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
    
    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Market Data Engine"));
    
    frame.render_widget(header, area);
}

fn render_main(frame: &mut Frame, area: Rect, state: &Arc<MarketState>) {
    let metrics = state.metrics.load();
    
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(area);
    
    // Left panel: Orderbook metrics
    render_orderbook_panel(frame, chunks[0], &metrics);
    
    // Right panel: Trade metrics
    render_trade_panel(frame, chunks[1], &metrics);
}

fn render_orderbook_panel(frame: &mut Frame, area: Rect, metrics: &crate::engine::metrics::MarketMetrics) {
    let items = vec![
        format!("Best Bid:  {:>12}", format_opt_decimal(metrics.best_bid)),
        format!("Best Ask:  {:>12}", format_opt_decimal(metrics.best_ask)),
        format!("Spread:    {:>12}", format_opt_decimal(metrics.spread)),
        format!("Mid Price: {:>12}", format_opt_decimal(metrics.mid_price)),
        format!("Imbalance: {:>12}", format_opt_decimal(metrics.imbalance_ratio)),
    ];
    
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(s.as_str()))
        .collect();
    
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Orderbook"));
    
    frame.render_widget(list, area);
}

fn render_trade_panel(frame: &mut Frame, area: Rect, metrics: &crate::engine::metrics::MarketMetrics) {
    let items = vec![
        format!("Last Price:  {:>12}", format_opt_decimal(metrics.last_price)),
        format!("Last Qty:    {:>12}", format_opt_decimal(metrics.last_qty)),
        format!("Volume (1m): {:>12}", metrics.volume_1m),
        format!("Trades (1m): {:>12}", metrics.trade_count_1m),
        format!("VWAP (1m):   {:>12}", format_opt_decimal(metrics.vwap_1m)),
    ];
    
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|s| ListItem::new(s.as_str()))
        .collect();
    
    let list = List::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Trades"));
    
    frame.render_widget(list, area);
}

fn render_footer(frame: &mut Frame, area: Rect) {
    let footer = Paragraph::new("Press 'q' or 'Esc' to quit")
        .block(Block::default().borders(Borders::ALL));
    
    frame.render_widget(footer, area);
}

fn format_opt_decimal(opt: Option<rust_decimal::Decimal>) -> String {
    opt.map(|d| d.to_string()).unwrap_or_else(|| "N/A".to_string())
}
