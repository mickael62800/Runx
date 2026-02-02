//! Terminal User Interface module
//!
//! Provides:
//! - Interactive task list
//! - Real-time log viewer
//! - Keyboard controls

mod app;
mod events;
mod ui;
mod widgets;

pub use app::*;
pub use events::*;
pub use ui::*;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::path::Path;

use crate::config::Config;
use crate::db::Database;

/// Run the TUI application
pub fn run_tui(config: &Config, base_dir: &Path, db: Option<Database>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(config, base_dir, db);

    // Run main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Main application loop
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if let Some(event) = events::poll_event()? {
            match events::handle_event(event, app) {
                EventResult::Continue => {}
                EventResult::Quit => break,
            }
        }

        // Update task status if running
        app.update();
    }

    Ok(())
}
