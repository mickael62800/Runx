# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Runx is a **Rust Test Explorer** - a CLI tool for discovering, running, and managing Rust tests with:
- Interactive TUI with tree view
- Automatic test discovery via `cargo test -- --list`
- Watch mode with affected test detection
- Filtering by name and status
- Configuration via `runx.toml` with profiles and tasks
- Code coverage support (LCOV, Cobertura)
- Notifications (Slack, Discord, GitHub)
- AI-powered test annotations (Anthropic, OpenAI)
- HTML dashboard with real-time WebSocket updates

## Build & Development Commands

```bash
# Build
cargo build                     # Debug build
cargo build --release           # Release build

# Run
cargo run                       # Run TUI (default)
cargo run -- run                # Run all tests
cargo run -- run "pattern"      # Run tests matching pattern
cargo run -- list               # List discovered tests
cargo run -- discover           # Discover tests
cargo run -- watch              # Watch mode

# Install
cargo install --path .          # Install as system command

# Test
cargo test                      # All tests

# Lint
cargo clippy                    # Static analysis
cargo check                     # Type checking
```

## Architecture

### Module Organization

```
src/
├── main.rs              # CLI entry point (clap), command dispatch
├── lib.rs               # Library exports
│
├── test_model.rs        # Test, TestNode, TestStatus structures
├── discovery.rs         # Test discovery via cargo test --list
├── test_runner.rs       # Test execution with streaming output
├── affected.rs          # File → test mapping for watch mode
├── watcher.rs           # File watching with debounce
│
├── config.rs            # Configuration (runx.toml) with profiles, tasks, notifications
├── task.rs              # Task execution (foreground/background processes)
├── junit.rs             # JUnit XML parsing for test results
├── report.rs            # HTML dashboard report generation
├── server.rs            # HTTP API + WebSocket server (Axum)
├── dashboard.html       # Embedded HTML dashboard template
│
├── ai/                  # AI-powered test annotations
│   ├── mod.rs          # Module exports
│   ├── annotator.rs    # Test annotation logic
│   └── providers.rs    # Anthropic/OpenAI providers
│
├── coverage/            # Code coverage support
│   ├── mod.rs          # CoverageData, FileCoverage types
│   ├── lcov.rs         # LCOV format parser
│   ├── cobertura.rs    # Cobertura XML parser
│   └── threshold.rs    # Coverage threshold validation
│
├── execution/           # Advanced task execution
│   ├── mod.rs          # Module exports
│   ├── runner.rs       # Runner with options
│   ├── parallel.rs     # Parallel execution with workers
│   ├── cache.rs        # Intelligent caching (file hashes)
│   └── retry.rs        # Retry logic with flaky detection
│
├── git/                 # Git integration
│   ├── mod.rs          # Module exports
│   ├── diff.rs         # Git diff parsing, file categorization
│   └── commits.rs      # Commit history, merge base detection
│
├── graph/               # Task dependency graph
│   ├── mod.rs          # Module exports
│   ├── toposort.rs     # Topological sorting
│   ├── affected.rs     # Affected task detection
│   └── workspace.rs    # Monorepo/workspace support
│
├── notifications/       # External notifications
│   ├── mod.rs          # send_notifications(), NotificationSummary
│   ├── slack.rs        # Slack webhook
│   ├── discord.rs      # Discord webhook
│   └── github.rs       # GitHub status checks
│
├── tui/                 # Terminal UI (ratatui)
│   ├── mod.rs          # TUI entry point
│   ├── app.rs          # Application state
│   ├── ui.rs           # Rendering
│   ├── events.rs       # Keyboard handling
│   └── widgets/        # Custom widgets
│       ├── test_tree.rs    # Test tree widget
│       ├── log_viewer.rs   # Log output widget
│       └── task_list.rs    # Task list widget
│
└── db/                  # SQLite persistence
    ├── mod.rs          # Database connection, Run, TaskResult, TestCase types
    ├── schema.rs       # Migrations (runs, task_results, test_cases, coverage, artifacts)
    ├── cache.rs        # Cache operations (hash-based invalidation)
    ├── flaky.rs        # Flaky test detection and tracking
    └── queries.rs      # Query builders
```

### Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `Test` | test_model.rs | A discovered test with status |
| `TestNode` | test_model.rs | Hierarchical tree node (module or test) |
| `TestStatus` | test_model.rs | Pending/Running/Passed/Failed/Ignored |
| `TestRunner` | test_runner.rs | Executes tests with streaming output |
| `TestWatcher` | watcher.rs | Watches files and re-runs affected tests |
| `App` | tui/app.rs | TUI application state |
| `Config` | config.rs | Configuration from runx.toml |
| `Task` | config.rs | Task definition with cmd, depends_on, retry, coverage |
| `Profile` | config.rs | Environment profile (dev, ci) with overrides |
| `TaskResult` | task.rs / db/mod.rs | Task execution result with timing |
| `CoverageData` | coverage/mod.rs | Coverage metrics (line/branch) |
| `Database` | db/mod.rs | SQLite connection with history, stats |
| `Run` | db/mod.rs | Test run record with pass/fail counts |
| `WsMessage` | server.rs | WebSocket message types for real-time updates |

### Key Patterns

1. **Test Discovery**: Uses `cargo test -- --list` to get all test names, then builds a hierarchical tree
2. **Streaming Output**: Test runner parses cargo test output in real-time
3. **Affected Tests**: Maps source files to tests for efficient watch mode
4. **Tree Widget**: Custom ratatui widget for expand/collapse navigation
5. **Task Graph**: Topological sorting resolves task dependencies before execution
6. **Caching**: File hash-based cache invalidation skips unchanged tasks
7. **Retry with Flaky Detection**: Automatically retries failed tests and tracks flaky patterns
8. **Real-time Dashboard**: WebSocket broadcasts task progress to connected clients

### Entry Point Flow

```
main() → run() → check_is_rust_project()
  → dispatch to command handler:
      - cmd_run()      → TestRunner.run_* → TestRunResult
      - cmd_list()     → discover_all_tests() → print tree
      - cmd_watch()    → TestWatcher.start()
      - cmd_tui()      → tui::run_tui() [default]
      - cmd_discover() → discover_all_tests() → print stats
```

## Configuration (runx.toml)

Runx supports a `runx.toml` configuration file for advanced features:

```toml
[project]
name = "my-project"
default_profile = "dev"

# Profiles for different environments
[profiles.dev]
parallel = false
cache = true
verbose = true

[profiles.ci]
parallel = true
workers = 4
cache = true
notifications = true
fail_fast = true

# Global cache settings
[cache]
enabled = true
ttl_hours = 24

# Notifications
[notifications]
enabled = true
on_failure = true

[notifications.slack]
webhook_url = "https://hooks.slack.com/..."

[notifications.discord]
webhook_url = "https://discord.com/api/webhooks/..."

[notifications.github]
enabled = true

# AI annotations
[ai]
provider = "anthropic"  # or "openai"
api_key = "${ANTHROPIC_API_KEY}"
auto_annotate = true
language = "en"

# Task definitions
[tasks.build]
cmd = "cargo build"
watch = ["src/**/*.rs"]

[tasks.test]
cmd = "cargo test"
depends_on = ["build"]
parallel = true
retry = 3
timeout_seconds = 300
coverage = true
coverage_format = "lcov"
coverage_path = "coverage/lcov.info"
coverage_threshold = 80.0
```

## CLI Usage

```bash
# TUI (default)
runx

# Run tests
runx run                    # Run all tests
runx run "pattern"          # Run tests matching pattern
runx run --failed           # Run failed tests (planned)
runx run -v                 # Verbose output

# List tests
runx list                   # List all tests
runx list "pattern"         # Filter by pattern
runx list --full            # Show full test paths

# Watch mode
runx watch                  # Watch all files
runx watch "pattern"        # Watch only matching tests

# Discovery
runx discover               # Discover and show stats
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate up/down |
| `Enter` | Run selected test/module |
| `Space` | Expand/collapse module |
| `a` | Run all tests |
| `f` | Run failed tests |
| `d` | Re-discover tests |
| `/` | Filter input mode |
| `1-4` | Filter by status (All/Passed/Failed/Pending) |
| `Tab` | Cycle filter mode |
| `e/c` | Expand/collapse all |
| `q/Esc` | Quit |

## Database Schema

SQLite database (`.runx.db`) stores:
- **runs**: Test run history (id, started_at, finished_at, status, passed, failed)
- **task_results**: Individual task results per run
- **test_cases**: JUnit-parsed test case details
- **test_history**: Flaky test detection data
- **coverage_results**: Coverage metrics per task
- **artifacts**: Test artifacts (screenshots, logs)
- **cache_entries**: File hash cache for invalidation

## Adding New Features

- **New CLI command**: Add variant to `Commands` enum in main.rs, add `cmd_*` handler
- **New test feature**: Extend `Test` struct in test_model.rs
- **TUI changes**: Update tui/ui.rs for rendering, tui/events.rs for keybindings
- **Watch triggers**: Update affected.rs for file→test mapping
- **New notification channel**: Add module in notifications/, update send_notifications()
- **New coverage format**: Add parser in coverage/, update parse_coverage()
- **New task option**: Extend `Task` struct in config.rs

## Dependencies

Key dependencies (see Cargo.toml):
- **clap**: CLI argument parsing
- **ratatui + crossterm**: TUI rendering
- **rusqlite**: SQLite database
- **notify**: File system watching
- **tokio**: Async runtime
- **axum + tower-http**: HTTP/WebSocket server
- **serde + toml**: Configuration parsing
- **chrono**: Date/time handling
- **quick-xml**: JUnit XML parsing
- **colored**: Terminal colors

## Platform Notes

- Test execution uses `cargo test` which works cross-platform
- TUI uses crossterm for cross-platform terminal support
- File watching uses notify crate
- Commands are executed via `cmd /C` on Windows, `sh -c` on Unix
