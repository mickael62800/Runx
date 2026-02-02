//! TUI rendering

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap},
};

use super::app::{App, TaskStatus};

/// Draw the entire UI
pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Footer
        ])
        .split(frame.size());

    draw_header(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_footer(frame, app, chunks[2]);

    // Draw search overlay if in search mode
    if app.search_mode {
        draw_search_overlay(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " Runx TUI - {} ",
        app.config.project.name
    );

    let progress = app.progress();
    let progress_text = format!(
        "{}/{} tasks ({} passed, {} failed)",
        app.completed,
        app.tasks.len(),
        app.passed,
        app.failed
    );

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .gauge_style(
            Style::default()
                .fg(if app.failed > 0 { Color::Red } else { Color::Green })
                .bg(Color::DarkGray)
        )
        .percent((progress * 100.0) as u16)
        .label(progress_text);

    frame.render_widget(gauge, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(30), // Task list
            Constraint::Percentage(70), // Log viewer
        ])
        .split(area);

    draw_task_list(frame, app, chunks[0]);
    draw_log_viewer(frame, app, chunks[1]);
}

fn draw_task_list(frame: &mut Frame, app: &App, area: Rect) {
    let filtered_tasks = app.filter_tasks();

    let items: Vec<ListItem> = filtered_tasks
        .iter()
        .enumerate()
        .map(|(display_idx, (real_idx, task))| {
            let is_selected = *real_idx == app.selected_task;

            // Build status indicator
            let status_symbol = task.status.symbol();
            let status_color = task.status.color();

            // Duration if available
            let duration = task.duration_ms
                .map(|d| format!(" {}ms", d))
                .unwrap_or_default();

            let content = format!("{} {}{}", status_symbol, task.name, duration);

            let style = if is_selected {
                Style::default().bg(Color::DarkGray).bold()
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![
                Span::styled(status_symbol, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(&task.name, style),
                Span::styled(duration, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let title = if app.search_query.is_empty() {
        " Tasks ".to_string()
    } else {
        format!(" Tasks [{}] ", app.search_query)
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("> ");

    frame.render_widget(list, area);
}

fn draw_log_viewer(frame: &mut Frame, app: &App, area: Rect) {
    let title = app.selected_task()
        .map(|t| format!(" Output: {} ", t.name))
        .unwrap_or_else(|| " Output ".to_string());

    let content = if let Some(task) = app.selected_task() {
        if task.output.is_empty() {
            match task.status {
                TaskStatus::Pending => "Waiting to run...".to_string(),
                TaskStatus::Running => "Running...".to_string(),
                TaskStatus::Passed => "Task completed successfully (no output)".to_string(),
                TaskStatus::Failed => "Task failed (no output)".to_string(),
                TaskStatus::Skipped => "Task skipped".to_string(),
            }
        } else {
            task.output
                .iter()
                .skip(app.log_scroll)
                .take(area.height as usize - 2)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        }
    } else {
        "No task selected".to_string()
    };

    let paragraph = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let help = if app.running {
        " [r]etry [s]kip [/]search [q]uit "
    } else {
        " [Enter]run [r]etry [s]kip [/]search [q]uit "
    };

    let status = format!(
        " {} ",
        if app.running { "Running" } else { "Ready" }
    );

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),
            Constraint::Length(15),
        ])
        .split(area);

    let help_text = Paragraph::new(help)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));

    let status_style = if app.running {
        Style::default().fg(Color::Yellow)
    } else if app.failed > 0 {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };

    let status_text = Paragraph::new(status)
        .block(Block::default().borders(Borders::ALL))
        .style(status_style)
        .alignment(Alignment::Center);

    frame.render_widget(help_text, chunks[0]);
    frame.render_widget(status_text, chunks[1]);
}

fn draw_search_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 3, frame.size());

    let input = Paragraph::new(app.search_query.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Search "))
        .style(Style::default().fg(Color::Yellow));

    frame.render_widget(Clear, area);
    frame.render_widget(input, area);
}

/// Create a centered rectangle
fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - 20) / 2),
            Constraint::Length(height),
            Constraint::Percentage((100 - 20) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
