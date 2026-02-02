//! Intelligent caching for task execution

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

use crate::config::Task;
use crate::db::Database;

/// Calculate a hash of task inputs for cache key
pub fn calculate_input_hash(task: &Task, task_name: &str, base_dir: &Path) -> Result<String> {
    let mut hasher = Sha256::new();

    // Include task name
    hasher.update(task_name.as_bytes());

    // Include command
    hasher.update(task.cmd.as_bytes());

    // Include working directory
    if let Some(ref cwd) = task.cwd {
        hasher.update(cwd.as_bytes());
    }

    // Include environment variables (sorted for consistency)
    let mut env_keys: Vec<_> = task.env.keys().collect();
    env_keys.sort();
    for key in env_keys {
        hasher.update(key.as_bytes());
        hasher.update(task.env.get(key).unwrap().as_bytes());
    }

    // Hash input files
    let input_patterns: Vec<&str> = task.watch.iter()
        .chain(task.inputs.iter())
        .map(|s| s.as_str())
        .collect();

    for pattern in input_patterns {
        let glob_pattern = if pattern.starts_with('/') || pattern.contains(':') {
            pattern.to_string()
        } else {
            format!("{}/{}", base_dir.display(), pattern)
        };

        if let Ok(paths) = glob::glob(&glob_pattern) {
            let mut sorted_paths: Vec<_> = paths.filter_map(|p| p.ok()).collect();
            sorted_paths.sort();

            for path in sorted_paths {
                if path.is_file() {
                    // Hash file path
                    hasher.update(path.to_string_lossy().as_bytes());

                    // Hash file content
                    if let Ok(content) = fs::read(&path) {
                        hasher.update(&content);
                    }
                }
            }
        }
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}

/// Check if a cached result exists and is valid
pub fn check_cache(
    db: &Database,
    task_name: &str,
    input_hash: &str,
) -> Result<Option<CachedResult>> {
    if let Some(entry) = db.get_cache_entry(task_name, input_hash)? {
        Ok(Some(CachedResult {
            status: entry.status,
            duration_ms: entry.duration_ms,
        }))
    } else {
        Ok(None)
    }
}

/// Store a task result in cache
pub fn store_cache(
    db: &Database,
    task_name: &str,
    input_hash: &str,
    status: &str,
    duration_ms: Option<i64>,
    ttl_hours: u32,
) -> Result<()> {
    db.set_cache_entry(task_name, input_hash, status, duration_ms, ttl_hours)
}

/// A cached task result
#[derive(Debug, Clone)]
pub struct CachedResult {
    pub status: String,
    pub duration_ms: Option<i64>,
}

impl CachedResult {
    pub fn is_success(&self) -> bool {
        self.status == "passed"
    }
}

/// Cache manager for convenient cache operations
pub struct CacheManager<'a> {
    db: &'a Database,
    ttl_hours: u32,
    enabled: bool,
}

impl<'a> CacheManager<'a> {
    pub fn new(db: &'a Database, ttl_hours: u32, enabled: bool) -> Self {
        Self { db, ttl_hours, enabled }
    }

    /// Check if a task can be skipped due to cache hit
    pub fn check(&self, task: &Task, task_name: &str, base_dir: &Path) -> Result<Option<CachedResult>> {
        if !self.enabled {
            return Ok(None);
        }

        let hash = calculate_input_hash(task, task_name, base_dir)?;
        check_cache(self.db, task_name, &hash)
    }

    /// Store a task result in cache
    pub fn store(&self, task: &Task, task_name: &str, base_dir: &Path, status: &str, duration_ms: i64) -> Result<()> {
        if !self.enabled {
            return Ok(());
        }

        let hash = calculate_input_hash(task, task_name, base_dir)?;
        store_cache(self.db, task_name, &hash, status, Some(duration_ms), self.ttl_hours)
    }

    /// Invalidate cache for a task
    pub fn invalidate(&self, task_name: &str) -> Result<u64> {
        self.db.invalidate_cache(task_name)
    }

    /// Clear all cache
    pub fn clear_all(&self) -> Result<u64> {
        self.db.clear_all_cache()
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<crate::db::CacheStats> {
        self.db.get_cache_stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_test_task() -> Task {
        Task {
            cmd: "echo test".to_string(),
            cwd: None,
            watch: vec!["*.rs".to_string()],
            depends_on: vec![],
            background: false,
            ready_when: None,
            ready_timeout: 30,
            category: None,
            results: None,
            parallel: false,
            workers: None,
            retry: 0,
            retry_delay_ms: 1000,
            timeout_seconds: None,
            coverage: false,
            coverage_format: None,
            coverage_path: None,
            coverage_threshold: None,
            artifacts: vec![],
            env: HashMap::new(),
            inputs: vec![],
            outputs: vec![],
        }
    }

    #[test]
    fn test_input_hash_consistency() {
        let dir = tempdir().unwrap();
        let task = create_test_task();

        let hash1 = calculate_input_hash(&task, "test", dir.path()).unwrap();
        let hash2 = calculate_input_hash(&task, "test", dir.path()).unwrap();

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_input_hash_changes_with_command() {
        let dir = tempdir().unwrap();
        let mut task1 = create_test_task();
        let mut task2 = create_test_task();
        task2.cmd = "echo different".to_string();

        let hash1 = calculate_input_hash(&task1, "test", dir.path()).unwrap();
        let hash2 = calculate_input_hash(&task2, "test", dir.path()).unwrap();

        assert_ne!(hash1, hash2);
    }
}
