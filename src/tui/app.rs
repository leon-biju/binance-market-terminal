use std::time::Duration;
use std::{sync::Arc};
use std::io;
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use crate::engine::state::MarketState;

pub struct App {
    pub state: Arc<MarketState>,
    pub should_quit: bool,
    pub frozen: bool,
    pub update_interval_ms: u64,
    pub start_time: std::time::Instant,
}

impl App {
    
    pub fn new(state: Arc<MarketState>) -> Self {
        Self {
            state,
            should_quit: false,
            frozen: false,
            update_interval_ms: 500,
            start_time: std::time::Instant::now(),
        }
    }

    pub async fn run(&mut self) -> io::Result<()> {
        // sets up panic hook to restore terminal
        let original_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic_info| {
            let _ = Self::restore_terminal();
            original_hook(panic_info);
        }));

        // sets up terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
        stdout.execute(EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        let result = self.run_loop(&mut terminal).await;

        // now program has ended restore the terminal back to normal
        Self::restore_terminal()?;

        // Restore original panic hook
        let _ = std::panic::take_hook();

        result
    }

    fn restore_terminal() -> io::Result<()> {
        let mut stdout = io::stdout();
        disable_raw_mode()?;
        stdout.execute(LeaveAlternateScreen)?;
        stdout.execute(crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
        stdout.execute(crossterm::cursor::Show)?;
        Ok(())
    }

    async fn run_loop<B: ratatui::backend::Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            if !self.frozen {
                terminal.draw(|f| super::ui::render(f, &self))?;
            }

            // Poll for events with timeout
            if event::poll(std::time::Duration::from_millis(self.update_interval_ms))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                self.should_quit = true;
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') => {
                                self.frozen = !self.frozen;
                                terminal.draw(|f| super::ui::render(f, &self))?;
                            }
                            KeyCode::Up => {
                                self.update_interval_ms = (self.update_interval_ms + 100).min(2000);
                            }
                            KeyCode::Down => {
                                self.update_interval_ms = (self.update_interval_ms - 100).max(100);
                            }
                            _ => {}
                        }
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }
}
