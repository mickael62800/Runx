use anyhow::Result;
use colored::Colorize;
use std::path::Path;

use crate::config::Config;
use crate::graph::topological_sort;
use crate::task::{execute_task, start_background_task, BackgroundProcess, TaskResult};

pub struct Runner<'a> {
    config: &'a Config,
    base_dir: &'a Path,
}

impl<'a> Runner<'a> {
    pub fn new(config: &'a Config, base_dir: &'a Path) -> Self {
        Self { config, base_dir }
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

        println!(
            "\n{} {} task(s) to run: {}\n",
            "→".blue(),
            task_names.len(),
            task_names.join(", ").dimmed()
        );

        for name in task_names {
            let task = self
                .config
                .get_task(name)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found", name))?;

            if task.background {
                // Start background process
                match start_background_task(name, task, self.base_dir) {
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
                let result = execute_task(name, task, self.base_dir)?;
                let success = result.success;
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
