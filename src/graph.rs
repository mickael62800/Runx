use anyhow::{bail, Result};
use std::collections::{HashMap, HashSet, VecDeque};

use crate::config::Config;

/// Build execution order using topological sort (Kahn's algorithm)
pub fn topological_sort(config: &Config, task_names: &[&str]) -> Result<Vec<String>> {
    // Build the subgraph of required tasks (including dependencies)
    let required_tasks = collect_required_tasks(config, task_names)?;

    // Build adjacency list and in-degree count for required tasks only
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

    for name in &required_tasks {
        in_degree.entry(name.as_str()).or_insert(0);
        dependents.entry(name.as_str()).or_insert_with(Vec::new);
    }

    for name in &required_tasks {
        if let Some(task) = config.get_task(name) {
            for dep in &task.depends_on {
                if required_tasks.contains(dep) {
                    *in_degree.entry(name.as_str()).or_insert(0) += 1;
                    dependents
                        .entry(dep.as_str())
                        .or_insert_with(Vec::new)
                        .push(name.as_str());
                }
            }
        }
    }

    // Kahn's algorithm
    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&name, _)| name)
        .collect();

    let mut result: Vec<String> = Vec::new();

    while let Some(current) = queue.pop_front() {
        result.push(current.to_string());

        if let Some(deps) = dependents.get(current) {
            for &dependent in deps {
                if let Some(deg) = in_degree.get_mut(dependent) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(dependent);
                    }
                }
            }
        }
    }

    if result.len() != required_tasks.len() {
        bail!("Circular dependency detected in task graph");
    }

    Ok(result)
}

/// Collect all tasks required to run the given tasks (including their dependencies)
fn collect_required_tasks(config: &Config, task_names: &[&str]) -> Result<HashSet<String>> {
    let mut required: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = task_names.iter().map(|&s| s.to_string()).collect();

    while let Some(name) = queue.pop_front() {
        if required.contains(&name) {
            continue;
        }

        if let Some(task) = config.get_task(&name) {
            required.insert(name.clone());
            for dep in &task.depends_on {
                if !required.contains(dep) {
                    queue.push_back(dep.clone());
                }
            }
        } else {
            bail!("Task '{}' not found", name);
        }
    }

    Ok(required)
}

/// Find tasks that watch the given file path
pub fn find_tasks_watching_file(config: &Config, file_path: &str) -> Vec<String> {
    let mut matching_tasks = Vec::new();

    for (name, task) in &config.tasks {
        for pattern in &task.watch {
            if let Ok(glob_pattern) = glob::Pattern::new(pattern) {
                if glob_pattern.matches(file_path) {
                    matching_tasks.push(name.clone());
                    break;
                }
            }
        }
    }

    matching_tasks
}

/// Get all tasks that depend on the given task (directly or indirectly)
pub fn get_dependent_tasks(config: &Config, task_name: &str) -> HashSet<String> {
    let mut dependents: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<String> = VecDeque::new();
    queue.push_back(task_name.to_string());

    while let Some(current) = queue.pop_front() {
        for (name, task) in &config.tasks {
            if task.depends_on.contains(&current) && !dependents.contains(name) {
                dependents.insert(name.clone());
                queue.push_back(name.clone());
            }
        }
    }

    dependents
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Project, Task};

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
            },
        );
        tasks.insert(
            "test".to_string(),
            Task {
                cmd: "cargo test".to_string(),
                cwd: None,
                watch: vec!["src/**/*.rs".to_string()],
                depends_on: vec!["build".to_string()],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
            },
        );
        tasks.insert(
            "lint".to_string(),
            Task {
                cmd: "cargo clippy".to_string(),
                cwd: None,
                watch: vec![],
                depends_on: vec![],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
            },
        );

        Config {
            project: Project {
                name: "test".to_string(),
            },
            tasks,
        }
    }

    #[test]
    fn test_topological_sort() {
        let config = create_test_config();
        let order = topological_sort(&config, &["test"]).unwrap();

        // build must come before test
        let build_pos = order.iter().position(|x| x == "build").unwrap();
        let test_pos = order.iter().position(|x| x == "test").unwrap();
        assert!(build_pos < test_pos);
    }

    #[test]
    fn test_get_dependent_tasks() {
        let config = create_test_config();
        let deps = get_dependent_tasks(&config, "build");
        assert!(deps.contains("test"));
        assert!(!deps.contains("lint"));
    }
}
