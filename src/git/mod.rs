//! Git operations module
//!
//! Provides:
//! - Git diff parsing
//! - Changed file detection
//! - Commit analysis

pub mod commits;
pub mod diff;

pub use commits::{get_recent_commits, get_commits_since, get_merge_base, ref_exists, get_branches, get_tags, CommitInfo};
pub use diff::{GitDiff, categorize_files, FileCategories};
