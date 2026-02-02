mod ai;
mod config;
mod coverage;
mod db;
mod execution;
mod git;
mod graph;
mod junit;
mod notifications;
mod report;
mod server;
mod task;
mod tui;
mod watcher;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::{Path, PathBuf};

use config::Config;
use db::Database;
use execution::{Runner, RunOptions};
use graph::affected::find_affected_tasks;
use watcher::TaskWatcher;

const CONFIG_FILE: &str = "runx.toml";
const DEFAULT_REPORT_PATH: &str = "runx-report.html";
const DEFAULT_DB_NAME: &str = ".runx.db";

#[derive(Parser)]
#[command(name = "runx")]
#[command(about = "Universal CLI for task orchestration with live dashboard")]
#[command(version = "0.3.0")]
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

        // v0.3.0 - New options
        /// Enable parallel execution
        #[arg(long)]
        parallel: bool,

        /// Number of parallel workers
        #[arg(long, default_value = "4")]
        workers: usize,

        /// Run only affected tasks (based on git changes)
        #[arg(long)]
        affected: bool,

        /// Git reference for affected detection (commit, branch, tag)
        #[arg(long)]
        since: Option<String>,

        /// Base branch for affected detection
        #[arg(long)]
        base: Option<String>,

        /// Ignore cache
        #[arg(long)]
        no_cache: bool,

        /// Include quarantined flaky tests
        #[arg(long)]
        include_quarantined: bool,

        /// Run all workspace packages
        #[arg(long)]
        workspace: bool,

        /// Run specific package (for monorepo)
        #[arg(long)]
        package: Option<String>,

        /// Filter tasks by pattern
        #[arg(long)]
        filter: Option<String>,

        /// Use a specific profile
        #[arg(long)]
        profile: Option<String>,

        /// Stop on first failure
        #[arg(long)]
        fail_fast: bool,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Watch files and re-run tasks on changes
    Watch {
        /// Task to watch (watches all if not specified)
        task: Option<String>,
    },

    /// List all available tasks
    List {
        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Start the live dashboard server
    Serve {
        /// Port to run the dashboard on
        #[arg(short, long, default_value = "3000")]
        port: u16,
    },

    /// Interactive TUI
    Tui,

    /// Cache management
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },

    /// Profile management
    Profiles {
        #[command(subcommand)]
        action: ProfileAction,
    },

    /// AI-powered test annotation
    Annotate {
        #[command(subcommand)]
        action: AnnotateAction,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// Clear all cached results
    Clear,
    /// Show cache statistics
    Show,
}

#[derive(Subcommand)]
enum ProfileAction {
    /// List available profiles
    List,
}

#[derive(Subcommand)]
enum AnnotateAction {
    /// Annotate tests in a file using AI
    File {
        /// Path to the test file
        path: PathBuf,

        /// AI provider (anthropic or openai)
        #[arg(long, default_value = "anthropic")]
        provider: String,

        /// Language for annotations (en, fr, es, de)
        #[arg(long, default_value = "en")]
        language: String,
    },

    /// Annotate all tests in the project
    All {
        /// AI provider (anthropic or openai)
        #[arg(long, default_value = "anthropic")]
        provider: String,

        /// Language for annotations (en, fr, es, de)
        #[arg(long, default_value = "en")]
        language: String,

        /// Glob pattern for test files
        #[arg(long, default_value = "**/*test*.rs")]
        pattern: String,
    },

    /// Show annotations for tests
    Show {
        /// Filter by test type (unit, integration, e2e)
        #[arg(long)]
        test_type: Option<String>,

        /// Filter by tag
        #[arg(long)]
        tag: Option<String>,
    },

    /// Export annotations to JSON
    Export {
        /// Output file path
        #[arg(short, long, default_value = "test-annotations.json")]
        output: PathBuf,
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
        Commands::Run {
            task,
            report,
            report_path,
            parallel,
            workers,
            affected,
            since,
            base,
            no_cache,
            include_quarantined,
            workspace: _,
            package: _,
            filter,
            profile,
            fail_fast,
            verbose,
        } => {
            cmd_run(
                &config,
                &base_dir,
                &db_path,
                task,
                report,
                &report_path,
                RunOptions {
                    parallel,
                    workers,
                    use_cache: !no_cache,
                    fail_fast,
                    include_quarantined,
                    profile,
                    verbose,
                },
                affected,
                since,
                base,
                filter,
            )
        }
        Commands::Watch { task } => cmd_watch(&config, &base_dir, &db_path, task),
        Commands::List { detailed } => cmd_list(&config, detailed),
        Commands::Serve { port } => cmd_serve(port, db_path),
        Commands::Tui => cmd_tui(&config, &base_dir, &db_path),
        Commands::Cache { action } => cmd_cache(&db_path, action),
        Commands::Profiles { action } => cmd_profiles(&config, action),
        Commands::Annotate { action } => cmd_annotate(&config, &base_dir, &db_path, action),
    }
}

fn cmd_run(
    config: &Config,
    base_dir: &Path,
    db_path: &Path,
    task: Option<String>,
    generate_report: bool,
    report_path: &Path,
    options: RunOptions,
    affected: bool,
    since: Option<String>,
    base: Option<String>,
    filter: Option<String>,
) -> Result<()> {
    let db = Database::open(db_path)?;

    // Get profile settings if specified
    let profile = config.get_profile(options.profile.as_deref());
    let mut effective_options = options.clone();

    // Merge profile settings with CLI options
    if profile.parallel && !options.parallel {
        effective_options.parallel = true;
    }
    if profile.workers.is_some() && options.workers == 4 {
        effective_options.workers = profile.workers.unwrap();
    }
    if profile.fail_fast && !options.fail_fast {
        effective_options.fail_fast = true;
    }

    let runner = Runner::new(config, base_dir, Some(db), None)
        .with_options(effective_options);

    let results = if affected {
        // Run only affected tasks
        println!("{} Detecting affected tasks...", "🔍".cyan());
        let affected_tasks = find_affected_tasks(
            config,
            base_dir,
            since.as_deref(),
            base.as_deref(),
        )?;

        if affected_tasks.is_empty() {
            println!("{} No affected tasks found", "✓".green());
            return Ok(());
        }

        println!(
            "{} Found {} affected task(s): {}",
            "→".blue(),
            affected_tasks.len(),
            affected_tasks.join(", ").cyan()
        );

        runner.run_tasks(&affected_tasks)?
    } else if let Some(name) = task {
        if config.get_task(&name).is_none() {
            anyhow::bail!("Task '{}' not found. Use 'runx list' to see available tasks.", name);
        }
        runner.run_task(&name)?
    } else if let Some(ref pattern) = filter {
        // Filter tasks by pattern
        let matching: Vec<String> = config.task_names()
            .iter()
            .filter(|n| n.contains(pattern))
            .cloned()
            .cloned()
            .collect();

        if matching.is_empty() {
            anyhow::bail!("No tasks match pattern '{}'. Use 'runx list' to see available tasks.", pattern);
        }

        runner.run_tasks(&matching)?
    } else {
        runner.run_all()?
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

fn cmd_list(config: &Config, detailed: bool) -> Result<()> {
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

        if detailed {
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

            if task.parallel {
                println!("    {} {}", "parallel:".dimmed(), "true".cyan());
            }

            if task.retry > 0 {
                println!("    {} {}", "retry:".dimmed(), task.retry);
            }

            if task.coverage {
                println!("    {} {} (threshold: {}%)",
                    "coverage:".dimmed(),
                    "enabled".green(),
                    task.coverage_threshold.unwrap_or(0.0)
                );
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
    }

    // Show profiles if any
    if !config.profiles.is_empty() {
        println!("\n{}", "Profiles:".bold());
        for (name, profile) in &config.profiles {
            let mut flags = Vec::new();
            if profile.parallel { flags.push("parallel"); }
            if profile.cache { flags.push("cache"); }
            if profile.fail_fast { flags.push("fail-fast"); }
            if profile.notifications { flags.push("notifications"); }

            println!(
                "  {} {} {}",
                "•".blue(),
                name.cyan(),
                if flags.is_empty() { "".to_string() } else { format!("({})", flags.join(", ")).dimmed().to_string() }
            );
        }
    }

    Ok(())
}

#[tokio::main]
async fn cmd_serve(port: u16, db_path: PathBuf) -> Result<()> {
    // Initialize database if it doesn't exist
    let _ = Database::open(&db_path)?;

    println!("{}", "━".repeat(50).dimmed());
    println!("  {} {}", "Runx Live Dashboard".bold().cyan(), "v0.3.0".dimmed());
    println!("{}", "━".repeat(50).dimmed());

    server::start_server(port, db_path).await
}

fn cmd_tui(config: &Config, base_dir: &Path, db_path: &Path) -> Result<()> {
    let db = Database::open(db_path)?;
    tui::run_tui(config, base_dir, Some(db))
}

fn cmd_cache(db_path: &Path, action: CacheAction) -> Result<()> {
    let db = Database::open(db_path)?;

    match action {
        CacheAction::Clear => {
            let cleared = db.clear_all_cache()?;
            println!("{} Cleared {} cache entries", "✓".green(), cleared);
        }
        CacheAction::Show => {
            let stats = db.get_cache_stats()?;
            println!("\n{}", "Cache Statistics:".bold());
            println!("  Total entries:  {}", stats.total_entries);
            println!("  Valid entries:  {}", stats.valid_entries.to_string().green());
            println!("  Expired:        {}", stats.expired_entries.to_string().yellow());
            println!("  Time saved:     {}ms", stats.total_time_saved_ms.to_string().cyan());

            if !stats.tasks_with_cache.is_empty() {
                println!("\n  {}", "Tasks with cache:".dimmed());
                for task in &stats.tasks_with_cache {
                    println!("    • {}", task.cyan());
                }
            }
        }
    }

    Ok(())
}

fn cmd_profiles(config: &Config, action: ProfileAction) -> Result<()> {
    match action {
        ProfileAction::List => {
            println!("\n{}", "Available Profiles:".bold());

            if config.profiles.is_empty() {
                println!("  {}", "No profiles defined".dimmed());
                return Ok(());
            }

            for (name, profile) in &config.profiles {
                println!("\n  {} {}", "▸".cyan(), name.bold());

                if profile.parallel {
                    println!("    parallel: {}", "true".green());
                    if let Some(workers) = profile.workers {
                        println!("    workers: {}", workers.to_string().cyan());
                    }
                }

                if profile.cache {
                    println!("    cache: {}", "true".green());
                }

                if profile.fail_fast {
                    println!("    fail_fast: {}", "true".yellow());
                }

                if profile.notifications {
                    println!("    notifications: {}", "true".blue());
                }

                if profile.verbose {
                    println!("    verbose: {}", "true".dimmed());
                }

                if !profile.task_overrides.is_empty() {
                    println!("    task_overrides: {}", profile.task_overrides.len());
                }
            }

            if let Some(ref default) = config.project.default_profile {
                println!("\n  Default profile: {}", default.cyan());
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn cmd_annotate(
    config: &Config,
    base_dir: &Path,
    db_path: &Path,
    action: AnnotateAction,
) -> Result<()> {
    use ai::{AiConfig, TestAnnotator};

    let db = Database::open(db_path)?;

    match action {
        AnnotateAction::File { path, provider, language } => {
            println!("{} Annotating tests in {}...", "🤖".cyan(), path.display());

            // Build AI config from CLI args or config file
            let ai_config = if let Some(ref ai_cfg) = config.ai {
                AiConfig {
                    provider: ai_cfg.provider.clone(),
                    api_key: ai_cfg.api_key.clone(),
                    model: ai_cfg.model.clone(),
                    auto_annotate: ai_cfg.auto_annotate,
                    language: ai_cfg.language.clone(),
                }
            } else {
                AiConfig {
                    provider,
                    api_key: None,
                    model: None,
                    auto_annotate: false,
                    language,
                }
            };

            let annotator = TestAnnotator::new(&ai_config)?;
            let full_path = base_dir.join(&path);
            let annotations = annotator.annotate_and_store(&full_path, &db).await?;

            println!(
                "{} Annotated {} test(s)\n",
                "✓".green(),
                annotations.len()
            );

            for annotation in &annotations {
                println!("  {} {}", "▸".cyan(), annotation.test_name.bold());
                println!("    {}", annotation.description);
                if let Some(ref purpose) = annotation.purpose {
                    println!("    {} {}", "Purpose:".dimmed(), purpose);
                }
                if let Some(ref tested_fn) = annotation.tested_function {
                    println!("    {} {}", "Tests:".dimmed(), tested_fn.cyan());
                }
                if let Some(ref test_type) = annotation.test_type {
                    println!("    {} {}", "Type:".dimmed(), test_type.magenta());
                }
                if !annotation.tags.is_empty() {
                    println!("    {} {}", "Tags:".dimmed(), annotation.tags.join(", ").yellow());
                }
                println!();
            }
        }

        AnnotateAction::All { provider, language, pattern } => {
            println!("{} Scanning for test files with pattern: {}", "🔍".cyan(), pattern);

            let ai_config = if let Some(ref ai_cfg) = config.ai {
                AiConfig {
                    provider: ai_cfg.provider.clone(),
                    api_key: ai_cfg.api_key.clone(),
                    model: ai_cfg.model.clone(),
                    auto_annotate: ai_cfg.auto_annotate,
                    language: ai_cfg.language.clone(),
                }
            } else {
                AiConfig {
                    provider,
                    api_key: None,
                    model: None,
                    auto_annotate: false,
                    language,
                }
            };

            let annotator = TestAnnotator::new(&ai_config)?;

            // Find all test files
            let full_pattern = base_dir.join(&pattern);
            let mut total_annotations = 0;

            for entry in glob::glob(&full_pattern.to_string_lossy())? {
                if let Ok(path) = entry {
                    println!("  {} {}", "→".blue(), path.display());
                    match annotator.annotate_and_store(&path, &db).await {
                        Ok(annotations) => {
                            total_annotations += annotations.len();
                            println!("    {} {} test(s)", "✓".green(), annotations.len());
                        }
                        Err(e) => {
                            println!("    {} {}", "✗".red(), e);
                        }
                    }
                }
            }

            println!(
                "\n{} Total: {} annotations created",
                "✓".green().bold(),
                total_annotations
            );
        }

        AnnotateAction::Show { test_type, tag } => {
            let annotations = if let Some(ref t) = test_type {
                db.get_annotations_by_type(t)?
            } else if let Some(ref t) = tag {
                db.get_annotations_by_tag(t)?
            } else {
                db.get_all_annotations()?
            };

            if annotations.is_empty() {
                println!("{}", "No annotations found".dimmed());
                return Ok(());
            }

            println!("\n{} {} Test Annotations\n", "📝".cyan(), annotations.len());

            for annotation in &annotations {
                println!("  {} {}", "▸".cyan(), annotation.test_name.bold());
                println!("    {}", annotation.description);

                if let Some(ref purpose) = annotation.purpose {
                    println!("    {} {}", "Purpose:".dimmed(), purpose);
                }
                if let Some(ref tested_fn) = annotation.tested_function {
                    println!("    {} {}", "Tests:".dimmed(), tested_fn.cyan());
                }
                if let Some(ref test_type) = annotation.test_type {
                    println!("    {} {}", "Type:".dimmed(), test_type.magenta());
                }
                if !annotation.tags.is_empty() {
                    println!("    {} {}", "Tags:".dimmed(), annotation.tags.join(", ").yellow());
                }
                println!();
            }
        }

        AnnotateAction::Export { output } => {
            let annotations = db.get_all_annotations()?;

            if annotations.is_empty() {
                println!("{}", "No annotations to export".dimmed());
                return Ok(());
            }

            let json = serde_json::to_string_pretty(&annotations)?;
            let output_path = base_dir.join(&output);
            std::fs::write(&output_path, json)?;

            println!(
                "{} Exported {} annotations to {}",
                "✓".green(),
                annotations.len(),
                output_path.display()
            );
        }
    }

    Ok(())
}
