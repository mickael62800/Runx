//! Test discovery module
//!
//! Discovers tests by parsing output from `cargo test -- --list`
//! and builds a hierarchical test tree.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

use crate::test_model::{Test, TestNode, TestStatus};

/// Discover all tests in the project
pub fn discover_tests(project_dir: &Path) -> Result<TestNode> {
    let output = run_cargo_test_list(project_dir)?;
    let tests = parse_test_list(&output);

    let mut root = TestNode::new_module("tests");

    for test in tests {
        root.add_test(test);
    }

    root.sort_children();
    root.update_counts();

    Ok(root)
}

/// Discover tests matching a filter pattern
pub fn discover_tests_filtered(project_dir: &Path, filter: &str) -> Result<TestNode> {
    let output = run_cargo_test_list_filtered(project_dir, filter)?;
    let tests = parse_test_list(&output);

    let mut root = TestNode::new_module("tests");

    for test in tests {
        root.add_test(test);
    }

    root.sort_children();
    root.update_counts();

    Ok(root)
}

/// Run `cargo test -- --list` and capture output
fn run_cargo_test_list(project_dir: &Path) -> Result<String> {
    let output = Command::new("cargo")
        .args(["test", "--", "--list"])
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute cargo test --list")?;

    // Cargo test --list outputs to stdout
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    // Also check stderr for any errors
    if !output.status.success() && stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cargo test --list failed: {}", stderr);
    }

    Ok(stdout)
}

/// Run `cargo test <filter> -- --list` and capture output
fn run_cargo_test_list_filtered(project_dir: &Path, filter: &str) -> Result<String> {
    let output = Command::new("cargo")
        .args(["test", filter, "--", "--list"])
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute cargo test --list")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() && stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cargo test --list failed: {}", stderr);
    }

    Ok(stdout)
}

/// Parse the output of `cargo test -- --list`
///
/// Output format:
/// ```text
/// module::submodule::test_name: test
/// other::test_name: test
///
/// 2 tests, 0 benchmarks
/// ```
fn parse_test_list(output: &str) -> Vec<Test> {
    let mut tests = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // Skip empty lines and summary lines
        if line.is_empty() {
            continue;
        }

        // Skip summary line "X tests, Y benchmarks"
        if line.contains(" tests,") || line.contains(" test,") {
            continue;
        }

        // Skip doc tests header
        if line.starts_with("Doc-tests") {
            continue;
        }

        // Parse test line: "test_name: test" or "module::test_name: test"
        if let Some((name, suffix)) = line.rsplit_once(": ") {
            let suffix = suffix.trim().to_lowercase();

            // Create test with appropriate status
            let mut test = Test::from_name(name);

            // Check if it's an ignored test
            if suffix == "test" {
                test.status = TestStatus::Pending;
            } else if suffix == "bench" {
                // Skip benchmarks for now
                continue;
            }

            tests.push(test);
        }
    }

    tests
}

/// Check if a directory is a Rust project
pub fn is_rust_project(dir: &Path) -> bool {
    dir.join("Cargo.toml").exists()
}

/// Get project name from Cargo.toml
pub fn get_project_name(project_dir: &Path) -> Result<String> {
    let cargo_toml_path = project_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml_path)
        .context("Failed to read Cargo.toml")?;

    // Simple parsing - look for name = "..." in [package] section
    let mut in_package = false;
    for line in content.lines() {
        let line = line.trim();
        if line == "[package]" {
            in_package = true;
            continue;
        }
        if line.starts_with('[') {
            in_package = false;
            continue;
        }
        if in_package && line.starts_with("name") {
            if let Some((_, value)) = line.split_once('=') {
                let name = value.trim().trim_matches('"').trim_matches('\'');
                return Ok(name.to_string());
            }
        }
    }

    // Fallback to directory name
    Ok(project_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string()))
}

/// Build ignored tests list from `cargo test -- --ignored --list`
pub fn discover_ignored_tests(project_dir: &Path) -> Result<Vec<String>> {
    let output = Command::new("cargo")
        .args(["test", "--", "--ignored", "--list"])
        .current_dir(project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute cargo test --ignored --list")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    let mut ignored = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if let Some((name, _suffix)) = line.rsplit_once(": ") {
            ignored.push(name.to_string());
        }
    }

    Ok(ignored)
}

/// Mark ignored tests in the tree
pub fn mark_ignored_tests(root: &mut TestNode, ignored_names: &[String]) {
    for name in ignored_names {
        if let Some(test) = root.find_test_mut(name) {
            test.status = TestStatus::Ignored;
        }
    }
    root.update_counts();
}

/// Full discovery: tests + ignored tests
pub fn discover_all_tests(project_dir: &Path) -> Result<TestNode> {
    let mut root = discover_tests(project_dir)?;

    // Try to get ignored tests
    if let Ok(ignored) = discover_ignored_tests(project_dir) {
        mark_ignored_tests(&mut root, &ignored);
    }

    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_test_list() {
        let output = r#"
module::tests::test_one: test
module::tests::test_two: test
other::test_three: test

3 tests, 0 benchmarks
"#;

        let tests = parse_test_list(output);
        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].full_name, "module::tests::test_one");
        assert_eq!(tests[0].short_name, "test_one");
        assert_eq!(tests[0].module_path, vec!["module", "tests"]);
    }

    #[test]
    fn test_parse_empty_output() {
        let output = "";
        let tests = parse_test_list(output);
        assert!(tests.is_empty());
    }

    #[test]
    fn test_parse_with_doc_tests() {
        let output = r#"
test_one: test
test_two: test

2 tests, 0 benchmarks

Doc-tests myproject

myproject::foo (line 10): test

1 tests, 0 benchmarks
"#;

        let tests = parse_test_list(output);
        // Should parse doc test too
        assert!(tests.len() >= 2);
    }
}
