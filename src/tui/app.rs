//! Test Explorer TUI application state

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};

use crate::db::Database;
use crate::discovery::{discover_all_tests, get_project_name};
use crate::test_model::{FilterMode, Test, TestNode, TestStats, TestStatus};
use crate::test_runner::{create_event_channel, run_tests_async, TestEvent};
use crate::tui::widgets::{TestTreeState, toggle_node_expansion, expand_all, collapse_all};

/// Focus area in the UI
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Focus {
    #[default]
    TestList,
    Output,
    Filter,
}

/// TUI application state
pub struct App {
    /// Project directory
    pub project_dir: PathBuf,
    /// Project name
    pub project_name: String,
    /// Database connection
    pub db: Option<Database>,
    /// Test tree (hierarchical)
    pub test_tree: TestNode,
    /// Tree widget state
    pub tree_state: TestTreeState,
    /// Current filter string
    pub filter: String,
    /// Filter mode (all/passed/failed/etc)
    pub filter_mode: FilterMode,
    /// Currently focused area
    pub focus: Focus,
    /// Whether we're in filter input mode
    pub filter_input_mode: bool,
    /// Output scroll position
    pub output_scroll: usize,
    /// Currently selected test (for output display)
    pub selected_test: Option<String>,
    /// Test statistics
    pub stats: TestStats,
    /// Whether tests are running
    pub running: bool,
    /// Event receiver for test updates
    pub event_rx: Option<Receiver<TestEvent>>,
    /// Event sender (kept to pass to runner)
    event_tx: Option<Sender<TestEvent>>,
    /// Status message
    pub status_message: Option<String>,
    /// Discovery in progress
    pub discovering: bool,
}

impl App {
    pub fn new(project_dir: &Path, db: Option<Database>) -> Self {
        let project_name = get_project_name(project_dir)
            .unwrap_or_else(|_| "Unknown".to_string());

        let test_tree = TestNode::new_module("tests");

        Self {
            project_dir: project_dir.to_path_buf(),
            project_name,
            db,
            test_tree,
            tree_state: TestTreeState::new(),
            filter: String::new(),
            filter_mode: FilterMode::All,
            focus: Focus::TestList,
            filter_input_mode: false,
            output_scroll: 0,
            selected_test: None,
            stats: TestStats::default(),
            running: false,
            event_rx: None,
            event_tx: None,
            status_message: Some("Press 'd' to discover tests".to_string()),
            discovering: false,
        }
    }

    /// Discover tests in the project
    pub fn discover_tests(&mut self) -> anyhow::Result<()> {
        self.discovering = true;
        self.status_message = Some("Discovering tests...".to_string());

        match discover_all_tests(&self.project_dir) {
            Ok(tree) => {
                self.test_tree = tree;
                self.stats = TestStats::from_tree(&self.test_tree);
                self.discovering = false;
                self.status_message = Some(format!("Found {} tests", self.stats.total));

                // Expand first level by default
                for child in &mut self.test_tree.children {
                    child.expanded = true;
                }

                Ok(())
            }
            Err(e) => {
                self.discovering = false;
                self.status_message = Some(format!("Discovery failed: {}", e));
                Err(e)
            }
        }
    }

    /// Get the currently selected test
    pub fn selected_test(&self) -> Option<&Test> {
        if let Some(item) = self.tree_state.selected_item() {
            if let Some(ref full_name) = item.full_name {
                return self.test_tree.find_test(full_name);
            }
        }
        None
    }

    /// Get the currently selected test (mutable)
    pub fn selected_test_mut(&mut self) -> Option<&mut Test> {
        let full_name = self.tree_state.selected_item()
            .and_then(|item| item.full_name.clone());

        if let Some(name) = full_name {
            return self.test_tree.find_test_mut(&name);
        }
        None
    }

    /// Move selection up
    pub fn select_prev(&mut self) {
        self.tree_state.up();
        self.output_scroll = 0;
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        let max = self.tree_state.visible_items.len();
        self.tree_state.down(max);
        self.output_scroll = 0;
    }

    /// Move to first item
    pub fn select_first(&mut self) {
        self.tree_state.selected = 0;
        self.output_scroll = 0;
    }

    /// Move to last item
    pub fn select_last(&mut self) {
        self.tree_state.selected = self.tree_state.visible_items.len().saturating_sub(1);
        self.output_scroll = 0;
    }

    /// Toggle expansion of selected item
    pub fn toggle_expand(&mut self) {
        if let Some(item) = self.tree_state.selected_item().cloned() {
            if item.is_module {
                toggle_node_expansion(&mut self.test_tree, &item.path);
            }
        }
    }

    /// Expand all nodes
    pub fn expand_all(&mut self) {
        expand_all(&mut self.test_tree);
    }

    /// Collapse all nodes
    pub fn collapse_all(&mut self) {
        collapse_all(&mut self.test_tree);
    }

    /// Scroll output up
    pub fn scroll_output_up(&mut self) {
        if self.output_scroll > 0 {
            self.output_scroll -= 1;
        }
    }

    /// Scroll output down
    pub fn scroll_output_down(&mut self) {
        self.output_scroll += 1;
    }

    /// Scroll output page up
    pub fn scroll_output_page_up(&mut self) {
        self.output_scroll = self.output_scroll.saturating_sub(20);
    }

    /// Scroll output page down
    pub fn scroll_output_page_down(&mut self) {
        self.output_scroll += 20;
    }

    /// Cycle filter mode
    pub fn cycle_filter_mode(&mut self) {
        self.filter_mode = self.filter_mode.cycle_next();
    }

    /// Set filter mode by number key
    pub fn set_filter_mode(&mut self, mode: FilterMode) {
        self.filter_mode = mode;
    }

    /// Start filter input mode
    pub fn start_filter_input(&mut self) {
        self.filter_input_mode = true;
        self.focus = Focus::Filter;
    }

    /// End filter input mode
    pub fn end_filter_input(&mut self) {
        self.filter_input_mode = false;
        self.focus = Focus::TestList;
    }

    /// Clear filter
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.filter_input_mode = false;
        self.focus = Focus::TestList;
    }

    /// Add character to filter
    pub fn filter_push(&mut self, c: char) {
        self.filter.push(c);
    }

    /// Remove character from filter
    pub fn filter_pop(&mut self) {
        self.filter.pop();
    }

    /// Run selected test
    pub fn run_selected(&mut self) {
        if self.running {
            return;
        }

        if let Some(item) = self.tree_state.selected_item().cloned() {
            if let Some(full_name) = item.full_name {
                self.run_test(&full_name);
            } else if item.is_module {
                // Run all tests in module
                self.run_filtered(&item.path.join("::"));
            }
        }
    }

    /// Run a single test
    pub fn run_test(&mut self, test_name: &str) {
        if self.running {
            return;
        }

        // Reset test status
        if let Some(test) = self.test_tree.find_test_mut(test_name) {
            test.status = TestStatus::Running;
            test.output.clear();
        }
        self.test_tree.update_counts();

        // Create event channel
        let (tx, rx) = create_event_channel();
        self.event_tx = Some(tx.clone());
        self.event_rx = Some(rx);

        // Start test runner in background
        self.running = true;
        self.status_message = Some(format!("Running: {}", test_name));

        let _handle = run_tests_async(&self.project_dir, Some(test_name.to_string()), tx);
    }

    /// Run tests matching filter
    pub fn run_filtered(&mut self, filter: &str) {
        if self.running {
            return;
        }

        // Mark matching tests as running
        let filter_lower = filter.to_lowercase();
        let matching_names: Vec<String> = self.test_tree.all_tests()
            .iter()
            .filter(|t| t.full_name.to_lowercase().contains(&filter_lower))
            .map(|t| t.full_name.clone())
            .collect();

        for name in matching_names {
            if let Some(t) = self.test_tree.find_test_mut(&name) {
                t.status = TestStatus::Running;
                t.output.clear();
            }
        }
        self.test_tree.update_counts();

        // Create event channel
        let (tx, rx) = create_event_channel();
        self.event_tx = Some(tx.clone());
        self.event_rx = Some(rx);

        // Start test runner
        self.running = true;
        self.status_message = Some(format!("Running tests matching: {}", filter));

        let _handle = run_tests_async(&self.project_dir, Some(filter.to_string()), tx);
    }

    /// Run all tests
    pub fn run_all(&mut self) {
        if self.running {
            return;
        }

        // Mark all tests as running
        for name in self.test_tree.all_test_names() {
            if let Some(test) = self.test_tree.find_test_mut(&name) {
                test.status = TestStatus::Running;
                test.output.clear();
            }
        }
        self.test_tree.update_counts();

        // Create event channel
        let (tx, rx) = create_event_channel();
        self.event_tx = Some(tx.clone());
        self.event_rx = Some(rx);

        // Start test runner
        self.running = true;
        self.status_message = Some("Running all tests...".to_string());

        let _handle = run_tests_async(&self.project_dir, None, tx);
    }

    /// Run failed tests
    pub fn run_failed(&mut self) {
        if self.running {
            return;
        }

        let failed_names: Vec<String> = self.test_tree
            .failed_tests()
            .iter()
            .map(|t| t.full_name.clone())
            .collect();

        if failed_names.is_empty() {
            self.status_message = Some("No failed tests to run".to_string());
            return;
        }

        // Mark failed tests as running
        for name in &failed_names {
            if let Some(test) = self.test_tree.find_test_mut(name) {
                test.status = TestStatus::Running;
                test.output.clear();
            }
        }
        self.test_tree.update_counts();

        // Create event channel
        let (tx, rx) = create_event_channel();
        self.event_tx = Some(tx.clone());
        self.event_rx = Some(rx);

        self.running = true;
        self.status_message = Some(format!("Running {} failed tests...", failed_names.len()));

        // Run each failed test
        for name in &failed_names {
            // This is a simplification - ideally we'd batch these
            let _handle = run_tests_async(&self.project_dir, Some(name.clone()), tx.clone());
        }
    }

    /// Update app state from test events
    pub fn update(&mut self) {
        if let Some(ref rx) = self.event_rx {
            // Process all available events
            while let Ok(event) = rx.try_recv() {
                match event {
                    TestEvent::Started { test_name } => {
                        if let Some(test) = self.test_tree.find_test_mut(&test_name) {
                            test.status = TestStatus::Running;
                        }
                        self.status_message = Some(format!("Running: {}", test_name));
                    }
                    TestEvent::Output { test_name, line } => {
                        if let Some(test) = self.test_tree.find_test_mut(&test_name) {
                            test.add_output(line);
                        }
                    }
                    TestEvent::Completed { test_name, status, duration_ms } => {
                        if let Some(test) = self.test_tree.find_test_mut(&test_name) {
                            test.status = status;
                            test.duration_ms = Some(duration_ms);
                            test.last_run = Some(chrono::Utc::now());
                        }
                    }
                    TestEvent::AllCompleted { passed, failed, ignored } => {
                        self.running = false;
                        self.status_message = Some(format!(
                            "Completed: {} passed, {} failed, {} ignored",
                            passed, failed, ignored
                        ));
                    }
                    TestEvent::Error { message } => {
                        self.running = false;
                        self.status_message = Some(format!("Error: {}", message));
                    }
                }
                self.test_tree.update_counts();
                self.stats = TestStats::from_tree(&self.test_tree);
            }
        }
    }

    /// Get output lines for currently selected test
    pub fn selected_output(&self) -> Vec<&str> {
        if let Some(test) = self.selected_test() {
            test.output.iter().map(|s| s.as_str()).collect()
        } else {
            Vec::new()
        }
    }
}

// Keep old types for backward compatibility during transition
// These will be removed in cleanup phase

/// Task status in TUI (legacy)
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

/// A task in the TUI (legacy)
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
        while self.output.len() > 1000 {
            self.output.pop_front();
        }
    }
}
