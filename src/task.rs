//! Task result types for reporting

/// Result of executing a task
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub name: String,
    pub success: bool,
    pub duration_ms: u128,
    pub category: Option<String>,
}
