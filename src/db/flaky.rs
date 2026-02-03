//! Flaky test detection

use anyhow::Result;
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Database;

/// A flaky test entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyTest {
    pub test_name: String,
    pub task_name: String,
    pub flaky_score: f64,
    pub total_runs: i32,
    pub pass_count: i32,
    pub fail_count: i32,
}

impl Database {
    /// Get flaky tests (tests with inconsistent pass/fail patterns)
    pub fn get_flaky_tests(&self, limit: i32) -> Result<Vec<FlakyTest>> {
        let mut stmt = self.conn.prepare(
            "SELECT test_name, task_name, flaky_score, total_runs, pass_count, fail_count
             FROM flaky_tests
             WHERE total_runs >= 3 AND flaky_score > 20 AND flaky_score < 80
             ORDER BY ABS(flaky_score - 50) ASC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(FlakyTest {
                test_name: row.get(0)?,
                task_name: row.get(1)?,
                flaky_score: row.get(2)?,
                total_runs: row.get(3)?,
                pass_count: row.get(4)?,
                fail_count: row.get(5)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }
}
