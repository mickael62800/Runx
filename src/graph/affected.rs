//! Detection of affected tasks based on git changes

use anyhow::Result;
use std::collections::HashSet;
use std::path::Path;

use crate::config::Config;
use crate::git::diff::GitDiff;
use super::toposort::{find_tasks_watching_file, get_dependent_tasks};

/// Find tasks affected by changes since a given git reference
pub fn find_affected_tasks(
    config: &Config,
    base_dir: &Path,
    since: Option<&str>,
    base: Option<&str>,
) -> Result<Vec<String>> {
    // Get changed files from git
    let diff = GitDiff::new(base_dir)?;
    let changed_files = diff.get_changed_files(since, base)?;

    find_affected_tasks_from_files(config, &changed_files)
}

/// Find tasks affected by a list of changed files
pub fn find_affected_tasks_from_files(
    config: &Config,
    changed_files: &[String],
) -> Result<Vec<String>> {
    let mut affected: HashSet<String> = HashSet::new();

    for file in changed_files {
        // Find tasks watching this file
        let watching = find_tasks_watching_file(config, file);

        for task_name in watching {
            affected.insert(task_name.clone());

            // Add all tasks that depend on this task
            let dependents = get_dependent_tasks(config, &task_name);
            affected.extend(dependents);
        }
    }

    // Also check if any task's input files were changed
    for (task_name, task) in &config.tasks {
        for input in &task.inputs {
            // Check if any changed file matches this input pattern
            for file in changed_files {
                if let Ok(pattern) = glob::Pattern::new(input) {
                    if pattern.matches(file) {
                        affected.insert(task_name.clone());
                        let dependents = get_dependent_tasks(config, task_name);
                        affected.extend(dependents);
                        break;
                    }
                }
            }
        }
    }

    let mut result: Vec<String> = affected.into_iter().collect();
    result.sort();

    Ok(result)
}

/// Filter to include only tasks that match a pattern
pub fn filter_tasks_by_pattern(tasks: &[String], pattern: &str) -> Vec<String> {
    if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
        tasks
            .iter()
            .filter(|t| glob_pattern.matches(t))
            .cloned()
            .collect()
    } else {
        // If pattern is invalid, treat it as exact match
        tasks
            .iter()
            .filter(|t| t.contains(pattern))
            .cloned()
            .collect()
    }
}

/// Get tasks that would be affected by changes to specific files
pub fn preview_affected(config: &Config, files: &[&str]) -> Vec<AffectedPreview> {
    let mut previews = Vec::new();

    for file in files {
        let watching = find_tasks_watching_file(config, file);
        let mut all_affected: HashSet<String> = HashSet::new();

        for task_name in &watching {
            all_affected.insert(task_name.clone());
            let dependents = get_dependent_tasks(config, task_name);
            all_affected.extend(dependents);
        }

        previews.push(AffectedPreview {
            file: file.to_string(),
            directly_affected: watching,
            transitively_affected: all_affected.into_iter().collect(),
        });
    }

    previews
}

/// Preview of tasks affected by a file change
#[derive(Debug, Clone)]
pub struct AffectedPreview {
    pub file: String,
    pub directly_affected: Vec<String>,
    pub transitively_affected: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Project, Task};
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut tasks = HashMap::new();

        tasks.insert(
            "build".to_string(),
            Task {
                cmd: "cargo build".to_string(),
                cwd: None,
                watch: vec!["src/**/*.rs".to_string()],
                depends_on: vec![],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: false,
                workers: None,
                retry: 0,
                retry_delay_ms: 1000,
                timeout_seconds: None,
                coverage: false,
                coverage_format: None,
                coverage_path: None,
                coverage_threshold: None,
                artifacts: vec![],
                env: HashMap::new(),
                inputs: vec![],
                outputs: vec![],
            },
        );

        tasks.insert(
            "test".to_string(),
            Task {
                cmd: "cargo test".to_string(),
                cwd: None,
                watch: vec!["src/**/*.rs".to_string(), "tests/**/*.rs".to_string()],
                depends_on: vec!["build".to_string()],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: false,
                workers: None,
                retry: 0,
                retry_delay_ms: 1000,
                timeout_seconds: None,
                coverage: false,
                coverage_format: None,
                coverage_path: None,
                coverage_threshold: None,
                artifacts: vec![],
                env: HashMap::new(),
                inputs: vec![],
                outputs: vec![],
            },
        );

        tasks.insert(
            "lint".to_string(),
            Task {
                cmd: "cargo clippy".to_string(),
                cwd: None,
                watch: vec!["src/**/*.rs".to_string()],
                depends_on: vec![],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: false,
                workers: None,
                retry: 0,
                retry_delay_ms: 1000,
                timeout_seconds: None,
                coverage: false,
                coverage_format: None,
                coverage_path: None,
                coverage_threshold: None,
                artifacts: vec![],
                env: HashMap::new(),
                inputs: vec![],
                outputs: vec![],
            },
        );

        Config {
            project: Project {
                name: "test".to_string(),
                default_profile: None,
            },
            profiles: HashMap::new(),
            workspaces: None,
            notifications: None,
            cache: None,
            ai: None,
            tasks,
        }
    }

    #[test]
    fn test_find_affected_from_files() {
        let config = create_test_config();
        let changed = vec!["src/main.rs".to_string()];

        let affected = find_affected_tasks_from_files(&config, &changed).unwrap();

        // build, test, and lint all watch src/**/*.rs
        assert!(affected.contains(&"build".to_string()));
        assert!(affected.contains(&"test".to_string()));
        assert!(affected.contains(&"lint".to_string()));
    }

    #[test]
    fn test_find_affected_test_only() {
        let config = create_test_config();
        let changed = vec!["tests/integration.rs".to_string()];

        let affected = find_affected_tasks_from_files(&config, &changed).unwrap();

        // Only test watches tests/**/*.rs
        assert!(affected.contains(&"test".to_string()));
        assert!(!affected.contains(&"build".to_string()));
        assert!(!affected.contains(&"lint".to_string()));
    }

    #[test]
    fn test_preview_affected() {
        let config = create_test_config();
        let previews = preview_affected(&config, &["src/lib.rs"]);

        assert_eq!(previews.len(), 1);
        assert!(!previews[0].directly_affected.is_empty());
    }
}
