//! Task list widget

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, StatefulWidget, Widget},
};

use crate::tui::app::TuiTask;

/// State for task list widget
#[derive(Default)]
pub struct TaskListState {
    pub selected: Option<usize>,
    pub offset: usize,
}

impl TaskListState {
    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
    }
}

/// Custom task list widget
pub struct TaskList<'a> {
    tasks: &'a [TuiTask],
    block: Option<Block<'a>>,
    highlight_style: Style,
}

impl<'a> TaskList<'a> {
    pub fn new(tasks: &'a [TuiTask]) -> Self {
        Self {
            tasks,
            block: None,
            highlight_style: Style::default().bg(Color::DarkGray),
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }
}

impl<'a> StatefulWidget for TaskList<'a> {
    type State = TaskListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let inner_area = match self.block {
            Some(b) => {
                let inner = b.inner(area);
                b.render(area, buf);
                inner
            }
            None => area,
        };

        if self.tasks.is_empty() {
            return;
        }

        let visible_height = inner_area.height as usize;

        // Adjust offset to keep selected item visible
        if let Some(selected) = state.selected {
            if selected < state.offset {
                state.offset = selected;
            } else if selected >= state.offset + visible_height {
                state.offset = selected - visible_height + 1;
            }
        }

        // Render visible tasks
        for (i, task) in self.tasks.iter().skip(state.offset).take(visible_height).enumerate() {
            let y = inner_area.y + i as u16;
            let is_selected = state.selected == Some(state.offset + i);

            // Status symbol
            let status_symbol = task.status.symbol();
            let status_color = task.status.color();

            // Duration
            let duration = task.duration_ms
                .map(|d| format!(" {}ms", d))
                .unwrap_or_default();

            // Build line
            let mut x = inner_area.x;

            // Selection indicator
            if is_selected {
                buf.set_string(x, y, ">", Style::default().fg(Color::Cyan));
            }
            x += 2;

            // Status symbol
            buf.set_string(x, y, status_symbol, Style::default().fg(status_color));
            x += 2;

            // Task name
            let name_style = if is_selected {
                self.highlight_style
            } else {
                Style::default()
            };
            let max_name_len = (inner_area.width as usize).saturating_sub(10);
            let display_name = if task.name.len() > max_name_len {
                format!("{}...", &task.name[..max_name_len - 3])
            } else {
                task.name.clone()
            };
            buf.set_string(x, y, &display_name, name_style);
            x += display_name.len() as u16 + 1;

            // Duration
            if !duration.is_empty() {
                buf.set_string(x, y, &duration, Style::default().fg(Color::DarkGray));
            }
        }
    }
}
