# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Runx is a **Rust Test Explorer** - a CLI tool for discovering, running, and managing Rust tests with:
- Interactive TUI with tree view
- Automatic test discovery via `cargo test -- --list`
- Watch mode with affected test detection
- Filtering by name and status

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
├── tui/                 # Terminal UI (ratatui)
│   ├── mod.rs          # TUI entry point
│   ├── app.rs          # Application state
│   ├── ui.rs           # Rendering
│   ├── events.rs       # Keyboard handling
│   └── widgets/        # Custom widgets
│       ├── test_tree.rs    # Test tree widget
│       ├── log_viewer.rs   # Log output widget
│       └── task_list.rs    # Legacy task list widget
│
└── db/                  # SQLite persistence (for future use)
    ├── mod.rs          # Database connection
    ├── schema.rs       # Migrations
    ├── cache.rs        # Cache operations
    ├── flaky.rs        # Flaky test tracking
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

### Key Patterns

1. **Test Discovery**: Uses `cargo test -- --list` to get all test names, then builds a hierarchical tree
2. **Streaming Output**: Test runner parses cargo test output in real-time
3. **Affected Tests**: Maps source files to tests for efficient watch mode
4. **Tree Widget**: Custom ratatui widget for expand/collapse navigation

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

## Adding New Features

- **New CLI command**: Add variant to `Commands` enum in main.rs, add `cmd_*` handler
- **New test feature**: Extend `Test` struct in test_model.rs
- **TUI changes**: Update tui/ui.rs for rendering, tui/events.rs for keybindings
- **Watch triggers**: Update affected.rs for file→test mapping

## Platform Notes

- Test execution uses `cargo test` which works cross-platform
- TUI uses crossterm for cross-platform terminal support
- File watching uses notify crate
