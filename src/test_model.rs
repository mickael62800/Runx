//! Test model definitions
//!
//! Core data structures for representing discovered tests, their status,
//! and the hierarchical test tree.

#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a test
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum TestStatus {
    /// Test has never been run
    #[default]
    Pending,
    /// Test is currently running
    Running,
    /// Test passed
    Passed,
    /// Test failed
    Failed,
    /// Test is marked with #[ignore]
    Ignored,
}

impl TestStatus {
    pub fn symbol(&self) -> &'static str {
        match self {
            TestStatus::Pending => "○",
            TestStatus::Running => "●",
            TestStatus::Passed => "✓",
            TestStatus::Failed => "✗",
            TestStatus::Ignored => "⊘",
        }
    }

    pub fn color(&self) -> ratatui::style::Color {
        use ratatui::style::Color;
        match self {
            TestStatus::Pending => Color::Gray,
            TestStatus::Running => Color::Yellow,
            TestStatus::Passed => Color::Green,
            TestStatus::Failed => Color::Red,
            TestStatus::Ignored => Color::DarkGray,
        }
    }
}


/// A discovered test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Test {
    /// Unique identifier (hash of full_name)
    pub id: String,
    /// Full test name including module path (e.g., "module::tests::test_name")
    pub full_name: String,
    /// Short test name without module path (e.g., "test_name")
    pub short_name: String,
    /// Module path as a vector (e.g., ["module", "tests"])
    pub module_path: Vec<String>,
    /// Current test status
    pub status: TestStatus,
    /// Duration in milliseconds (if run)
    pub duration_ms: Option<u64>,
    /// Test output lines
    pub output: Vec<String>,
    /// Last run timestamp
    pub last_run: Option<DateTime<Utc>>,
    /// Source file path (if known)
    pub source_file: Option<String>,
    /// Line number in source file (if known)
    pub line_number: Option<u32>,
}

impl Test {
    /// Create a new test from a full test name
    pub fn from_name(full_name: &str) -> Self {
        let parts: Vec<&str> = full_name.split("::").collect();
        let short_name = parts.last().unwrap_or(&full_name).to_string();
        let module_path: Vec<String> = if parts.len() > 1 {
            parts[..parts.len() - 1].iter().map(|s| s.to_string()).collect()
        } else {
            Vec::new()
        };

        // Generate a simple ID from the full name
        let id = format!("{:x}", md5_hash(full_name));

        Self {
            id,
            full_name: full_name.to_string(),
            short_name,
            module_path,
            status: TestStatus::Pending,
            duration_ms: None,
            output: Vec::new(),
            last_run: None,
            source_file: None,
            line_number: None,
        }
    }

    /// Add output line to the test
    pub fn add_output(&mut self, line: String) {
        self.output.push(line);
        // Keep only last 1000 lines
        if self.output.len() > 1000 {
            self.output.remove(0);
        }
    }

    /// Clear test output
    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    /// Reset test to pending state
    pub fn reset(&mut self) {
        self.status = TestStatus::Pending;
        self.duration_ms = None;
        self.output.clear();
    }
}

/// Simple hash function for generating test IDs
fn md5_hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

/// A node in the test tree (can be a module or a test)
#[derive(Debug, Clone)]
pub struct TestNode {
    /// Node name (module name or test name)
    pub name: String,
    /// Whether this node is expanded in the UI
    pub expanded: bool,
    /// Child nodes (modules or tests)
    pub children: Vec<TestNode>,
    /// Test data if this is a test node (leaf)
    pub test: Option<Test>,
    /// Aggregated status from children
    pub status: TestStatus,
    /// Count of tests in this subtree
    pub test_count: usize,
    /// Count of passed tests in this subtree
    pub passed_count: usize,
    /// Count of failed tests in this subtree
    pub failed_count: usize,
}

impl TestNode {
    /// Create a new module node
    pub fn new_module(name: &str) -> Self {
        Self {
            name: name.to_string(),
            expanded: false,
            children: Vec::new(),
            test: None,
            status: TestStatus::Pending,
            test_count: 0,
            passed_count: 0,
            failed_count: 0,
        }
    }

    /// Create a new test node (leaf)
    pub fn new_test(test: Test) -> Self {
        let status = test.status;
        Self {
            name: test.short_name.clone(),
            expanded: false,
            children: Vec::new(),
            test: Some(test),
            status,
            test_count: 1,
            passed_count: if status == TestStatus::Passed { 1 } else { 0 },
            failed_count: if status == TestStatus::Failed { 1 } else { 0 },
        }
    }

    /// Check if this is a leaf node (test)
    pub fn is_test(&self) -> bool {
        self.test.is_some()
    }

    /// Check if this is a module node
    pub fn is_module(&self) -> bool {
        self.test.is_none()
    }

    /// Toggle expanded state
    pub fn toggle_expanded(&mut self) {
        if self.is_module() {
            self.expanded = !self.expanded;
        }
    }

    /// Update aggregated counts from children
    pub fn update_counts(&mut self) {
        if self.is_test() {
            // Leaf node, counts are already set
            return;
        }

        self.test_count = 0;
        self.passed_count = 0;
        self.failed_count = 0;

        for child in &mut self.children {
            child.update_counts();
            self.test_count += child.test_count;
            self.passed_count += child.passed_count;
            self.failed_count += child.failed_count;
        }

        // Determine aggregate status
        if self.failed_count > 0 {
            self.status = TestStatus::Failed;
        } else if self.passed_count == self.test_count && self.test_count > 0 {
            self.status = TestStatus::Passed;
        } else if self.children.iter().any(|c| c.status == TestStatus::Running) {
            self.status = TestStatus::Running;
        } else {
            self.status = TestStatus::Pending;
        }
    }

    /// Find or create a child module node
    pub fn get_or_create_child(&mut self, name: &str) -> &mut TestNode {
        if let Some(idx) = self.children.iter().position(|c| c.name == name) {
            &mut self.children[idx]
        } else {
            self.children.push(TestNode::new_module(name));
            self.children.last_mut().unwrap()
        }
    }

    /// Add a test to this tree
    pub fn add_test(&mut self, test: Test) {
        if test.module_path.is_empty() {
            // Add directly as a child
            self.children.push(TestNode::new_test(test));
        } else {
            // Navigate/create path to the correct module
            let mut current = self;
            for module in &test.module_path {
                current = current.get_or_create_child(module);
            }
            current.children.push(TestNode::new_test(test));
        }
    }

    /// Sort children alphabetically (modules first, then tests)
    pub fn sort_children(&mut self) {
        self.children.sort_by(|a, b| {
            match (a.is_module(), b.is_module()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.cmp(&b.name),
            }
        });

        for child in &mut self.children {
            child.sort_children();
        }
    }

    /// Find a test by its full name
    pub fn find_test(&self, full_name: &str) -> Option<&Test> {
        if let Some(ref test) = self.test {
            if test.full_name == full_name {
                return Some(test);
            }
        }
        for child in &self.children {
            if let Some(test) = child.find_test(full_name) {
                return Some(test);
            }
        }
        None
    }

    /// Find a test by its full name (mutable)
    pub fn find_test_mut(&mut self, full_name: &str) -> Option<&mut Test> {
        if let Some(ref mut test) = self.test {
            if test.full_name == full_name {
                return Some(test);
            }
        }
        for child in &mut self.children {
            if let Some(test) = child.find_test_mut(full_name) {
                return Some(test);
            }
        }
        None
    }

    /// Get all tests as a flat list
    pub fn all_tests(&self) -> Vec<&Test> {
        let mut tests = Vec::new();
        self.collect_tests(&mut tests);
        tests
    }

    fn collect_tests<'a>(&'a self, tests: &mut Vec<&'a Test>) {
        if let Some(ref test) = self.test {
            tests.push(test);
        }
        for child in &self.children {
            child.collect_tests(tests);
        }
    }

    /// Get all test names
    pub fn all_test_names(&self) -> Vec<String> {
        self.all_tests().iter().map(|t| t.full_name.clone()).collect()
    }

    /// Get tests matching a filter
    pub fn filter_tests(&self, filter: &str) -> Vec<&Test> {
        let filter_lower = filter.to_lowercase();
        self.all_tests()
            .into_iter()
            .filter(|t| t.full_name.to_lowercase().contains(&filter_lower))
            .collect()
    }

    /// Get tests by status
    pub fn tests_by_status(&self, status: TestStatus) -> Vec<&Test> {
        self.all_tests()
            .into_iter()
            .filter(|t| t.status == status)
            .collect()
    }

    /// Get failed tests
    pub fn failed_tests(&self) -> Vec<&Test> {
        self.tests_by_status(TestStatus::Failed)
    }
}

/// Filter mode for the test list
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FilterMode {
    #[default]
    All,
    Passed,
    Failed,
    Pending,
    Ignored,
}

impl FilterMode {
    pub fn label(&self) -> &'static str {
        match self {
            FilterMode::All => "All",
            FilterMode::Passed => "Passed",
            FilterMode::Failed => "Failed",
            FilterMode::Pending => "Pending",
            FilterMode::Ignored => "Ignored",
        }
    }

    pub fn matches(&self, status: TestStatus) -> bool {
        match self {
            FilterMode::All => true,
            FilterMode::Passed => status == TestStatus::Passed,
            FilterMode::Failed => status == TestStatus::Failed,
            FilterMode::Pending => status == TestStatus::Pending,
            FilterMode::Ignored => status == TestStatus::Ignored,
        }
    }

    pub fn cycle_next(&self) -> Self {
        match self {
            FilterMode::All => FilterMode::Passed,
            FilterMode::Passed => FilterMode::Failed,
            FilterMode::Failed => FilterMode::Pending,
            FilterMode::Pending => FilterMode::Ignored,
            FilterMode::Ignored => FilterMode::All,
        }
    }
}

/// Statistics for the test run
#[derive(Debug, Clone, Default)]
pub struct TestStats {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pending: usize,
    pub running: usize,
    pub ignored: usize,
}

impl TestStats {
    pub fn from_tree(tree: &TestNode) -> Self {
        let tests = tree.all_tests();
        let mut stats = Self::default();
        stats.total = tests.len();

        for test in tests {
            match test.status {
                TestStatus::Passed => stats.passed += 1,
                TestStatus::Failed => stats.failed += 1,
                TestStatus::Pending => stats.pending += 1,
                TestStatus::Running => stats.running += 1,
                TestStatus::Ignored => stats.ignored += 1,
            }
        }

        stats
    }

    pub fn pass_rate(&self) -> f64 {
        let completed = self.passed + self.failed;
        if completed == 0 {
            0.0
        } else {
            (self.passed as f64) / (completed as f64) * 100.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_name() {
        let test = Test::from_name("module::submodule::test_something");
        assert_eq!(test.short_name, "test_something");
        assert_eq!(test.module_path, vec!["module", "submodule"]);
        assert_eq!(test.full_name, "module::submodule::test_something");
    }

    #[test]
    fn test_tree_building() {
        let mut root = TestNode::new_module("root");

        root.add_test(Test::from_name("module_a::test_one"));
        root.add_test(Test::from_name("module_a::test_two"));
        root.add_test(Test::from_name("module_b::submod::test_three"));

        root.sort_children();
        root.update_counts();

        assert_eq!(root.test_count, 3);
        assert_eq!(root.children.len(), 2); // module_a, module_b
    }

    #[test]
    fn test_filter_tests() {
        let mut root = TestNode::new_module("root");
        root.add_test(Test::from_name("module::test_alpha"));
        root.add_test(Test::from_name("module::test_beta"));
        root.add_test(Test::from_name("other::test_gamma"));

        let filtered = root.filter_tests("alpha");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].short_name, "test_alpha");
    }
}
