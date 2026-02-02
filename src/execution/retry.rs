//! Retry logic with flaky detection

use anyhow::Result;
use colored::Colorize;
use std::path::Path;
use std::time::Duration;

use crate::config::Task;
use crate::db::Database;
use crate::task::{execute_task, TaskResult};

/// Retry configuration for a task
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub delay_ms: u64,
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 0,
            delay_ms: 1000,
            exponential_backoff: false,
        }
    }
}

impl From<&Task> for RetryConfig {
    fn from(task: &Task) -> Self {
        Self {
            max_retries: task.retry,
            delay_ms: task.retry_delay_ms,
            exponential_backoff: false,
        }
    }
}

/// Execute a task with retry logic
pub fn execute_with_retry(
    name: &str,
    task: &Task,
    base_dir: &Path,
    retry_config: &RetryConfig,
) -> Result<TaskResult> {
    let mut attempts = 0;
    let mut last_result: Option<TaskResult> = None;

    loop {
        attempts += 1;

        if attempts > 1 {
            let delay = if retry_config.exponential_backoff {
                retry_config.delay_ms * 2u64.pow(attempts - 2)
            } else {
                retry_config.delay_ms
            };

            println!(
                "{} {} attempt {} of {} (waiting {}ms)",
                "↻".yellow(),
                "Retrying".bold(),
                attempts,
                retry_config.max_retries + 1,
                delay
            );

            std::thread::sleep(Duration::from_millis(delay));
        }

        let result = execute_task(name, task, base_dir)?;

        if result.success {
            // If this was a retry, mark it as potentially flaky
            if attempts > 1 {
                println!(
                    "{} {} passed on attempt {}",
                    "⚠".yellow(),
                    name.cyan(),
                    attempts
                );
            }
            return Ok(result);
        }

        last_result = Some(result);

        if attempts > retry_config.max_retries {
            break;
        }
    }

    // All retries exhausted
    Ok(last_result.unwrap())
}

/// Execute a task with retry and flaky tracking
pub fn execute_with_flaky_tracking(
    name: &str,
    task: &Task,
    base_dir: &Path,
    retry_config: &RetryConfig,
    db: Option<&Database>,
    run_id: &str,
) -> Result<(TaskResult, FlakyInfo)> {
    let mut attempts = 0;
    let mut results: Vec<bool> = Vec::new();
    let mut last_result: Option<TaskResult> = None;

    loop {
        attempts += 1;

        if attempts > 1 {
            let delay = if retry_config.exponential_backoff {
                retry_config.delay_ms * 2u64.pow(attempts - 2)
            } else {
                retry_config.delay_ms
            };

            println!(
                "{} {} attempt {} of {} (waiting {}ms)",
                "↻".yellow(),
                "Retrying".bold(),
                attempts,
                retry_config.max_retries + 1,
                delay
            );

            std::thread::sleep(Duration::from_millis(delay));
        }

        let result = execute_task(name, task, base_dir)?;
        results.push(result.success);

        // Track test history for flaky detection
        if let Some(db) = db {
            // Record task-level history
            let status = if result.success { "passed" } else { "failed" };
            let _ = db.record_test_history(
                name,
                task.category.as_deref(),
                name,
                status,
                Some(result.duration_ms as i64),
                run_id,
            );
        }

        if result.success {
            let flaky_info = FlakyInfo {
                attempts,
                passed_on_retry: attempts > 1,
                is_flaky: results.len() > 1 && results.iter().any(|&r| r) && results.iter().any(|&r| !r),
            };

            if flaky_info.is_flaky {
                println!(
                    "{} {} is flaky (passed on attempt {}/{})",
                    "⚠".yellow(),
                    name.cyan(),
                    attempts,
                    retry_config.max_retries + 1
                );
            }

            return Ok((result, flaky_info));
        }

        last_result = Some(result);

        if attempts > retry_config.max_retries {
            break;
        }
    }

    let flaky_info = FlakyInfo {
        attempts,
        passed_on_retry: false,
        is_flaky: false,
    };

    Ok((last_result.unwrap(), flaky_info))
}

/// Information about flaky behavior during execution
#[derive(Debug, Clone)]
pub struct FlakyInfo {
    pub attempts: u32,
    pub passed_on_retry: bool,
    pub is_flaky: bool,
}

/// Check if a test should be quarantined based on flaky history
pub fn should_quarantine(db: &Database, test_name: &str, threshold: f64) -> Result<bool> {
    let score = db.calculate_flaky_score(test_name)?;

    // If score is between 20-80%, the test is flaky
    // A perfectly stable test would be 0% (always fails) or 100% (always passes)
    let is_flaky = score > 20.0 && score < 80.0;

    // If the test has been run enough times and is consistently flaky, quarantine it
    if is_flaky {
        let flaky_tests = db.get_flaky_tests(100)?;
        if let Some(test) = flaky_tests.iter().find(|t| t.test_name == test_name) {
            // Quarantine if:
            // 1. Run at least 10 times
            // 2. Flaky score is within threshold of 50%
            if test.total_runs >= 10 {
                let distance_from_50 = (test.flaky_score - 50.0).abs();
                return Ok(distance_from_50 < threshold);
            }
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_task() -> Task {
        Task {
            cmd: "echo test".to_string(),
            cwd: None,
            watch: vec![],
            depends_on: vec![],
            background: false,
            ready_when: None,
            ready_timeout: 30,
            category: None,
            results: None,
            parallel: false,
            workers: None,
            retry: 2,
            retry_delay_ms: 100,
            timeout_seconds: None,
            coverage: false,
            coverage_format: None,
            coverage_path: None,
            coverage_threshold: None,
            artifacts: vec![],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![],
        }
    }

    #[test]
    fn test_retry_config_from_task() {
        let task = create_test_task();
        let config = RetryConfig::from(&task);

        assert_eq!(config.max_retries, 2);
        assert_eq!(config.delay_ms, 100);
    }
}
