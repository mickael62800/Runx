use anyhow::{Context, Result};
use colored::Colorize;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Task;

#[derive(Debug)]
pub struct TaskResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u128,
    pub category: Option<String>,
}

/// Handle to a background process
pub struct BackgroundProcess {
    pub name: String,
    pub child: Child,
}

impl BackgroundProcess {
    pub fn kill(&mut self) -> Result<()> {
        println!(
            "{} {} {}",
            "■".yellow(),
            "Stopping".bold(),
            self.name.cyan()
        );
        self.child.kill().ok();
        self.child.wait().ok();
        Ok(())
    }
}

impl Drop for BackgroundProcess {
    fn drop(&mut self) {
        self.kill().ok();
    }
}

fn create_command(cmd: &str, work_dir: &Path) -> Command {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", cmd]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", cmd]);
        c
    };
    command.current_dir(work_dir);
    command
}

/// Execute a foreground task (waits for completion)
pub fn execute_task(name: &str, task: &Task, base_dir: &Path) -> Result<TaskResult> {
    let start = Instant::now();

    let work_dir = match &task.cwd {
        Some(cwd) => base_dir.join(cwd),
        None => base_dir.to_path_buf(),
    };

    println!(
        "{} {} {}",
        "▶".blue(),
        "Running".bold(),
        name.cyan()
    );
    println!("  {} {}", "cmd:".dimmed(), task.cmd.dimmed());

    let mut command = create_command(&task.cmd, &work_dir);
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let output = command
        .output()
        .with_context(|| format!("Failed to execute task '{}' in '{}'", name, work_dir.display()))?;

    let duration = start.elapsed().as_millis();
    let success = output.status.success();

    if success {
        println!(
            "{} {} {} ({}ms)\n",
            "✓".green(),
            name.green(),
            "completed".green(),
            duration
        );
    } else {
        println!(
            "{} {} {} ({}ms)\n",
            "✗".red(),
            name.red(),
            "failed".red(),
            duration
        );
    }

    Ok(TaskResult {
        name: name.to_string(),
        success,
        duration_ms: duration,
        category: task.category.clone(),
    })
}

/// Start a background task (returns immediately or waits for ready_when)
pub fn start_background_task(
    name: &str,
    task: &Task,
    base_dir: &Path,
) -> Result<BackgroundProcess> {
    let work_dir = match &task.cwd {
        Some(cwd) => base_dir.join(cwd),
        None => base_dir.to_path_buf(),
    };

    println!(
        "{} {} {} {}",
        "▶".blue(),
        "Starting".bold(),
        name.cyan(),
        "(background)".dimmed()
    );
    println!("  {} {}", "cmd:".dimmed(), task.cmd.dimmed());

    let mut command = create_command(&task.cmd, &work_dir);

    // If we need to wait for ready, capture stdout
    if task.ready_when.is_some() {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
    } else {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }

    let mut child = command
        .spawn()
        .with_context(|| format!("Failed to start background task '{}' in '{}'", name, work_dir.display()))?;

    // Wait for ready_when pattern if specified
    if let Some(ref pattern) = task.ready_when {
        let timeout = Duration::from_secs(task.ready_timeout);
        let start = Instant::now();

        println!(
            "  {} waiting for: \"{}\"",
            "⏳".dimmed(),
            pattern.dimmed()
        );

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let (tx, rx) = mpsc::channel();
        let pattern_clone = pattern.clone();

        // Thread to read stdout
        if let Some(stdout) = stdout {
            let tx = tx.clone();
            let pattern = pattern_clone.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        println!("  {}", line.dimmed());
                        if line.contains(&pattern) {
                            tx.send(true).ok();
                            return;
                        }
                    }
                }
                tx.send(false).ok();
            });
        }

        // Thread to read stderr
        if let Some(stderr) = stderr {
            let tx = tx.clone();
            let pattern = pattern_clone;
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        eprintln!("  {}", line.dimmed());
                        if line.contains(&pattern) {
                            tx.send(true).ok();
                            return;
                        }
                    }
                }
            });
        }

        // Wait for ready signal or timeout
        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(true) => {
                    println!(
                        "{} {} {} ({}ms)\n",
                        "✓".green(),
                        name.green(),
                        "ready".green(),
                        start.elapsed().as_millis()
                    );
                    break;
                }
                Ok(false) => {
                    anyhow::bail!("Background task '{}' exited before becoming ready", name);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if start.elapsed() > timeout {
                        child.kill().ok();
                        anyhow::bail!(
                            "Background task '{}' timed out waiting for ready ({}s)",
                            name,
                            task.ready_timeout
                        );
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("Background task '{}' failed to start", name);
                }
            }

            // Check if process is still running
            if let Ok(Some(status)) = child.try_wait() {
                if !status.success() {
                    anyhow::bail!("Background task '{}' exited with error", name);
                }
            }
        }
    } else {
        // No ready_when, just wait a bit and check it's still running
        thread::sleep(Duration::from_millis(500));
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                anyhow::bail!("Background task '{}' exited immediately with error", name);
            }
        }
        println!(
            "{} {} {}\n",
            "✓".green(),
            name.green(),
            "started".green()
        );
    }

    Ok(BackgroundProcess {
        name: name.to_string(),
        child,
    })
}

/// Async version of execute_task for parallel execution
pub async fn execute_task_async(name: String, task: Task, base_dir: PathBuf) -> Result<TaskResult> {
    // Spawn blocking because we use std::process
    let result = tokio::task::spawn_blocking(move || {
        execute_task_inner(&name, &task, &base_dir)
    })
    .await
    .map_err(|e| anyhow::anyhow!("Task join error: {}", e))??;

    Ok(result)
}

/// Inner task execution (used by both sync and async versions)
fn execute_task_inner(name: &str, task: &Task, base_dir: &Path) -> Result<TaskResult> {
    let start = Instant::now();

    let work_dir = match &task.cwd {
        Some(cwd) => base_dir.join(cwd),
        None => base_dir.to_path_buf(),
    };

    println!(
        "{} {} {}",
        "▶".blue(),
        "Running".bold(),
        name.cyan()
    );
    println!("  {} {}", "cmd:".dimmed(), task.cmd.dimmed());

    let mut command = create_command(&task.cmd, &work_dir);
    command.stdout(Stdio::inherit()).stderr(Stdio::inherit());

    let output = command
        .output()
        .with_context(|| format!("Failed to execute task '{}' in '{}'", name, work_dir.display()))?;

    let duration = start.elapsed().as_millis();
    let success = output.status.success();

    if success {
        println!(
            "{} {} {} ({}ms)\n",
            "✓".green(),
            name.green(),
            "completed".green(),
            duration
        );
    } else {
        println!(
            "{} {} {} ({}ms)\n",
            "✗".red(),
            name.red(),
            "failed".red(),
            duration
        );
    }

    Ok(TaskResult {
        name: name.to_string(),
        success,
        duration_ms: duration,
        category: task.category.clone(),
    })
}
