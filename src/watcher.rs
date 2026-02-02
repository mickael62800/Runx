//! File watcher for automatic test re-running
//!
//! Watches for file changes and automatically re-runs affected tests.

use anyhow::Result;
use colored::Colorize;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use crate::affected::find_affected_from_files;
use crate::db::Database;
use crate::discovery::discover_all_tests;
use crate::test_model::TestNode;
use crate::test_runner::TestRunner;

const DEBOUNCE_MS: u128 = 300;
const EXCLUDED_DIRS: &[&str] = &["target", "node_modules", "dist", "out", ".git"];

/// Test watcher for automatic re-running
pub struct TestWatcher<'a> {
    project_dir: &'a Path,
    test_filter: Option<String>,
    db: Option<Database>,
    test_tree: Option<TestNode>,
}

impl<'a> TestWatcher<'a> {
    pub fn new(
        project_dir: &'a Path,
        test_filter: Option<String>,
        db: Option<Database>,
    ) -> Self {
        Self {
            project_dir,
            test_filter,
            db,
            test_tree: None,
        }
    }

    pub fn start(&mut self) -> Result<()> {
        // Initial test discovery
        println!("{} Discovering tests...", "🔍".cyan());
        self.test_tree = Some(discover_all_tests(self.project_dir)?);

        if let Some(ref tree) = self.test_tree {
            let count = tree.all_tests().len();
            println!("{} Found {} tests", "✓".green(), count);
        }

        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        )?;

        // Watch the project directory recursively
        watcher.watch(self.project_dir, RecursiveMode::Recursive)?;

        println!(
            "\n{} {} {}\n",
            "👀".cyan(),
            "Watching for changes in".bold(),
            self.project_dir.display()
        );

        if let Some(ref filter) = self.test_filter {
            println!("   Filtering for tests matching: {}\n", filter.cyan());
        }

        println!("{}", "Press Ctrl+C to stop\n".dimmed());

        self.event_loop(rx)?;

        Ok(())
    }

    fn event_loop(&mut self, rx: Receiver<Event>) -> Result<()> {
        let mut last_run = Instant::now() - Duration::from_secs(10);

        while let Ok(event) = rx.recv() {
            // Debounce
            if last_run.elapsed().as_millis() < DEBOUNCE_MS {
                continue;
            }

            // Process changed files
            let changed_files: Vec<String> = event
                .paths
                .iter()
                .filter_map(|p| {
                    let path_str = p.to_string_lossy().to_string();

                    // Skip excluded directories
                    if EXCLUDED_DIRS.iter().any(|exc| path_str.contains(exc)) {
                        return None;
                    }

                    // Only watch Rust files and Cargo.toml
                    if !path_str.ends_with(".rs") && !path_str.ends_with("Cargo.toml") {
                        return None;
                    }

                    // Convert to relative path
                    p.strip_prefix(self.project_dir)
                        .ok()
                        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                })
                .collect();

            if changed_files.is_empty() {
                continue;
            }

            last_run = Instant::now();

            // Re-discover tests if Cargo.toml changed
            if changed_files.iter().any(|f| f.ends_with("Cargo.toml")) {
                println!("\n{} Cargo.toml changed, re-discovering tests...", "↻".yellow());
                if let Ok(tree) = discover_all_tests(self.project_dir) {
                    self.test_tree = Some(tree);
                }
            }

            self.run_affected_tests(&changed_files)?;
        }

        Ok(())
    }

    fn run_affected_tests(&mut self, changed_files: &[String]) -> Result<()> {
        println!(
            "\n{} {} {}",
            "↻".yellow(),
            "Files changed:".bold(),
            changed_files.join(", ").dimmed()
        );

        // Find affected tests
        let affected = if let Some(ref tree) = self.test_tree {
            find_affected_from_files(changed_files, tree, self.project_dir)
        } else {
            Vec::new()
        };

        // Apply filter if specified
        let tests_to_run: Vec<String> = if let Some(ref filter) = self.test_filter {
            let filter_lower = filter.to_lowercase();
            affected.into_iter()
                .filter(|t| t.to_lowercase().contains(&filter_lower))
                .collect()
        } else {
            affected
        };

        if tests_to_run.is_empty() {
            println!("{}", "No matching tests to run".dimmed());
            return Ok(());
        }

        println!(
            "{} Running {} affected test(s)...\n",
            "→".blue(),
            tests_to_run.len()
        );

        // Run tests
        let runner = TestRunner::new(self.project_dir);

        // For efficiency, we run all affected tests at once using filter
        // This is faster than running them individually
        let filter = if tests_to_run.len() == 1 {
            tests_to_run[0].clone()
        } else {
            // Cargo test supports multiple filters separated by spaces
            // But for exact matching, we use the first common prefix
            find_common_prefix(&tests_to_run)
        };

        let result = runner.run_filtered(&filter)?;

        // Print summary
        println!();
        if result.failed > 0 {
            println!(
                "{} {} passed, {} failed",
                "✗".red(),
                result.passed.to_string().green(),
                result.failed.to_string().red()
            );

            // Show failed test names
            for test_result in &result.test_results {
                if test_result.status == crate::test_model::TestStatus::Failed {
                    println!("   {} {}", "✗".red(), test_result.name.red());
                }
            }
        } else {
            println!(
                "{} {} test(s) passed",
                "✓".green(),
                result.passed.to_string().green()
            );
        }

        println!("\n{}", "Watching for changes...".dimmed());

        Ok(())
    }
}

/// Find common prefix among test names for efficient filtering
fn find_common_prefix(names: &[String]) -> String {
    if names.is_empty() {
        return String::new();
    }

    if names.len() == 1 {
        return names[0].clone();
    }

    // Find shortest common prefix
    let first = &names[0];
    let mut prefix_len = first.len();

    for name in &names[1..] {
        let common = first.chars()
            .zip(name.chars())
            .take_while(|(a, b)| a == b)
            .count();
        prefix_len = prefix_len.min(common);
    }

    if prefix_len == 0 {
        // No common prefix, run all tests
        return String::new();
    }

    first[..prefix_len].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_common_prefix() {
        assert_eq!(
            find_common_prefix(&["module::test_one".to_string(), "module::test_two".to_string()]),
            "module::test_"
        );

        assert_eq!(
            find_common_prefix(&["foo::bar".to_string(), "foo::baz".to_string()]),
            "foo::ba"
        );

        assert_eq!(
            find_common_prefix(&["abc".to_string(), "xyz".to_string()]),
            ""
        );
    }
}
