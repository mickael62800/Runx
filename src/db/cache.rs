//! Cache database operations

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Database;

/// Cached task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub id: i64,
    pub task_name: String,
    pub input_hash: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Database {
    /// Get a cache entry for a task with a specific input hash
    pub fn get_cache_entry(&self, task_name: &str, input_hash: &str) -> Result<Option<CacheEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_name, input_hash, status, duration_ms, created_at, expires_at
             FROM task_cache
             WHERE task_name = ?1 AND input_hash = ?2
             AND (expires_at IS NULL OR expires_at > datetime('now'))"
        )?;

        let mut rows = stmt.query(params![task_name, input_hash])?;

        if let Some(row) = rows.next()? {
            Ok(Some(CacheEntry {
                id: row.get(0)?,
                task_name: row.get(1)?,
                input_hash: row.get(2)?,
                status: row.get(3)?,
                duration_ms: row.get(4)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(5)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
                expires_at: row.get::<_, Option<String>>(6)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
            }))
        } else {
            Ok(None)
        }
    }

    /// Store a cache entry
    pub fn set_cache_entry(
        &self,
        task_name: &str,
        input_hash: &str,
        status: &str,
        duration_ms: Option<i64>,
        ttl_hours: u32,
    ) -> Result<()> {
        let now = Utc::now();
        let expires_at = now + Duration::hours(ttl_hours as i64);

        self.conn.execute(
            "INSERT OR REPLACE INTO task_cache (task_name, input_hash, status, duration_ms, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                task_name,
                input_hash,
                status,
                duration_ms,
                now.to_rfc3339(),
                expires_at.to_rfc3339(),
            ],
        )?;

        Ok(())
    }

    /// Invalidate cache for a specific task
    pub fn invalidate_cache(&self, task_name: &str) -> Result<u64> {
        let count = self.conn.execute(
            "DELETE FROM task_cache WHERE task_name = ?1",
            params![task_name],
        )?;
        Ok(count as u64)
    }

    /// Clear all expired cache entries
    pub fn clear_expired_cache(&self) -> Result<u64> {
        let count = self.conn.execute(
            "DELETE FROM task_cache WHERE expires_at < datetime('now')",
            [],
        )?;
        Ok(count as u64)
    }

    /// Clear all cache entries
    pub fn clear_all_cache(&self) -> Result<u64> {
        let count = self.conn.execute("DELETE FROM task_cache", [])?;
        Ok(count as u64)
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> Result<CacheStats> {
        let total_entries: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM task_cache",
            [],
            |row| row.get(0),
        )?;

        let valid_entries: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM task_cache WHERE expires_at IS NULL OR expires_at > datetime('now')",
            [],
            |row| row.get(0),
        )?;

        let expired_entries = total_entries - valid_entries;

        let hits: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM task_cache WHERE status = 'passed'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let total_size: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(duration_ms), 0) FROM task_cache WHERE status = 'passed'",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        let tasks_with_cache: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT DISTINCT task_name FROM task_cache
                 WHERE expires_at IS NULL OR expires_at > datetime('now')"
            )?;
            let result: Vec<String> = stmt.query_map([], |row| row.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            result
        };

        Ok(CacheStats {
            total_entries: total_entries as u64,
            valid_entries: valid_entries as u64,
            expired_entries: expired_entries as u64,
            cache_hits: hits as u64,
            total_time_saved_ms: total_size as u64,
            tasks_with_cache,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_entries: u64,
    pub valid_entries: u64,
    pub expired_entries: u64,
    pub cache_hits: u64,
    pub total_time_saved_ms: u64,
    pub tasks_with_cache: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_cache_operations() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        // Set cache entry
        db.set_cache_entry("test-task", "abc123", "passed", Some(1000), 24).unwrap();

        // Get cache entry
        let entry = db.get_cache_entry("test-task", "abc123").unwrap();
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.task_name, "test-task");
        assert_eq!(entry.input_hash, "abc123");
        assert_eq!(entry.status, "passed");

        // Cache stats
        let stats = db.get_cache_stats().unwrap();
        assert_eq!(stats.valid_entries, 1);

        // Clear cache
        db.clear_all_cache().unwrap();
        let entry = db.get_cache_entry("test-task", "abc123").unwrap();
        assert!(entry.is_none());
    }
}
