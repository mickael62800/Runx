//! Log viewer widget

use ratatui::{
    prelude::*,
    widgets::{Block, StatefulWidget, Widget},
};

/// State for log viewer
#[derive(Default)]
pub struct LogViewerState {
    pub offset: usize,
    pub follow: bool,
}

impl LogViewerState {
    pub fn scroll_to(&mut self, offset: usize) {
        self.offset = offset;
        self.follow = false;
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.offset = self.offset.saturating_sub(amount);
        self.follow = false;
    }

    pub fn scroll_down(&mut self, amount: usize, max: usize) {
        self.offset = (self.offset + amount).min(max);
    }

    pub fn enable_follow(&mut self) {
        self.follow = true;
    }
}

/// Log viewer widget
pub struct LogViewer<'a> {
    lines: &'a [String],
    block: Option<Block<'a>>,
    style: Style,
    highlight_pattern: Option<&'a str>,
}

impl<'a> LogViewer<'a> {
    pub fn new(lines: &'a [String]) -> Self {
        Self {
            lines,
            block: None,
            style: Style::default(),
            highlight_pattern: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Self {
        self.style = style;
        self
    }

    pub fn highlight(mut self, pattern: &'a str) -> Self {
        self.highlight_pattern = Some(pattern);
        self
    }
}

impl<'a> StatefulWidget for LogViewer<'a> {
    type State = LogViewerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let inner_area = match self.block {
            Some(b) => {
                let inner = b.inner(area);
                b.render(area, buf);
                inner
            }
            None => area,
        };

        if self.lines.is_empty() {
            return;
        }

        let visible_height = inner_area.height as usize;

        // Adjust offset if following
        if state.follow {
            state.offset = self.lines.len().saturating_sub(visible_height);
        }

        // Render visible lines
        for (i, line) in self.lines.iter().skip(state.offset).take(visible_height).enumerate() {
            let y = inner_area.y + i as u16;

            // Determine line style based on content
            let line_style = if line.contains("error") || line.contains("FAILED") || line.contains("Error") {
                Style::default().fg(Color::Red)
            } else if line.contains("warning") || line.contains("WARN") {
                Style::default().fg(Color::Yellow)
            } else if line.contains("PASSED") || line.contains("ok") {
                Style::default().fg(Color::Green)
            } else {
                self.style
            };

            // Highlight search pattern if present
            if let Some(pattern) = self.highlight_pattern {
                if !pattern.is_empty() && line.to_lowercase().contains(&pattern.to_lowercase()) {
                    let highlighted_style = line_style.bg(Color::DarkGray);
                    render_line_with_highlight(buf, inner_area.x, y, inner_area.width, line, pattern, highlighted_style);
                    continue;
                }
            }

            // Truncate line if needed
            let display_line = if line.len() > inner_area.width as usize {
                format!("{}...", &line[..inner_area.width as usize - 3])
            } else {
                line.clone()
            };

            buf.set_string(inner_area.x, y, &display_line, line_style);
        }

        // Render scroll indicator
        if self.lines.len() > visible_height {
            let scroll_pct = if self.lines.len() <= visible_height {
                0
            } else {
                (state.offset * 100) / (self.lines.len() - visible_height)
            };

            let indicator = format!("{}%", scroll_pct);
            let x = inner_area.x + inner_area.width - indicator.len() as u16 - 1;
            let y = inner_area.y;
            buf.set_string(x, y, &indicator, Style::default().fg(Color::DarkGray));
        }
    }
}

fn render_line_with_highlight(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    width: u16,
    line: &str,
    pattern: &str,
    highlight_style: Style,
) {
    let pattern_lower = pattern.to_lowercase();

    let mut current_x = x;
    let mut remaining = line;

    while let Some(pos) = remaining.to_lowercase().find(&pattern_lower) {
        // Render text before match
        if pos > 0 {
            let before = &remaining[..pos];
            buf.set_string(current_x, y, before, Style::default());
            current_x += before.len() as u16;
        }

        // Render match
        let match_text = &remaining[pos..pos + pattern.len()];
        buf.set_string(current_x, y, match_text, highlight_style);
        current_x += match_text.len() as u16;

        remaining = &remaining[pos + pattern.len()..];

        if current_x >= x + width {
            break;
        }
    }

    // Render remaining text
    if !remaining.is_empty() && current_x < x + width {
        buf.set_string(current_x, y, remaining, Style::default());
    }
}
