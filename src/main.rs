//! Runx - Rust Test Explorer
//!
//! A CLI tool for discovering, running, and managing Rust tests with:
//! - Interactive TUI with tree view
//! - Automatic test discovery
//! - Watch mode with affected test detection
//! - Filtering by name and status
//! - Web dashboard with real-time updates
//! - HTML report generation

mod affected;
mod artifacts;
mod db;
mod discovery;
mod report;
mod server;
mod task;
mod test_model;
mod test_runner;
mod tui;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::{Path, PathBuf};
use chrono::Utc;
use uuid::Uuid;

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

        /// Generate HTML report after run
        #[arg(long)]
        report: bool,

        /// Retry failed tests N times
        #[arg(long, value_name = "N")]
        retry: Option<u32>,
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

    /// Launch web dashboard with real-time updates
    Dashboard {
        /// Port to run the server on
        #[arg(short, long, default_value = "3000")]
        port: u16,
        /// Enable watch mode for real-time updates
        #[arg(short, long)]
        watch: bool,
    },

    /// Generate HTML report from last run
    Report {
        /// Output file path
        #[arg(short, long, default_value = "runx-report.html")]
        output: PathBuf,

        /// Specific run ID to report on
        #[arg(long)]
        run: Option<String>,
    },

    /// Show run history
    History {
        /// Number of runs to show
        #[arg(short, long, default_value = "20")]
        limit: i32,

        /// Clear all history
        #[arg(long)]
        clear: bool,
    },

    /// Show statistics
    Stats {
        /// Show flaky tests
        #[arg(long)]
        flaky: bool,
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
        Some(Commands::Run { filter, failed, verbose, report, retry }) => {
            cmd_run(&project_dir, &db_path, filter, failed, verbose, report, retry)
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
        Some(Commands::Dashboard { port, watch }) => {
            cmd_dashboard(&project_dir, &db_path, port, watch)
        }
        Some(Commands::Report { output, run }) => {
            cmd_report(&project_dir, &db_path, &output, run)
        }
        Some(Commands::History { limit, clear }) => {
            cmd_history(&db_path, limit, clear)
        }
        Some(Commands::Stats { flaky }) => {
            cmd_stats(&db_path, flaky)
        }
    }
}

fn cmd_run(
    project_dir: &Path,
    db_path: &Path,
    filter: Option<String>,
    failed: bool,
    verbose: bool,
    generate_report: bool,
    retry: Option<u32>,
) -> Result<()> {
    let project_name = get_project_name(project_dir)?;
    let db = Database::open(db_path).ok();

    println!("\n{} {} {}\n", "🧪".cyan(), "Running tests for".bold(), project_name.cyan());

    let runner = TestRunner::new(project_dir);

    // Create run in database
    let run_id = Uuid::new_v4().to_string();
    if let Some(ref db) = db {
        db.create_run(&run_id, 0)?;
    }

    let mut result = if failed {
        // Load failed tests from last run
        if let Some(ref db) = db {
            let failed_tests = db.get_failed_tests_from_last_run()?;
            if failed_tests.is_empty() {
                println!("{}", "No failed tests from last run".dimmed());
                return Ok(());
            }
            println!("{} {} failed test(s) to retry\n", "→".blue(), failed_tests.len());
            runner.run_specific(&failed_tests)?
        } else {
            println!("{}", "No database available, running all tests".dimmed());
            runner.run_all()?
        }
    } else if let Some(ref f) = filter {
        println!("{} {}\n", "Filter:".dimmed(), f.cyan());
        runner.run_filtered(f)?
    } else {
        runner.run_all()?
    };

    // Retry failed tests if requested
    if let Some(max_retries) = retry {
        let mut retries = 0;
        while result.failed > 0 && retries < max_retries {
            retries += 1;
            let failed_names: Vec<String> = result.test_results
                .iter()
                .filter(|t| t.status == TestStatus::Failed)
                .map(|t| t.name.clone())
                .collect();

            println!("\n{} Retry {}/{} for {} failed test(s)...\n",
                "🔄".yellow(), retries, max_retries, failed_names.len());

            let retry_result = runner.run_specific(&failed_names)?;

            // Update results
            for retry_test in retry_result.test_results {
                if let Some(orig) = result.test_results.iter_mut().find(|t| t.name == retry_test.name) {
                    if retry_test.status == TestStatus::Passed {
                        orig.status = TestStatus::Passed;
                        result.passed += 1;
                        result.failed -= 1;
                    }
                }
            }
        }
    }

    // Save results to database
    if let Some(ref db) = db {
        let started_at = Utc::now();
        for test in &result.test_results {
            let task_result = db::TaskResult {
                id: Uuid::new_v4().to_string(),
                run_id: run_id.clone(),
                task_name: test.name.clone(),
                category: Some("test".to_string()),
                status: match test.status {
                    TestStatus::Passed => "passed".to_string(),
                    TestStatus::Failed => "failed".to_string(),
                    TestStatus::Ignored => "skipped".to_string(),
                    _ => "pending".to_string(),
                },
                duration_ms: test.duration_ms.unwrap_or(0) as i64,
                started_at,
                output: if test.output.is_empty() { None } else { Some(test.output.join("\n")) },
            };
            db.insert_task_result(&task_result)?;
        }
        db.finish_run(&run_id, result.passed as i32, result.failed as i32)?;
    }

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
    } else {
        println!(
            "\n{} {} passed, {} ignored\n",
            "✓".green(),
            result.passed.to_string().green(),
            result.ignored.to_string().dimmed()
        );
    }

    // Generate report if requested
    if generate_report {
        let report_path = project_dir.join("runx-report.html");
        println!("{} Generating report...", "📊".cyan());

        if let Some(ref db) = db {
            if let Some(summary) = db.get_run_summary(&run_id)? {
                let task_results: Vec<task::TaskResult> = summary.tasks.iter().map(|t| {
                    task::TaskResult {
                        name: t.task_name.clone(),
                        success: t.status == "passed",
                        duration_ms: t.duration_ms as u128,
                        category: t.category.clone(),
                    }
                }).collect();

                report::generate_report(&project_name, &task_results, &report_path)?;
                println!("{} Report saved to {}\n", "✓".green(), report_path.display());
            }
        }
    }

    if result.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_list(project_dir: &Path, filter: Option<String>, full: bool) -> Result<()> {
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

fn cmd_watch(project_dir: &Path, db_path: &Path, filter: Option<String>) -> Result<()> {
    let db = Database::open(db_path).ok();
    let mut watcher = TestWatcher::new(project_dir, filter, db);
    watcher.start()
}

fn cmd_discover(project_dir: &Path) -> Result<()> {
    let project_name = get_project_name(project_dir)?;

    println!("\n{} Discovering tests in {}...\n", "🔍".cyan(), project_name.cyan());

    let tree = discover_all_tests(project_dir)?;

    let stats = test_model::TestStats::from_tree(&tree);

    println!("{} {} test(s) discovered\n", "✓".green(), stats.total);

    // Show summary by module
    let mut modules: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for test in tree.all_tests() {
        let module = test.module_path.first().cloned()
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

fn cmd_tui(project_dir: &Path, db_path: &Path) -> Result<()> {
    let db = Database::open(db_path).ok();
    tui::run_tui(project_dir, db)
}

fn cmd_dashboard(project_dir: &Path, db_path: &Path, port: u16, watch: bool) -> Result<()> {
    println!("\n{} Starting Runx Dashboard...\n", "🚀".cyan());

    if watch {
        println!("{} Watch mode enabled - tests will run on file changes\n", "👀".cyan());
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        server::start_server(port, db_path.to_path_buf(), project_dir.to_path_buf(), watch).await
    })?;

    Ok(())
}

fn cmd_report(
    project_dir: &Path,
    db_path: &Path,
    output: &Path,
    run_id: Option<String>,
) -> Result<()> {
    let db = Database::open(db_path)
        .context("No database found. Run some tests first with 'runx run'")?;

    let project_name = get_project_name(project_dir)?;

    // Get the run to report on
    let run_id = if let Some(id) = run_id {
        id
    } else {
        // Get last run
        let runs = db.get_recent_runs(1)?;
        if runs.is_empty() {
            anyhow::bail!("No runs found. Run some tests first with 'runx run'");
        }
        runs[0].id.clone()
    };

    println!("{} Generating report for run {}...\n", "📊".cyan(), &run_id[..8]);

    let summary = db.get_run_summary(&run_id)?
        .context("Run not found")?;

    let task_results: Vec<task::TaskResult> = summary.tasks.iter().map(|t| {
        task::TaskResult {
            name: t.task_name.clone(),
            success: t.status == "passed",
            duration_ms: t.duration_ms as u128,
            category: t.category.clone(),
        }
    }).collect();

    report::generate_report(&project_name, &task_results, output)?;

    println!("{} Report saved to {}\n", "✓".green(), output.display());
    Ok(())
}

fn cmd_history(db_path: &Path, limit: i32, clear: bool) -> Result<()> {
    let db = Database::open(db_path)
        .context("No database found. Run some tests first with 'runx run'")?;

    if clear {
        println!("{} Clearing all history...", "🗑".yellow());
        let deleted = db.clear_all_history()?;
        println!("{} Deleted {} records\n", "✓".green(), deleted);
        return Ok(());
    }

    let runs = db.get_recent_runs(limit)?;

    if runs.is_empty() {
        println!("{}", "No runs found. Run some tests first with 'runx run'".dimmed());
        return Ok(());
    }

    println!("\n{} Run History (last {})\n", "📜".cyan(), limit);
    println!("{}", "─".repeat(70).dimmed());

    for run in &runs {
        let status_icon = if run.status == "passed" { "✓".green() } else { "✗".red() };
        let duration = run.finished_at
            .map(|f| (f - run.started_at).num_milliseconds())
            .unwrap_or(0);

        println!(
            "{} {} │ {} passed, {} failed │ {}ms │ {}",
            status_icon,
            &run.id[..8],
            run.passed.to_string().green(),
            run.failed.to_string().red(),
            duration,
            run.started_at.format("%Y-%m-%d %H:%M:%S")
        );
    }

    println!("{}", "─".repeat(70).dimmed());
    println!();

    Ok(())
}

fn cmd_stats(db_path: &Path, show_flaky: bool) -> Result<()> {
    let db = Database::open(db_path)
        .context("No database found. Run some tests first with 'runx run'")?;

    let stats = db.get_dashboard_stats()?;

    println!("\n{} Runx Statistics\n", "📊".cyan());
    println!("{}", "─".repeat(50).dimmed());

    println!("  {} Total runs:        {}", "•".blue(), stats.total_runs);
    println!("  {} Tasks executed:    {}", "•".blue(), stats.total_tasks_executed);
    println!("  {} Overall pass rate: {}%", "•".blue(),
        format!("{:.1}", stats.overall_pass_rate).green());
    println!("  {} Avg duration:      {}ms", "•".blue(), stats.avg_duration_ms);

    println!("{}", "─".repeat(50).dimmed());

    if show_flaky {
        println!("\n{} Flaky Tests\n", "⚠".yellow());

        let flaky_tests = db.get_flaky_tests(10)?;

        if flaky_tests.is_empty() {
            println!("  {}", "No flaky tests detected".dimmed());
        } else {
            for test in &flaky_tests {
                let flaky_pct = if test.total_runs > 0 {
                    (test.fail_count as f64 / test.total_runs as f64) * 100.0
                } else {
                    0.0
                };
                println!(
                    "  {} {} ({:.0}% failure rate, {} runs)",
                    "•".yellow(),
                    test.test_name,
                    flaky_pct,
                    test.total_runs
                );
            }
        }

        println!();
    }

    // Pass rate history
    if !stats.pass_rate_history.is_empty() {
        println!("\n{} Pass Rate Trend (7 days)\n", "📈".cyan());

        for point in &stats.pass_rate_history {
            let bar_len = (point.pass_rate / 5.0) as usize;
            let bar = "█".repeat(bar_len);
            println!("  {} │ {:>5.1}% {} ({} runs)",
                point.date,
                point.pass_rate,
                bar.green(),
                point.run_count
            );
        }

        println!();
    }

    Ok(())
}
