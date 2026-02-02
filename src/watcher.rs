use anyhow::Result;
use chrono::Utc;
use colored::Colorize;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::config::Config as RunxConfig;
use crate::db::{Database, TaskResult as DbTaskResult};
use crate::graph::{find_tasks_watching_file, get_dependent_tasks, topological_sort};
use crate::task::execute_task;

const DEBOUNCE_MS: u128 = 300;
const EXCLUDED_DIRS: &[&str] = &["target", "node_modules", "dist", "out", ".git"];

pub struct TaskWatcher<'a> {
    config: &'a RunxConfig,
    base_dir: &'a Path,
    task_filter: Option<String>,
    db: Option<Database>,
}

impl<'a> TaskWatcher<'a> {
    pub fn new(
        config: &'a RunxConfig,
        base_dir: &'a Path,
        task_filter: Option<String>,
        db: Option<Database>,
    ) -> Self {
        Self {
            config,
            base_dir,
            task_filter,
            db,
        }
    }

    pub fn start(&self) -> Result<()> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.send(event);
                }
            },
            Config::default(),
        )?;

        // Watch the base directory recursively
        watcher.watch(self.base_dir, RecursiveMode::Recursive)?;

        println!(
            "\n{} {} {}\n",
            "👀".cyan(),
            "Watching for changes in".bold(),
            self.base_dir.display()
        );

        if let Some(ref task) = self.task_filter {
            println!("   Filtering for task: {}\n", task.cyan());
        }

        println!("{}", "Press Ctrl+C to stop\n".dimmed());

        self.event_loop(rx)?;

        Ok(())
    }

    fn event_loop(&self, rx: Receiver<Event>) -> Result<()> {
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

                    // Convert to relative path
                    p.strip_prefix(self.base_dir)
                        .ok()
                        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
                })
                .collect();

            if changed_files.is_empty() {
                continue;
            }

            // Find tasks to run
            let tasks_to_run = self.find_tasks_to_run(&changed_files);

            if tasks_to_run.is_empty() {
                continue;
            }

            last_run = Instant::now();
            self.run_tasks(&tasks_to_run, &changed_files)?;
        }

        Ok(())
    }

    fn find_tasks_to_run(&self, changed_files: &[String]) -> Vec<String> {
        let mut tasks_to_run: HashSet<String> = HashSet::new();

        for file in changed_files {
            let matching = find_tasks_watching_file(self.config, file);
            for task_name in matching {
                // Apply task filter if specified
                if let Some(ref filter) = self.task_filter {
                    if &task_name != filter {
                        continue;
                    }
                }

                tasks_to_run.insert(task_name.clone());

                // Add dependent tasks
                let dependents = get_dependent_tasks(self.config, &task_name);
                for dep in dependents {
                    if self.task_filter.is_none() || self.task_filter.as_ref() == Some(&dep) {
                        tasks_to_run.insert(dep);
                    }
                }
            }
        }

        tasks_to_run.into_iter().collect()
    }

    fn run_tasks(&self, task_names: &[String], changed_files: &[String]) -> Result<()> {
        println!(
            "\n{} {} {}\n",
            "↻".yellow(),
            "File changed:".bold(),
            changed_files.join(", ").dimmed()
        );

        // Get execution order
        let task_refs: Vec<&str> = task_names.iter().map(|s| s.as_str()).collect();
        let execution_order = match topological_sort(self.config, &task_refs) {
            Ok(order) => order,
            Err(e) => {
                eprintln!("{} Failed to resolve dependencies: {}", "✗".red(), e);
                return Ok(());
            }
        };

        println!(
            "{} Running: {}\n",
            "→".blue(),
            execution_order.join(" → ").cyan()
        );

        // Create run in database
        let run_id = Uuid::new_v4().to_string();
        if let Some(ref db) = self.db {
            db.create_run(&run_id, execution_order.len() as i32)?;
        }

        let mut passed = 0;
        let mut failed = 0;

        // Execute tasks in order
        for name in &execution_order {
            if let Some(task) = self.config.get_task(name) {
                let started_at = Utc::now();
                let result = execute_task(name, task, self.base_dir)?;

                // Store in database
                if let Some(ref db) = self.db {
                    let db_result = DbTaskResult {
                        id: Uuid::new_v4().to_string(),
                        run_id: run_id.clone(),
                        task_name: name.clone(),
                        category: task.category.clone(),
                        status: if result.success { "passed".to_string() } else { "failed".to_string() },
                        duration_ms: result.duration_ms as i64,
                        started_at,
                        output: None,
                    };
                    db.insert_task_result(&db_result)?;
                }

                if result.success {
                    passed += 1;
                } else {
                    failed += 1;
                    println!("{} Stopping due to failure\n", "!".yellow());
                    break;
                }
            }
        }

        // Finish run in database
        if let Some(ref db) = self.db {
            db.finish_run(&run_id, passed, failed)?;
        }

        println!("{}", "Watching for changes...".dimmed());

        Ok(())
    }
}
