mod config;
mod db;
mod graph;
mod junit;
mod report;
mod runner;
mod server;
mod task;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::{Path, PathBuf};

use config::Config;
use db::Database;
use runner::Runner;
use watcher::TaskWatcher;

const CONFIG_FILE: &str = "runx.toml";
const DEFAULT_REPORT_PATH: &str = "runx-report.html";
const DEFAULT_DB_NAME: &str = ".runx.db";

#[derive(Parser)]
#[command(name = "runx")]
#[command(about = "Universal CLI for task orchestration with live dashboard")]
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

        /// Generate HTML report (static, without live updates)
        #[arg(long)]
        report: bool,

        /// Output path for HTML report
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

    /// Start the live dashboard server
    Serve {
        /// Port to run the dashboard on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },
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

    let db_path = base_dir.join(DEFAULT_DB_NAME);

    // For list command without config, show helpful error
    let config = Config::load(&config_path)
        .with_context(|| format!("Could not load {}", config_path.display()))?;

    match cli.command {
        Commands::Run { task, report, report_path } => {
            cmd_run(&config, &base_dir, &db_path, task, report, &report_path)
        }
        Commands::Watch { task } => cmd_watch(&config, &base_dir, &db_path, task),
        Commands::List => cmd_list(&config),
        Commands::Serve { port } => cmd_serve(port, db_path),
    }
}

fn cmd_run(
    config: &Config,
    base_dir: &Path,
    db_path: &Path,
    task: Option<String>,
    generate_report: bool,
    report_path: &Path,
) -> Result<()> {
    let db = Database::open(db_path)?;
    let runner = Runner::new(config, base_dir, Some(db), None);

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

fn cmd_watch(config: &Config, base_dir: &Path, db_path: &Path, task: Option<String>) -> Result<()> {
    // Validate task exists if specified
    if let Some(ref name) = task {
        if config.get_task(name).is_none() {
            anyhow::bail!("Task '{}' not found. Use 'runx list' to see available tasks.", name);
        }
    }

    let db = Database::open(db_path)?;
    let watcher = TaskWatcher::new(config, base_dir, task, Some(db));
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

        if let Some(ref cat) = task.category {
            println!("    {} {}", "category:".dimmed(), cat.magenta());
        }

        if task.background {
            println!("    {} {}", "background:".dimmed(), "true".yellow());
        }

        if let Some(ref results) = task.results {
            println!("    {} {}", "results:".dimmed(), results);
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

#[tokio::main]
async fn cmd_serve(port: u16, db_path: PathBuf) -> Result<()> {
    // Initialize database if it doesn't exist
    let _ = Database::open(&db_path)?;

    println!("{}", "━".repeat(50).dimmed());
    println!("  {} {}", "Runx Live Dashboard".bold().cyan(), "v0.2.0".dimmed());
    println!("{}", "━".repeat(50).dimmed());

    server::start_server(port, db_path).await
}
