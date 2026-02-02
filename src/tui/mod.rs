//! Terminal User Interface module
//!
//! Provides:
//! - Interactive test explorer with tree view
//! - Real-time test output viewer
//! - Keyboard controls for navigation and test execution

mod app;
mod events;
mod ui;
mod widgets;

pub use app::*;
pub use events::*;
pub use ui::*;
pub use widgets::*;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::path::Path;

use crate::db::Database;

/// Run the TUI application
pub fn run_tui(project_dir: &Path, db: Option<Database>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new(project_dir, db);

    // Auto-discover tests on startup
    let _ = app.discover_tests();

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

        // Update test status if running
        app.update();
    }

    Ok(())
}
