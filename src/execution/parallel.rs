//! Parallel task execution using tokio

use anyhow::Result;
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::config::{Config, Profile};
use crate::graph::topological_sort;
use crate::task::TaskResult;

/// Calculate execution levels for parallel execution
/// Tasks at the same level have no dependencies on each other
pub fn calculate_execution_levels(config: &Config, task_names: &[String]) -> Result<Vec<Vec<String>>> {
    // Get the full execution order with dependencies
    let task_refs: Vec<&str> = task_names.iter().map(|s| s.as_str()).collect();
    let all_tasks = topological_sort(config, &task_refs)?;

    // Build a set of tasks we need to run
    let task_set: HashSet<String> = all_tasks.iter().cloned().collect();

    // Build reverse dependency map: for each task, which tasks depend on it
    let mut dependents: HashMap<String, Vec<String>> = HashMap::new();
    let mut in_degree: HashMap<String, usize> = HashMap::new();

    for task_name in &all_tasks {
        in_degree.insert(task_name.clone(), 0);
        dependents.insert(task_name.clone(), Vec::new());
    }

    for task_name in &all_tasks {
        if let Some(task) = config.get_task(task_name) {
            for dep in &task.depends_on {
                if task_set.contains(dep) {
                    *in_degree.get_mut(task_name).unwrap() += 1;
                    dependents.get_mut(dep).unwrap().push(task_name.clone());
                }
            }
        }
    }

    // Calculate levels using modified Kahn's algorithm
    let mut levels: Vec<Vec<String>> = Vec::new();
    let mut remaining: HashSet<String> = all_tasks.iter().cloned().collect();

    while !remaining.is_empty() {
        // Find all tasks with in_degree 0
        let current_level: Vec<String> = remaining
            .iter()
            .filter(|task| in_degree.get(*task).copied().unwrap_or(0) == 0)
            .cloned()
            .collect();

        if current_level.is_empty() {
            anyhow::bail!("Circular dependency detected");
        }

        // Remove current level tasks and update in_degrees
        for task in &current_level {
            remaining.remove(task);
            if let Some(deps) = dependents.get(task) {
                for dep in deps.clone() {
                    if let Some(degree) = in_degree.get_mut(&dep) {
                        *degree = degree.saturating_sub(1);
                    }
                }
            }
        }

        levels.push(current_level);
    }

    Ok(levels)
}

/// Filter tasks that can run in parallel at a given level
pub fn filter_parallel_tasks(config: &Config, tasks: &[String], profile: &Profile) -> (Vec<String>, Vec<String>) {
    let mut parallel_tasks = Vec::new();
    let mut sequential_tasks = Vec::new();

    for task_name in tasks {
        if let Some(task) = config.get_task(task_name) {
            // Check if task is marked as parallel in config or profile
            let is_parallel = task.parallel || profile.parallel;
            // Background tasks cannot run in parallel with others
            let is_background = task.background;

            if is_parallel && !is_background {
                parallel_tasks.push(task_name.clone());
            } else {
                sequential_tasks.push(task_name.clone());
            }
        }
    }

    (parallel_tasks, sequential_tasks)
}

/// Parallel executor using tokio
pub struct ParallelExecutor {
    max_workers: usize,
}

impl ParallelExecutor {
    pub fn new(max_workers: usize) -> Self {
        Self {
            max_workers: max_workers.max(1),
        }
    }

    /// Execute tasks in parallel with a semaphore to limit concurrency
    pub async fn execute_parallel<F, Fut>(
        &self,
        tasks: Vec<String>,
        executor: F,
    ) -> Result<Vec<TaskResult>>
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<TaskResult>> + Send + 'static,
    {
        let semaphore = Arc::new(Semaphore::new(self.max_workers));
        let executor = Arc::new(executor);
        let mut join_set = JoinSet::new();

        println!(
            "{} {} task(s) with {} workers",
            "⚡".yellow(),
            "Running in parallel:".bold(),
            self.max_workers
        );

        for task_name in tasks {
            let sem = semaphore.clone();
            let exec = executor.clone();

            join_set.spawn(async move {
                let _permit = sem.acquire().await.expect("Semaphore closed");
                exec(task_name).await
            });
        }

        let mut results = Vec::new();
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(task_result)) => results.push(task_result),
                Ok(Err(e)) => {
                    eprintln!("{} Task failed: {}", "✗".red(), e);
                    // Continue with other tasks
                }
                Err(e) => {
                    eprintln!("{} Task panicked: {}", "✗".red(), e);
                }
            }
        }

        Ok(results)
    }
}

/// Statistics for parallel execution
#[derive(Debug, Default)]
pub struct ParallelStats {
    pub total_tasks: usize,
    pub parallel_tasks: usize,
    pub sequential_tasks: usize,
    pub levels: usize,
    pub max_parallelism: usize,
}

impl ParallelStats {
    pub fn from_levels(levels: &[Vec<String>], config: &Config, profile: &Profile) -> Self {
        let total_tasks: usize = levels.iter().map(|l| l.len()).sum();
        let mut parallel_tasks = 0;
        let mut sequential_tasks = 0;
        let mut max_parallelism = 0;

        for level in levels {
            let (parallel, sequential) = filter_parallel_tasks(config, level, profile);
            parallel_tasks += parallel.len();
            sequential_tasks += sequential.len();
            max_parallelism = max_parallelism.max(parallel.len());
        }

        Self {
            total_tasks,
            parallel_tasks,
            sequential_tasks,
            levels: levels.len(),
            max_parallelism,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Project, Task};
    use std::collections::HashMap;

    fn create_test_config() -> Config {
        let mut tasks = HashMap::new();

        // Level 0: no dependencies
        tasks.insert(
            "build-a".to_string(),
            Task {
                cmd: "echo a".to_string(),
                cwd: None,
                watch: vec![],
                depends_on: vec![],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: true,
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
            "build-b".to_string(),
            Task {
                cmd: "echo b".to_string(),
                cwd: None,
                watch: vec![],
                depends_on: vec![],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: true,
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

        // Level 1: depends on build-a and build-b
        tasks.insert(
            "test".to_string(),
            Task {
                cmd: "echo test".to_string(),
                cwd: None,
                watch: vec![],
                depends_on: vec!["build-a".to_string(), "build-b".to_string()],
                background: false,
                ready_when: None,
                ready_timeout: 30,
                category: None,
                results: None,
                parallel: true,
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
    fn test_execution_levels() {
        let config = create_test_config();
        let levels = calculate_execution_levels(
            &config,
            &vec!["test".to_string()],
        ).unwrap();

        assert_eq!(levels.len(), 2);
        // Level 0 should have build-a and build-b
        assert!(levels[0].contains(&"build-a".to_string()) || levels[0].contains(&"build-b".to_string()));
        // Level 1 should have test
        assert!(levels[1].contains(&"test".to_string()));
    }

    #[test]
    fn test_parallel_stats() {
        let config = create_test_config();
        let profile = Profile::default();
        let levels = calculate_execution_levels(
            &config,
            &vec!["test".to_string()],
        ).unwrap();

        let stats = ParallelStats::from_levels(&levels, &config, &profile);
        assert_eq!(stats.total_tasks, 3);
        assert_eq!(stats.levels, 2);
    }
}
