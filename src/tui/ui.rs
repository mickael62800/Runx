//! TUI rendering for Test Explorer

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap},
};

use super::app::{App, Focus};
use super::widgets::TestTree;
use crate::test_model::FilterMode;

/// Draw the entire UI
pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),  // Header
            Constraint::Length(1),  // Filter bar
            Constraint::Min(10),    // Main content
            Constraint::Length(3),  // Footer
        ])
        .split(frame.size());

    draw_header(frame, app, chunks[0]);
    draw_filter_bar(frame, app, chunks[1]);
    draw_main(frame, app, chunks[2]);
    draw_footer(frame, app, chunks[3]);

    // Draw filter overlay if in filter input mode
    if app.filter_input_mode {
        draw_filter_overlay(frame, app);
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!(
        " Test Explorer - {} ",
        app.project_name
    );

    let progress = if app.stats.total > 0 {
        (app.stats.passed + app.stats.failed) as f64 / app.stats.total as f64
    } else {
        0.0
    };

    let progress_text = format!(
        "Passed: {}/{} | Failed: {} | Pending: {}",
        app.stats.passed,
        app.stats.total,
        app.stats.failed,
        app.stats.pending
    );

    let gauge_color = if app.stats.failed > 0 {
        Color::Red
    } else if app.stats.passed == app.stats.total && app.stats.total > 0 {
        Color::Green
    } else if app.running {
        Color::Yellow
    } else {
        Color::Blue
    };

    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .gauge_style(
            Style::default()
                .fg(gauge_color)
                .bg(Color::DarkGray)
        )
        .percent((progress * 100.0) as u16)
        .label(progress_text);

    frame.render_widget(gauge, area);
}

fn draw_filter_bar(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),    // Filter text
            Constraint::Length(40), // Filter mode buttons
        ])
        .split(area);

    // Filter text
    let filter_text = if app.filter.is_empty() {
        " [/] Search...".to_string()
    } else {
        format!(" Filter: {} ", app.filter)
    };

    let filter_style = if app.filter_input_mode {
        Style::default().fg(Color::Yellow)
    } else if !app.filter.is_empty() {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    frame.render_widget(
        Paragraph::new(filter_text).style(filter_style),
        chunks[0]
    );

    // Filter mode buttons
    let modes = [
        (FilterMode::All, "1"),
        (FilterMode::Passed, "2"),
        (FilterMode::Failed, "3"),
        (FilterMode::Pending, "4"),
    ];

    let mode_spans: Vec<Span> = modes.iter().map(|(mode, key)| {
        let label = format!("[{}]{} ", key, mode.label());
        if *mode == app.filter_mode {
            Span::styled(label, Style::default().fg(Color::Cyan).bold())
        } else {
            Span::styled(label, Style::default().fg(Color::DarkGray))
        }
    }).collect();

    frame.render_widget(
        Paragraph::new(Line::from(mode_spans)),
        chunks[1]
    );
}

fn draw_main(frame: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Test tree
            Constraint::Percentage(60), // Output viewer
        ])
        .split(area);

    draw_test_tree(frame, app, chunks[0]);
    draw_output_viewer(frame, app, chunks[1]);
}

fn draw_test_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = if app.filter.is_empty() {
        " Tests ".to_string()
    } else {
        format!(" Tests [{}] ", app.filter)
    };

    let border_style = if app.focus == Focus::TestList {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    let tree = TestTree::new(&app.test_tree)
        .block(block)
        .filter(&app.filter)
        .filter_mode(app.filter_mode);

    frame.render_stateful_widget(tree, area, &mut app.tree_state);
}

fn draw_output_viewer(frame: &mut Frame, app: &App, area: Rect) {
    let (title, content) = if let Some(test) = app.selected_test() {
        let status_symbol = test.status.symbol();
        let duration = test.duration_ms
            .map(|d| format!(" ({}ms)", d))
            .unwrap_or_default();

        let title = format!(" {} {} {} ", status_symbol, test.short_name, duration);

        let content = if test.output.is_empty() {
            match test.status {
                crate::test_model::TestStatus::Pending => "Test not yet run.\n\nPress Enter to run.".to_string(),
                crate::test_model::TestStatus::Running => "Running...".to_string(),
                crate::test_model::TestStatus::Passed => "Test passed (no output)".to_string(),
                crate::test_model::TestStatus::Failed => "Test failed (no output captured)".to_string(),
                crate::test_model::TestStatus::Ignored => "Test is ignored (#[ignore])".to_string(),
            }
        } else {
            let output: Vec<&str> = test.output.iter()
                .skip(app.output_scroll)
                .map(|s| s.as_str())
                .collect();
            output.join("\n")
        };

        (title, content)
    } else if let Some(item) = app.tree_state.selected_item() {
        if item.is_module {
            let title = format!(" {} ", item.name);
            let content = format!(
                "Module: {}\n\nTests: {}\nPassed: {}\nFailed: {}\n\nPress Enter to run all tests in this module.",
                item.path.join("::"),
                item.test_count,
                item.passed_count,
                item.failed_count
            );
            (title, content)
        } else {
            (" Output ".to_string(), "Select a test to view output".to_string())
        }
    } else {
        (" Output ".to_string(), "No test selected".to_string())
    };

    let border_style = if app.focus == Focus::Output {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };

    let paragraph = Paragraph::new(content)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(title))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Gray));

    frame.render_widget(paragraph, area);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),    // Help
            Constraint::Length(50), // Status
        ])
        .split(area);

    // Help text
    let help = if app.filter_input_mode {
        " [Enter] apply [Esc] cancel "
    } else if app.running {
        " [j/k] nav [Tab] focus [q] quit "
    } else {
        " [j/k] nav [Enter] run [a] all [f] failed [d] discover [/] filter [q] quit "
    };

    let help_text = Paragraph::new(help)
        .block(Block::default().borders(Borders::ALL))
        .style(Style::default().fg(Color::DarkGray));

    frame.render_widget(help_text, chunks[0]);

    // Status text
    let status = app.status_message.as_deref().unwrap_or("Ready");
    let status_style = if app.running {
        Style::default().fg(Color::Yellow)
    } else if app.stats.failed > 0 {
        Style::default().fg(Color::Red)
    } else if app.stats.passed > 0 && app.stats.passed == app.stats.total {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Cyan)
    };

    let status_text = Paragraph::new(format!(" {} ", status))
        .block(Block::default().borders(Borders::ALL))
        .style(status_style)
        .alignment(Alignment::Right);

    frame.render_widget(status_text, chunks[1]);
}

fn draw_filter_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(50, 3, frame.size());

    let input = Paragraph::new(app.filter.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Filter Tests "))
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
