//! TUI event handling for Test Explorer

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use super::App;
use crate::test_model::FilterMode;

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
    // Handle filter input mode separately
    if app.filter_input_mode {
        return handle_filter_key(key, app);
    }

    match key.code {
        // Quit
        KeyCode::Char('q') => EventResult::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => EventResult::Quit,
        KeyCode::Esc => {
            if !app.filter.is_empty() {
                app.clear_filter();
                EventResult::Continue
            } else {
                EventResult::Quit
            }
        }

        // Navigation
        KeyCode::Up | KeyCode::Char('k') => {
            app.select_prev();
            EventResult::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
            EventResult::Continue
        }
        KeyCode::Home | KeyCode::Char('g') => {
            app.select_first();
            EventResult::Continue
        }
        KeyCode::End | KeyCode::Char('G') => {
            app.select_last();
            EventResult::Continue
        }

        // Expand/collapse
        KeyCode::Char(' ') | KeyCode::Right | KeyCode::Char('l') => {
            app.toggle_expand();
            EventResult::Continue
        }
        KeyCode::Left | KeyCode::Char('h') => {
            // Collapse current node or go to parent
            if let Some(item) = app.tree_state.selected_item().cloned() {
                if item.is_module && item.expanded {
                    app.toggle_expand();
                }
            }
            EventResult::Continue
        }
        KeyCode::Char('e') => {
            app.expand_all();
            EventResult::Continue
        }
        KeyCode::Char('c') => {
            app.collapse_all();
            EventResult::Continue
        }

        // Output scrolling
        KeyCode::PageUp => {
            app.scroll_output_page_up();
            EventResult::Continue
        }
        KeyCode::PageDown => {
            app.scroll_output_page_down();
            EventResult::Continue
        }

        // Actions
        KeyCode::Enter => {
            app.run_selected();
            EventResult::Continue
        }
        KeyCode::Char('a') => {
            app.run_all();
            EventResult::Continue
        }
        KeyCode::Char('f') => {
            app.run_failed();
            EventResult::Continue
        }
        KeyCode::Char('d') => {
            let _ = app.discover_tests();
            EventResult::Continue
        }

        // Filter
        KeyCode::Char('/') => {
            app.start_filter_input();
            EventResult::Continue
        }

        // Filter mode (number keys)
        KeyCode::Char('1') => {
            app.set_filter_mode(FilterMode::All);
            EventResult::Continue
        }
        KeyCode::Char('2') => {
            app.set_filter_mode(FilterMode::Passed);
            EventResult::Continue
        }
        KeyCode::Char('3') => {
            app.set_filter_mode(FilterMode::Failed);
            EventResult::Continue
        }
        KeyCode::Char('4') => {
            app.set_filter_mode(FilterMode::Pending);
            EventResult::Continue
        }

        // Tab to cycle filter mode
        KeyCode::Tab => {
            app.cycle_filter_mode();
            EventResult::Continue
        }

        _ => EventResult::Continue,
    }
}

fn handle_filter_key(key: KeyEvent, app: &mut App) -> EventResult {
    match key.code {
        KeyCode::Esc => {
            app.clear_filter();
            EventResult::Continue
        }
        KeyCode::Enter => {
            app.end_filter_input();
            EventResult::Continue
        }
        KeyCode::Backspace => {
            app.filter_pop();
            EventResult::Continue
        }
        KeyCode::Char(c) => {
            app.filter_push(c);
            EventResult::Continue
        }
        _ => EventResult::Continue,
    }
}
