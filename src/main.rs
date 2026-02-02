mod config;
mod graph;
mod report;
mod runner;
mod task;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use config::Config;
use runner::Runner;
use watcher::TaskWatcher;

const CONFIG_FILE: &str = "runx.toml";
const DEFAULT_REPORT_PATH: &str = "runx-report.html";

#[derive(Parser)]
#[command(name = "runx")]
#[command(about = "Universal CLI for task orchestration with intelligent watch")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to config file (default: runx.toml)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run one or all tasks
    Run {
        /// Task name to run (runs all if not specified)
        task: Option<String>,

        /// Generate HTML dashboard report
        #[arg(long)]
        report: bool,

        /// Output path for HTML report (default: runx-report.html)
        #[arg(long, default_value = DEFAULT_REPORT_PATH)]
        report_path: PathBuf,
    },

    /// Watch files and re-run tasks on changes
    Watch {
        /// Task to watch (watches all if not specified)
        task: Option<String>,
    },

    /// List all available tasks
    List,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    let config_path = cli.config.unwrap_or_else(|| PathBuf::from(CONFIG_FILE));

    // Canonicalize config path to get absolute path, then get parent
    let config_path = std::fs::canonicalize(&config_path)
        .with_context(|| format!("Could not find config file: {}", config_path.display()))?;

    let base_dir = config_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // For list command without config, show helpful error
    let config = Config::load(&config_path)
        .with_context(|| format!("Could not load {}", config_path.display()))?;

    match cli.command {
        Commands::Run { task, report, report_path } => {
            cmd_run(&config, &base_dir, task, report, &report_path)
        }
        Commands::Watch { task } => cmd_watch(&config, &base_dir, task),
        Commands::List => cmd_list(&config),
    }
}

fn cmd_run(
    config: &Config,
    base_dir: &PathBuf,
    task: Option<String>,
    generate_report: bool,
    report_path: &PathBuf,
) -> Result<()> {
    let runner = Runner::new(config, base_dir);

    let results = match task {
        Some(name) => {
            if config.get_task(&name).is_none() {
                anyhow::bail!("Task '{}' not found. Use 'runx list' to see available tasks.", name);
            }
            runner.run_task(&name)?
        }
        None => runner.run_all()?,
    };

    // Generate HTML report if requested
    if generate_report {
        let report_full_path = base_dir.join(report_path);
        report::generate_report(&config.project.name, &results, &report_full_path)?;
        println!(
            "\n{} Report generated: {}",
            "📊".cyan(),
            report_full_path.display().to_string().green()
        );
    }

    // Exit with error code if any task failed
    let all_passed = results.iter().all(|r| r.success);
    if !all_passed {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_watch(config: &Config, base_dir: &PathBuf, task: Option<String>) -> Result<()> {
    // Validate task exists if specified
    if let Some(ref name) = task {
        if config.get_task(name).is_none() {
            anyhow::bail!("Task '{}' not found. Use 'runx list' to see available tasks.", name);
        }
    }

    let watcher = TaskWatcher::new(config, base_dir, task);
    watcher.start()
}

fn cmd_list(config: &Config) -> Result<()> {
    println!("\n{} {}\n", "📦".cyan(), config.project.name.bold());

    if config.tasks.is_empty() {
        println!("  {}", "No tasks defined".dimmed());
        return Ok(());
    }

    println!("{}", "Tasks:".bold());

    let mut task_names: Vec<_> = config.tasks.keys().collect();
    task_names.sort();

    for name in task_names {
        let task = &config.tasks[name];
        println!("  {} {}", "•".green(), name.cyan());
        println!("    {} {}", "cmd:".dimmed(), task.cmd);

        if let Some(ref cwd) = task.cwd {
            println!("    {} {}", "cwd:".dimmed(), cwd);
        }

        if task.background {
            println!("    {} {}", "background:".dimmed(), "true".yellow());
        }

        if !task.depends_on.is_empty() {
            println!(
                "    {} {}",
                "depends_on:".dimmed(),
                task.depends_on.join(", ")
            );
        }

        if !task.watch.is_empty() {
            println!(
                "    {} {}",
                "watch:".dimmed(),
                task.watch.join(", ")
            );
        }

        println!();
    }

    Ok(())
}
