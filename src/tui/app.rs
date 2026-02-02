//! TUI application state

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::db::Database;

/// Task status in TUI
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    Running,
    Passed,
    Failed,
    Skipped,
}

impl TaskStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "○",
            TaskStatus::Running => "●",
            TaskStatus::Passed => "✓",
            TaskStatus::Failed => "✗",
            TaskStatus::Skipped => "⊘",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            TaskStatus::Pending => Color::Gray,
            TaskStatus::Running => Color::Yellow,
            TaskStatus::Passed => Color::Green,
            TaskStatus::Failed => Color::Red,
            TaskStatus::Skipped => Color::DarkGray,
        }
    }
}

/// A task in the TUI
#[derive(Debug, Clone)]
pub struct TuiTask {
    pub name: String,
    pub status: TaskStatus,
    pub duration_ms: Option<u128>,
    pub output: VecDeque<String>,
}

impl TuiTask {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: TaskStatus::Pending,
            duration_ms: None,
            output: VecDeque::with_capacity(1000),
        }
    }

    pub fn add_output(&mut self, line: String) {
        self.output.push_back(line);
        // Keep only last 1000 lines
        while self.output.len() > 1000 {
            self.output.pop_front();
        }
    }
}

/// TUI application state
pub struct App<'a> {
    pub config: &'a Config,
    pub base_dir: PathBuf,
    pub db: Option<Database>,
    pub tasks: Vec<TuiTask>,
    pub selected_task: usize,
    pub log_scroll: usize,
    pub search_query: String,
    pub search_mode: bool,
    pub running: bool,
    pub completed: usize,
    pub passed: usize,
    pub failed: usize,
}

impl<'a> App<'a> {
    pub fn new(config: &'a Config, base_dir: &Path, db: Option<Database>) -> Self {
        // Build task list from config
        let mut task_names: Vec<_> = config.task_names().into_iter().cloned().collect();
        task_names.sort();

        let tasks: Vec<TuiTask> = task_names
            .into_iter()
            .map(TuiTask::new)
            .collect();

        Self {
            config,
            base_dir: base_dir.to_path_buf(),
            db,
            tasks,
            selected_task: 0,
            log_scroll: 0,
            search_query: String::new(),
            search_mode: false,
            running: false,
            completed: 0,
            passed: 0,
            failed: 0,
        }
    }

    pub fn selected_task(&self) -> Option<&TuiTask> {
        self.tasks.get(self.selected_task)
    }

    pub fn selected_task_mut(&mut self) -> Option<&mut TuiTask> {
        self.tasks.get_mut(self.selected_task)
    }

    pub fn next_task(&mut self) {
        if self.selected_task < self.tasks.len().saturating_sub(1) {
            self.selected_task += 1;
            self.log_scroll = 0;
        }
    }

    pub fn prev_task(&mut self) {
        if self.selected_task > 0 {
            self.selected_task -= 1;
            self.log_scroll = 0;
        }
    }

    pub fn scroll_log_down(&mut self) {
        if let Some(task) = self.selected_task() {
            if self.log_scroll < task.output.len().saturating_sub(1) {
                self.log_scroll += 1;
            }
        }
    }

    pub fn scroll_log_up(&mut self) {
        if self.log_scroll > 0 {
            self.log_scroll -= 1;
        }
    }

    pub fn scroll_log_page_down(&mut self) {
        if let Some(task) = self.selected_task() {
            let max = task.output.len().saturating_sub(20);
            self.log_scroll = (self.log_scroll + 20).min(max);
        }
    }

    pub fn scroll_log_page_up(&mut self) {
        self.log_scroll = self.log_scroll.saturating_sub(20);
    }

    pub fn filter_tasks(&self) -> Vec<(usize, &TuiTask)> {
        if self.search_query.is_empty() {
            self.tasks.iter().enumerate().collect()
        } else {
            let query = self.search_query.to_lowercase();
            self.tasks
                .iter()
                .enumerate()
                .filter(|(_, t)| t.name.to_lowercase().contains(&query))
                .collect()
        }
    }

    pub fn update(&mut self) {
        // Update task status based on running state
        // This would be called from the main loop
    }

    pub fn start_run(&mut self) {
        self.running = true;
        self.completed = 0;
        self.passed = 0;
        self.failed = 0;

        // Reset all tasks to pending
        for task in &mut self.tasks {
            task.status = TaskStatus::Pending;
            task.duration_ms = None;
            task.output.clear();
        }
    }

    pub fn task_started(&mut self, name: &str) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.name == name) {
            task.status = TaskStatus::Running;
        }
    }

    pub fn task_completed(&mut self, name: &str, success: bool, duration_ms: u128) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.name == name) {
            task.status = if success { TaskStatus::Passed } else { TaskStatus::Failed };
            task.duration_ms = Some(duration_ms);

            self.completed += 1;
            if success {
                self.passed += 1;
            } else {
                self.failed += 1;
            }
        }
    }

    pub fn task_output(&mut self, name: &str, line: String) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.name == name) {
            task.add_output(line);
        }
    }

    pub fn retry_selected(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            task.status = TaskStatus::Pending;
            task.duration_ms = None;
            task.output.clear();
        }
    }

    pub fn skip_selected(&mut self) {
        if let Some(task) = self.tasks.get_mut(self.selected_task) {
            if task.status == TaskStatus::Pending {
                task.status = TaskStatus::Skipped;
            }
        }
    }

    pub fn progress(&self) -> f64 {
        if self.tasks.is_empty() {
            return 0.0;
        }
        self.completed as f64 / self.tasks.len() as f64
    }
}
