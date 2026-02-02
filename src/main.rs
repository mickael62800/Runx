//! Runx - Rust Test Explorer
//!
//! A CLI tool for discovering, running, and managing Rust tests with:
//! - Interactive TUI with tree view
//! - Automatic test discovery
//! - Watch mode with affected test detection
//! - Filtering by name and status

mod affected;
mod db;
mod discovery;
mod test_model;
mod test_runner;
mod tui;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

use db::Database;
use discovery::{discover_all_tests, get_project_name, is_rust_project};
use test_model::TestStatus;
use test_runner::TestRunner;
use watcher::TestWatcher;

const DEFAULT_DB_NAME: &str = ".runx.db";

#[derive(Parser)]
#[command(name = "runx")]
#[command(about = "Rust Test Explorer - Discover, run, and manage tests")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Project directory (default: current directory)
    #[arg(short, long, global = true)]
    dir: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run tests (default if no subcommand)
    Run {
        /// Filter pattern to match test names
        filter: Option<String>,

        /// Run only failed tests from last run
        #[arg(long)]
        failed: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// List all discovered tests
    List {
        /// Filter pattern to match test names
        filter: Option<String>,

        /// Show full test paths
        #[arg(short, long)]
        full: bool,
    },

    /// Watch for changes and re-run affected tests
    Watch {
        /// Filter pattern to match test names
        filter: Option<String>,
    },

    /// Discover tests without running them
    Discover,

    /// Interactive TUI mode
    Tui,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Determine project directory
    let project_dir = cli.dir
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    let project_dir = std::fs::canonicalize(&project_dir)
        .with_context(|| format!("Could not access directory: {}", project_dir.display()))?;

    // Check if it's a Rust project
    if !is_rust_project(&project_dir) {
        anyhow::bail!(
            "No Cargo.toml found in {}. Run this command from a Rust project directory.",
            project_dir.display()
        );
    }

    let db_path = project_dir.join(DEFAULT_DB_NAME);

    match cli.command {
        None => {
            // Default: run TUI
            cmd_tui(&project_dir, &db_path)
        }
        Some(Commands::Run { filter, failed, verbose }) => {
            cmd_run(&project_dir, filter, failed, verbose)
        }
        Some(Commands::List { filter, full }) => {
            cmd_list(&project_dir, filter, full)
        }
        Some(Commands::Watch { filter }) => {
            cmd_watch(&project_dir, &db_path, filter)
        }
        Some(Commands::Discover) => {
            cmd_discover(&project_dir)
        }
        Some(Commands::Tui) => {
            cmd_tui(&project_dir, &db_path)
        }
    }
}

fn cmd_run(
    project_dir: &PathBuf,
    filter: Option<String>,
    failed: bool,
    verbose: bool,
) -> Result<()> {
    let project_name = get_project_name(project_dir)?;

    println!("\n{} {} {}\n", "🧪".cyan(), "Running tests for".bold(), project_name.cyan());

    let runner = TestRunner::new(project_dir);

    let result = if failed {
        // TODO: Load failed tests from last run
        println!("{}", "Running all tests (--failed not yet implemented with persistence)".dimmed());
        runner.run_all()?
    } else if let Some(ref f) = filter {
        println!("{} {}\n", "Filter:".dimmed(), f.cyan());
        runner.run_filtered(f)?
    } else {
        runner.run_all()?
    };

    // Print results
    println!("\n{}", "─".repeat(50).dimmed());

    if result.failed > 0 {
        println!(
            "\n{} {} passed, {} failed, {} ignored\n",
            "Results:".bold(),
            result.passed.to_string().green(),
            result.failed.to_string().red(),
            result.ignored.to_string().dimmed()
        );

        // Show failed tests
        if verbose || result.failed <= 10 {
            println!("{}", "Failed tests:".red().bold());
            for test in &result.test_results {
                if test.status == TestStatus::Failed {
                    println!("  {} {}", "✗".red(), test.name);
                    if verbose && !test.output.is_empty() {
                        for line in &test.output {
                            println!("    {}", line.dimmed());
                        }
                    }
                }
            }
        }

        std::process::exit(1);
    } else {
        println!(
            "\n{} {} passed, {} ignored\n",
            "✓".green(),
            result.passed.to_string().green(),
            result.ignored.to_string().dimmed()
        );
    }

    Ok(())
}

fn cmd_list(project_dir: &PathBuf, filter: Option<String>, full: bool) -> Result<()> {
    let project_name = get_project_name(project_dir)?;

    println!("\n{} {}\n", "📦".cyan(), project_name.bold());

    let tree = discover_all_tests(project_dir)?;
    let all_tests = tree.all_tests();

    let tests: Vec<_> = if let Some(ref f) = filter {
        let f_lower = f.to_lowercase();
        all_tests.into_iter()
            .filter(|t| t.full_name.to_lowercase().contains(&f_lower))
            .collect()
    } else {
        all_tests
    };

    if tests.is_empty() {
        if filter.is_some() {
            println!("  {}", "No tests match the filter".dimmed());
        } else {
            println!("  {}", "No tests found".dimmed());
        }
        return Ok(());
    }

    println!("{} {} test(s)\n", "Found".bold(), tests.len());

    // Group by module if not showing full paths
    if full {
        for test in &tests {
            let status = test.status.symbol();
            println!("  {} {}", status, test.full_name);
        }
    } else {
        // Group by first module level
        let mut current_module = String::new();
        for test in &tests {
            let module = test.module_path.first()
                .map(|s| s.as_str())
                .unwrap_or("(root)");

            if module != current_module {
                if !current_module.is_empty() {
                    println!();
                }
                println!("  {} {}", "▸".cyan(), module.bold());
                current_module = module.to_string();
            }

            let status = test.status.symbol();
            println!("    {} {}", status, test.short_name);
        }
    }

    println!();
    Ok(())
}

fn cmd_watch(project_dir: &PathBuf, db_path: &PathBuf, filter: Option<String>) -> Result<()> {
    let db = Database::open(db_path).ok();
    let mut watcher = TestWatcher::new(project_dir, filter, db);
    watcher.start()
}

fn cmd_discover(project_dir: &PathBuf) -> Result<()> {
    let project_name = get_project_name(project_dir)?;

    println!("\n{} Discovering tests in {}...\n", "🔍".cyan(), project_name.cyan());

    let tree = discover_all_tests(project_dir)?;

    let stats = test_model::TestStats::from_tree(&tree);

    println!("{} {} test(s) discovered\n", "✓".green(), stats.total);

    // Show summary by module
    let mut modules: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for test in tree.all_tests() {
        let module = test.module_path.first()
            .map(|s| s.clone())
            .unwrap_or_else(|| "(root)".to_string());
        *modules.entry(module).or_insert(0) += 1;
    }

    if !modules.is_empty() {
        println!("{}", "Modules:".bold());
        let mut sorted: Vec<_> = modules.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));

        for (module, count) in sorted {
            println!("  {} {} ({})", "▸".cyan(), module, count);
        }
    }

    if stats.ignored > 0 {
        println!("\n{} {} test(s) ignored", "⊘".dimmed(), stats.ignored);
    }

    println!();
    Ok(())
}

fn cmd_tui(project_dir: &PathBuf, db_path: &PathBuf) -> Result<()> {
    let db = Database::open(db_path).ok();
    tui::run_tui(project_dir, db)
}
