//! Main task runner with support for parallel execution and caching

use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, Semaphore};
use uuid::Uuid;

use crate::config::{Config, Profile, Task};
use crate::db::{Database, TaskResult as DbTaskResult};
use crate::execution::{
    cache::CacheManager,
    parallel::{calculate_execution_levels, filter_parallel_tasks},
    retry::{execute_with_retry, RetryConfig},
};
use crate::graph::topological_sort;
use crate::junit;
use crate::server::WsMessage;
use crate::task::{execute_task, execute_task_async, start_background_task, BackgroundProcess, TaskResult};

/// Options for running tasks
#[derive(Debug, Clone, Default)]
pub struct RunOptions {
    /// Enable parallel execution
    pub parallel: bool,
    /// Number of parallel workers
    pub workers: usize,
    /// Use cache
    pub use_cache: bool,
    /// Stop on first failure
    pub fail_fast: bool,
    /// Include quarantined tests
    pub include_quarantined: bool,
    /// Profile name to use
    pub profile: Option<String>,
    /// Verbose output
    pub verbose: bool,
}

impl RunOptions {
    pub fn from_profile(profile: &Profile) -> Self {
        Self {
            parallel: profile.parallel,
            workers: profile.workers.unwrap_or(4),
            use_cache: profile.cache,
            fail_fast: profile.fail_fast,
            include_quarantined: false,
            profile: None,
            verbose: profile.verbose,
        }
    }
}

/// Enhanced task runner with parallel execution and caching
pub struct Runner<'a> {
    config: &'a Config,
    base_dir: &'a Path,
    db: Option<Database>,
    ws_tx: Option<broadcast::Sender<WsMessage>>,
    options: RunOptions,
}

impl<'a> Runner<'a> {
    pub fn new(
        config: &'a Config,
        base_dir: &'a Path,
        db: Option<Database>,
        ws_tx: Option<broadcast::Sender<WsMessage>>,
    ) -> Self {
        Self {
            config,
            base_dir,
            db,
            ws_tx,
            options: RunOptions::default(),
        }
    }

    pub fn with_options(mut self, options: RunOptions) -> Self {
        self.options = options;
        self
    }

    /// Run a single task with its dependencies
    pub fn run_task(&self, task_name: &str) -> Result<Vec<TaskResult>> {
        let execution_order = topological_sort(self.config, &[task_name])?;
        self.execute_tasks(&execution_order)
    }

    /// Run all tasks in the correct order
    pub fn run_all(&self) -> Result<Vec<TaskResult>> {
        let all_tasks: Vec<&str> = self.config.task_names().iter().map(|s| s.as_str()).collect();
        let execution_order = topological_sort(self.config, &all_tasks)?;
        self.execute_tasks(&execution_order)
    }

    /// Run specific tasks
    pub fn run_tasks(&self, task_names: &[String]) -> Result<Vec<TaskResult>> {
        let task_refs: Vec<&str> = task_names.iter().map(|s| s.as_str()).collect();
        let execution_order = topological_sort(self.config, &task_refs)?;
        self.execute_tasks(&execution_order)
    }

    fn execute_tasks(&self, task_names: &[String]) -> Result<Vec<TaskResult>> {
        let mut results = Vec::new();
        let mut background_processes: Vec<BackgroundProcess> = Vec::new();

        // Create run in database
        let run_id = Uuid::new_v4().to_string();
        if let Some(ref db) = self.db {
            db.create_run(&run_id, task_names.len() as i32)?;
        }

        // Broadcast run started
        if let Some(ref tx) = self.ws_tx {
            let _ = tx.send(WsMessage::RunStarted {
                run_id: run_id.clone(),
                total_tasks: task_names.len() as i32,
            });
        }

        println!(
            "\n{} {} task(s) to run: {}\n",
            "→".blue(),
            task_names.len(),
            task_names.join(", ").dimmed()
        );

        // Setup cache manager
        let cache_manager = self.db.as_ref().map(|db| {
            CacheManager::new(
                db,
                self.config.get_cache_ttl(),
                self.options.use_cache && self.config.is_cache_enabled(),
            )
        });

        let mut passed = 0;
        let mut failed = 0;
        let mut cached = 0;

        // Get profile
        let profile = self.config.get_profile(self.options.profile.as_deref());

        if self.options.parallel {
            // Parallel execution using tokio runtime
            let levels = calculate_execution_levels(self.config, task_names)?;

            println!(
                "{} {} execution levels, {} workers",
                "⚡".yellow(),
                levels.len(),
                self.options.workers
            );

            let rt = tokio::runtime::Runtime::new()?;
            let semaphore = Arc::new(Semaphore::new(self.options.workers));

            for (level_idx, level_tasks) in levels.iter().enumerate() {
                if self.options.verbose {
                    println!(
                        "\n{} Level {}: {}",
                        "→".blue(),
                        level_idx,
                        level_tasks.join(", ").cyan()
                    );
                }

                let (parallel_tasks, sequential) = filter_parallel_tasks(self.config, level_tasks, &profile);

                // Run parallel tasks concurrently
                if !parallel_tasks.is_empty() {
                    println!(
                        "{} Running {} task(s) in parallel",
                        "⚡".yellow(),
                        parallel_tasks.len()
                    );

                    let parallel_results = rt.block_on(async {
                        self.execute_parallel_tasks(
                            &parallel_tasks,
                            &run_id,
                            semaphore.clone(),
                        ).await
                    })?;

                    for result in parallel_results {
                        if result.success {
                            passed += 1;
                        } else {
                            failed += 1;
                        }
                        results.push(result);
                    }

                    if failed > 0 && self.options.fail_fast {
                        stop_background_processes(&mut background_processes);
                        break;
                    }
                }

                // Run sequential tasks
                for task_name in &sequential {
                    let task_result = self.execute_single_task(
                        task_name,
                        &run_id,
                        cache_manager.as_ref(),
                        &mut background_processes,
                    )?;

                    if let Some(result) = task_result {
                        if result.success {
                            passed += 1;
                        } else {
                            failed += 1;
                            if self.options.fail_fast {
                                stop_background_processes(&mut background_processes);
                                break;
                            }
                        }
                        results.push(result);
                    } else {
                        cached += 1;
                    }
                }

                if failed > 0 && self.options.fail_fast {
                    break;
                }
            }
        } else {
            // Sequential execution (original behavior)
            for name in task_names {
                let task_result = self.execute_single_task(
                    name,
                    &run_id,
                    cache_manager.as_ref(),
                    &mut background_processes,
                )?;

                if let Some(result) = task_result {
                    if result.success {
                        passed += 1;
                    } else {
                        failed += 1;
                        if self.options.fail_fast {
                            println!("{} Stopping due to task failure", "!".yellow());
                            stop_background_processes(&mut background_processes);
                            break;
                        }
                    }
                    results.push(result);
                } else {
                    cached += 1;
                }
            }
        }

        // Stop all background processes when done
        if !background_processes.is_empty() {
            println!("{}", "─".repeat(50).dimmed());
            stop_background_processes(&mut background_processes);
        }

        // Finish run in database
        if let Some(ref db) = self.db {
            db.finish_run(&run_id, passed, failed)?;
        }

        // Broadcast run completed
        if let Some(ref tx) = self.ws_tx {
            let _ = tx.send(WsMessage::RunCompleted {
                run_id: run_id.clone(),
                passed,
                failed,
            });
        }

        // Print summary
        self.print_summary(&results, cached);

        Ok(results)
    }

    fn execute_single_task(
        &self,
        name: &str,
        run_id: &str,
        cache_manager: Option<&CacheManager>,
        background_processes: &mut Vec<BackgroundProcess>,
    ) -> Result<Option<TaskResult>> {
        let task_config = self
            .config
            .get_task(name)
            .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", name))?;

        // Check cache
        if let Some(cm) = cache_manager {
            if let Some(cached) = cm.check(task_config, name, self.base_dir)? {
                println!(
                    "{} {} {} (cached, {}ms)",
                    "⏭".cyan(),
                    name.cyan(),
                    "skipped".green(),
                    cached.duration_ms.unwrap_or(0)
                );
                return Ok(None); // Cached, no result to add
            }
        }

        // Broadcast task started
        if let Some(ref tx) = self.ws_tx {
            let _ = tx.send(WsMessage::TaskStarted {
                run_id: run_id.to_string(),
                task_name: name.to_string(),
            });
        }

        if task_config.background {
            // Start background process
            match start_background_task(name, task_config, self.base_dir) {
                Ok(process) => {
                    background_processes.push(process);
                    return Ok(None);
                }
                Err(e) => {
                    stop_background_processes(background_processes);
                    return Err(e);
                }
            }
        }

        // Execute foreground task with retry if configured
        let started_at = Utc::now();
        let retry_config = RetryConfig::from(task_config);

        let result = if retry_config.max_retries > 0 {
            execute_with_retry(name, task_config, self.base_dir, &retry_config)?
        } else {
            execute_task(name, task_config, self.base_dir)?
        };

        let success = result.success;

        // Store in cache
        if let Some(cm) = cache_manager {
            let status = if success { "passed" } else { "failed" };
            cm.store(task_config, name, self.base_dir, status, result.duration_ms as i64)?;
        }

        // Store in database
        if let Some(ref db) = self.db {
            let task_result_id = Uuid::new_v4().to_string();
            let db_result = DbTaskResult {
                id: task_result_id.clone(),
                run_id: run_id.to_string(),
                task_name: name.to_string(),
                category: result.category.clone(),
                status: if success { "passed".to_string() } else { "failed".to_string() },
                duration_ms: result.duration_ms as i64,
                started_at,
                output: None,
            };
            db.insert_task_result(&db_result)?;

            // Parse JUnit results if configured
            if let Some(ref results_path) = task_config.results {
                let full_path = self.base_dir.join(results_path);
                if full_path.exists() {
                    if let Ok(suites) = junit::parse_junit_xml(&full_path) {
                        let test_cases = junit::junit_to_test_cases(&suites, &task_result_id);
                        db.insert_test_cases(&task_result_id, &test_cases)?;

                        // Track test history for flaky detection
                        for case in &test_cases {
                            db.record_test_history(
                                &case.name,
                                case.classname.as_deref(),
                                name,
                                &case.status,
                                case.duration_ms,
                                run_id,
                            )?;
                        }
                    }
                }
            }

            // Broadcast task completed
            if let Some(ref tx) = self.ws_tx {
                let _ = tx.send(WsMessage::TaskCompleted {
                    run_id: run_id.to_string(),
                    task: db_result,
                });
            }
        }

        Ok(Some(result))
    }

    /// Execute multiple tasks in parallel using tokio
    async fn execute_parallel_tasks(
        &self,
        task_names: &[String],
        run_id: &str,
        semaphore: Arc<Semaphore>,
    ) -> Result<Vec<TaskResult>> {
        use futures::future::join_all;

        let mut handles = Vec::new();

        for name in task_names {
            let task_config = match self.config.get_task(name) {
                Some(t) => t.clone(),
                None => {
                    eprintln!("{} Task '{}' not found", "✗".red(), name);
                    continue;
                }
            };

            // Skip background tasks in parallel execution
            if task_config.background {
                continue;
            }

            let sem = semaphore.clone();
            let task_name = name.clone();
            let base_dir = self.base_dir.to_path_buf();
            let run_id_clone = run_id.to_string();
            let ws_tx = self.ws_tx.clone();

            handles.push(tokio::spawn(async move {
                // Acquire semaphore permit to limit concurrency
                let _permit = sem.acquire().await.expect("Semaphore closed");

                // Broadcast task started
                if let Some(ref tx) = ws_tx {
                    let _ = tx.send(WsMessage::TaskStarted {
                        run_id: run_id_clone.clone(),
                        task_name: task_name.clone(),
                    });
                }

                // Execute the task
                let result = execute_task_async(task_name, task_config, base_dir).await;

                result
            }));
        }

        // Wait for all tasks to complete
        let results: Vec<Result<TaskResult>> = join_all(handles)
            .await
            .into_iter()
            .map(|r| match r {
                Ok(Ok(result)) => Ok(result),
                Ok(Err(e)) => Err(e),
                Err(e) => Err(anyhow::anyhow!("Task join error: {}", e)),
            })
            .collect();

        // Collect successful results and report errors
        let mut successful_results = Vec::new();
        for result in results {
            match result {
                Ok(r) => successful_results.push(r),
                Err(e) => eprintln!("{} Task error: {}", "✗".red(), e),
            }
        }

        Ok(successful_results)
    }

    fn print_summary(&self, results: &[TaskResult], cached: i32) {
        let total = results.len();
        if total == 0 && cached == 0 {
            println!(
                "{} Only background tasks were run",
                "✓".green().bold()
            );
            return;
        }

        let passed = results.iter().filter(|r| r.success).count();
        let failed = total - passed;
        let total_time: u128 = results.iter().map(|r| r.duration_ms).sum();

        println!("{}", "─".repeat(50).dimmed());

        if failed == 0 {
            if cached > 0 {
                println!(
                    "{} {} task(s) completed, {} cached ({}ms)",
                    "✓".green().bold(),
                    total,
                    cached.to_string().cyan(),
                    total_time
                );
            } else {
                println!(
                    "{} All {} task(s) completed successfully ({}ms)",
                    "✓".green().bold(),
                    total,
                    total_time
                );
            }
        } else {
            println!(
                "{} {}/{} task(s) failed ({}ms)",
                "✗".red().bold(),
                failed,
                total,
                total_time
            );

            println!("\n{}", "Failed tasks:".red());
            for result in results.iter().filter(|r| !r.success) {
                println!("  {} {}", "•".red(), result.name);
            }
        }
    }
}

fn stop_background_processes(processes: &mut Vec<BackgroundProcess>) {
    for process in processes.iter_mut().rev() {
        process.kill().ok();
    }
    processes.clear();
}
