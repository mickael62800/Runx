//! TUI event handling

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::App;

/// Result of handling an event
pub enum EventResult {
    Continue,
    Quit,
}

/// Poll for an event with timeout
pub fn poll_event() -> anyhow::Result<Option<Event>> {
    if event::poll(Duration::from_millis(100))? {
        Ok(Some(event::read()?))
    } else {
        Ok(None)
    }
}

/// Handle a keyboard event
pub fn handle_event(event: Event, app: &mut App) -> EventResult {
    match event {
        Event::Key(key) => handle_key_event(key, app),
        Event::Mouse(_) => EventResult::Continue,
        Event::Resize(_, _) => EventResult::Continue,
        _ => EventResult::Continue,
    }
}

fn handle_key_event(key: KeyEvent, app: &mut App) -> EventResult {
    // Handle search mode separately
    if app.search_mode {
        return handle_search_key(key, app);
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => EventResult::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => EventResult::Quit,
        KeyCode::Esc => EventResult::Quit,

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.prev_task();
            EventResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.next_task();
            EventResult::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.selected_task = 0;
            app.log_scroll = 0;
            EventResult::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.selected_task = app.tasks.len().saturating_sub(1);
            app.log_scroll = 0;
            EventResult::Continue
        }

        // Log scrolling
        KeyCode::Left | KeyCode::Char('h') => {
            app.scroll_log_up();
            EventResult::Continue
        }
        KeyCode::Right | KeyCode::Char('l') => {
            app.scroll_log_down();
            EventResult::Continue
        }
        KeyCode::PageUp => {
            app.scroll_log_page_up();
            EventResult::Continue
        }
        KeyCode::PageDown => {
            app.scroll_log_page_down();
            EventResult::Continue
        }

        // Actions
        KeyCode::Char('r') => {
            app.retry_selected();
            EventResult::Continue
        }
        KeyCode::Char('s') => {
            app.skip_selected();
            EventResult::Continue
        }
        KeyCode::Enter => {
            // View task details or run selected
            EventResult::Continue
        }

        // Search
        KeyCode::Char('/') => {
            app.search_mode = true;
            app.search_query.clear();
            EventResult::Continue
        }

        _ => EventResult::Continue,
    }
}

fn handle_search_key(key: KeyEvent, app: &mut App) -> EventResult {
    match key.code {
        KeyCode::Esc => {
            app.search_mode = false;
            app.search_query.clear();
            EventResult::Continue
        }
        KeyCode::Enter => {
            app.search_mode = false;
            EventResult::Continue
        }
        KeyCode::Backspace => {
            app.search_query.pop();
            EventResult::Continue
        }
        KeyCode::Char(c) => {
            app.search_query.push(c);
            EventResult::Continue
        }
        _ => EventResult::Continue,
    }
}
