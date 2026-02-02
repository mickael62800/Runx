use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use std::path::Path;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::config::Config;
use crate::db::{Database, TaskResult as DbTaskResult};
use crate::graph::topological_sort;
use crate::junit;
use crate::server::WsMessage;
use crate::task::{execute_task, start_background_task, BackgroundProcess, TaskResult};

pub struct Runner<'a> {
    config: &'a Config,
    base_dir: &'a Path,
    db: Option<Database>,
    ws_tx: Option<broadcast::Sender<WsMessage>>,
}

impl<'a> Runner<'a> {
    pub fn new(
        config: &'a Config,
        base_dir: &'a Path,
        db: Option<Database>,
        ws_tx: Option<broadcast::Sender<WsMessage>>,
    ) -> Self {
        Self { config, base_dir, db, ws_tx }
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

        let mut passed = 0;
        let mut failed = 0;

        for name in task_names {
            let task_config = self
                .config
                .get_task(name)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", name))?;

            // Broadcast task started
            if let Some(ref tx) = self.ws_tx {
                let _ = tx.send(WsMessage::TaskStarted {
                    run_id: run_id.clone(),
                    task_name: name.clone(),
                });
            }

            if task_config.background {
                // Start background process
                match start_background_task(name, task_config, self.base_dir) {
                    Ok(process) => {
                        background_processes.push(process);
                    }
                    Err(e) => {
                        // Stop all background processes on error
                        stop_background_processes(&mut background_processes);
                        return Err(e);
                    }
                }
            } else {
                // Execute foreground task
                let started_at = Utc::now();
                let result = execute_task(name, task_config, self.base_dir)?;
                let success = result.success;

                // Store in database
                if let Some(ref db) = self.db {
                    let task_result_id = Uuid::new_v4().to_string();
                    let db_result = DbTaskResult {
                        id: task_result_id.clone(),
                        run_id: run_id.clone(),
                        task_name: name.clone(),
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
                            }
                        }
                    }

                    // Broadcast task completed
                    if let Some(ref tx) = self.ws_tx {
                        let _ = tx.send(WsMessage::TaskCompleted {
                            run_id: run_id.clone(),
                            task: db_result,
                        });
                    }
                }

                if success {
                    passed += 1;
                } else {
                    failed += 1;
                }

                results.push(result);

                if !success {
                    println!(
                        "{} Stopping due to task failure",
                        "!".yellow()
                    );
                    // Stop all background processes on failure
                    stop_background_processes(&mut background_processes);
                    break;
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
        self.print_summary(&results);

        Ok(results)
    }

    fn print_summary(&self, results: &[TaskResult]) {
        let total = results.len();
        if total == 0 {
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
            println!(
                "{} All {} task(s) completed successfully ({}ms)",
                "✓".green().bold(),
                total,
                total_time
            );
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
