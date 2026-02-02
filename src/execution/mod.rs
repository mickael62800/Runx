//! Execution module for Runx
//!
//! Provides:
//! - Sequential and parallel task execution
//! - Intelligent caching
//! - Retry logic with flaky detection
//! - Timeout handling

pub mod cache;
pub mod parallel;
pub mod retry;
pub mod runner;

pub use cache::CacheManager;
pub use parallel::{calculate_execution_levels, filter_parallel_tasks, ParallelExecutor};
pub use retry::{execute_with_retry, RetryConfig};
pub use runner::{Runner, RunOptions};
