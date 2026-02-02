//! Runx - Universal CLI for task orchestration with live dashboard
//!
//! This library provides the core functionality for:
//! - Task configuration and parsing
//! - Parallel execution with dependency resolution
//! - Intelligent caching
//! - Flaky test detection
//! - Coverage integration
//! - TUI and web dashboard

pub mod config;
pub mod coverage;
pub mod db;
pub mod execution;
pub mod git;
pub mod graph;
pub mod junit;
pub mod notifications;
pub mod report;
pub mod server;
pub mod task;
pub mod tui;
pub mod watcher;

pub use config::{Config, Profile, Task};
pub use db::Database;
pub use execution::Runner;
pub use task::TaskResult;
