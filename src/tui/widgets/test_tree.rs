//! Test tree widget for displaying hierarchical tests

use ratatui::{
    prelude::*,
    widgets::{Block, StatefulWidget},
};

use crate::test_model::{FilterMode, TestNode, TestStatus};

/// State for test tree widget
#[derive(Default)]
pub struct TestTreeState {
    /// Currently selected index in the flattened view
    pub selected: usize,
    /// Scroll offset
    pub offset: usize,
    /// Flattened list of visible items
    pub visible_items: Vec<TreeItem>,
}

impl TestTreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Move selection up
    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn down(&mut self, max: usize) {
        if self.selected < max.saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Get currently selected item
    pub fn selected_item(&self) -> Option<&TreeItem> {
        self.visible_items.get(self.selected)
    }

    /// Ensure selected item is visible
    pub fn ensure_visible(&mut self, visible_height: usize) {
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + visible_height {
            self.offset = self.selected - visible_height + 1;
        }
    }
}

/// A flattened item in the tree view
#[derive(Debug, Clone)]
pub struct TreeItem {
    /// Display name
    pub name: String,
    /// Full test name (if this is a test)
    pub full_name: Option<String>,
    /// Indentation level
    pub depth: usize,
    /// Whether this is a module (can be expanded)
    pub is_module: bool,
    /// Whether this module is expanded
    pub expanded: bool,
    /// Test status
    pub status: TestStatus,
    /// Test count (for modules)
    pub test_count: usize,
    /// Passed count (for modules)
    pub passed_count: usize,
    /// Failed count (for modules)
    pub failed_count: usize,
    /// Path to this node (module names)
    pub path: Vec<String>,
}

/// Test tree widget
pub struct TestTree<'a> {
    tree: &'a TestNode,
    block: Option<Block<'a>>,
    filter: &'a str,
    filter_mode: FilterMode,
}

impl<'a> TestTree<'a> {
    pub fn new(tree: &'a TestNode) -> Self {
        Self {
            tree,
            block: None,
            filter: "",
            filter_mode: FilterMode::All,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    pub fn filter(mut self, filter: &'a str) -> Self {
        self.filter = filter;
        self
    }

    pub fn filter_mode(mut self, mode: FilterMode) -> Self {
        self.filter_mode = mode;
        self
    }

    /// Flatten the tree into visible items
    fn flatten_tree(&self) -> Vec<TreeItem> {
        let mut items = Vec::new();
        self.flatten_node(self.tree, 0, &mut items, Vec::new());
        items
    }

    fn flatten_node(
        &self,
        node: &TestNode,
        depth: usize,
        items: &mut Vec<TreeItem>,
        path: Vec<String>,
    ) {
        // Skip root node, start with its children
        if depth == 0 {
            for child in &node.children {
                let mut child_path = path.clone();
                child_path.push(child.name.clone());
                self.flatten_node(child, 0, items, child_path);
            }
            return;
        }

        // Check if this node matches the filter
        let matches_filter = if self.filter.is_empty() {
            true
        } else {
            let filter_lower = self.filter.to_lowercase();
            node.name.to_lowercase().contains(&filter_lower)
                || node.all_tests().iter().any(|t| t.full_name.to_lowercase().contains(&filter_lower))
        };

        // Check status filter
        let matches_status = if node.is_test() {
            if let Some(ref test) = node.test {
                self.filter_mode.matches(test.status)
            } else {
                true
            }
        } else {
            // For modules, show if any child matches
            node.all_tests().iter().any(|t| self.filter_mode.matches(t.status))
        };

        if !matches_filter || !matches_status {
            return;
        }

        let item = TreeItem {
            name: node.name.clone(),
            full_name: node.test.as_ref().map(|t| t.full_name.clone()),
            depth,
            is_module: node.is_module(),
            expanded: node.expanded,
            status: node.status,
            test_count: node.test_count,
            passed_count: node.passed_count,
            failed_count: node.failed_count,
            path: path.clone(),
        };

        items.push(item);

        // Add children if expanded (or if module has only one child, auto-expand)
        if node.is_module() && node.expanded {
            for child in &node.children {
                let mut child_path = path.clone();
                child_path.push(child.name.clone());
                self.flatten_node(child, depth + 1, items, child_path);
            }
        }
    }
}

impl<'a> StatefulWidget for TestTree<'a> {
    type State = TestTreeState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Flatten tree into visible items (before taking self.block)
        state.visible_items = self.flatten_tree();

        let inner_area = match self.block {
            Some(b) => {
                let inner = b.inner(area);
                b.render(area, buf);
                inner
            }
            None => area,
        };

        if state.visible_items.is_empty() {
            let empty_msg = if !self.filter.is_empty() {
                "No tests match filter"
            } else {
                "No tests found"
            };
            buf.set_string(
                inner_area.x + 2,
                inner_area.y + 1,
                empty_msg,
                Style::default().fg(Color::DarkGray),
            );
            return;
        }

        // Ensure selection is valid
        if state.selected >= state.visible_items.len() {
            state.selected = state.visible_items.len().saturating_sub(1);
        }

        let visible_height = inner_area.height as usize;
        state.ensure_visible(visible_height);

        // Render visible items
        for (i, item) in state
            .visible_items
            .iter()
            .skip(state.offset)
            .take(visible_height)
            .enumerate()
        {
            let y = inner_area.y + i as u16;
            let is_selected = state.selected == state.offset + i;

            let mut x = inner_area.x;

            // Selection indicator
            if is_selected {
                buf.set_string(x, y, "│", Style::default().fg(Color::Cyan));
            }
            x += 1;

            // Indentation
            let indent = "  ".repeat(item.depth);
            buf.set_string(x, y, &indent, Style::default());
            x += indent.len() as u16;

            // Expand/collapse indicator for modules
            if item.is_module {
                let indicator = if item.expanded { "▼" } else { "▶" };
                buf.set_string(x, y, indicator, Style::default().fg(Color::Cyan));
            } else {
                buf.set_string(x, y, " ", Style::default());
            }
            x += 2;

            // Status symbol
            let status_symbol = item.status.symbol();
            let status_color = item.status.color();
            buf.set_string(x, y, status_symbol, Style::default().fg(status_color));
            x += 2;

            // Name
            let name_style = if is_selected {
                Style::default().bg(Color::DarkGray).bold()
            } else {
                Style::default()
            };

            let max_name_width = (inner_area.width as usize).saturating_sub((x - inner_area.x) as usize + 15);
            let display_name = if item.name.len() > max_name_width && max_name_width > 3 {
                format!("{}...", &item.name[..max_name_width - 3])
            } else {
                item.name.clone()
            };
            buf.set_string(x, y, &display_name, name_style);
            x += display_name.len() as u16 + 1;

            // Stats for modules
            if item.is_module && item.test_count > 0 {
                let stats = format!(
                    "[{}/{}]",
                    item.passed_count,
                    item.test_count
                );
                let stats_color = if item.failed_count > 0 {
                    Color::Red
                } else if item.passed_count == item.test_count {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                buf.set_string(x, y, &stats, Style::default().fg(stats_color));
            }
        }
    }
}

/// Helper to toggle expansion of a node by path
pub fn toggle_node_expansion(tree: &mut TestNode, path: &[String]) {
    if path.is_empty() {
        return;
    }

    let mut current = tree;
    for (i, segment) in path.iter().enumerate() {
        if let Some(child) = current.children.iter_mut().find(|c| &c.name == segment) {
            if i == path.len() - 1 {
                // This is the target node
                child.toggle_expanded();
                return;
            }
            current = child;
        } else {
            return;
        }
    }
}

/// Expand all nodes in the tree
pub fn expand_all(tree: &mut TestNode) {
    tree.expanded = true;
    for child in &mut tree.children {
        expand_all(child);
    }
}

/// Collapse all nodes in the tree
pub fn collapse_all(tree: &mut TestNode) {
    tree.expanded = false;
    for child in &mut tree.children {
        collapse_all(child);
    }
}
