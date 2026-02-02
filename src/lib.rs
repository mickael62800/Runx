//! Runx - Rust Test Explorer
//!
//! A library for discovering, running, and managing Rust tests with:
//! - Automatic test discovery via `cargo test -- --list`
//! - Test tree with hierarchical module structure
//! - Real-time test execution with streaming output
//! - Watch mode with affected test detection
//! - Terminal UI with tree view

pub mod affected;
pub mod db;
pub mod discovery;
pub mod test_model;
pub mod test_runner;
pub mod tui;
pub mod watcher;

pub use discovery::{discover_all_tests, discover_tests, get_project_name, is_rust_project};
pub use test_model::{FilterMode, Test, TestNode, TestStats, TestStatus};
pub use test_runner::{TestEvent, TestRunResult, TestRunner};
