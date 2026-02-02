//! Graph module for task dependency resolution
//!
//! Provides:
//! - Topological sorting
//! - Affected task detection
//! - Workspace/monorepo support

pub mod affected;
pub mod toposort;
pub mod workspace;

pub use affected::{find_affected_tasks, find_affected_tasks_from_files, filter_tasks_by_pattern, preview_affected, AffectedPreview};
pub use toposort::{topological_sort, find_tasks_watching_file, get_dependent_tasks, get_task_dependencies};
pub use workspace::{Workspace, Package, merge_workspace_configs};
