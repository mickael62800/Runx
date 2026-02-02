//! Flaky test detection and tracking

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Database;

/// A flaky test entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyTest {
    pub id: i64,
    pub test_name: String,
    pub classname: Option<String>,
    pub task_name: String,
    pub flaky_score: f64,
    pub total_runs: i32,
    pub pass_count: i32,
    pub fail_count: i32,
    pub quarantined: bool,
    pub updated_at: DateTime<Utc>,
}

/// Test history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestHistoryEntry {
    pub id: i64,
    pub test_name: String,
    pub classname: Option<String>,
    pub task_name: String,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub run_id: String,
    pub created_at: DateTime<Utc>,
}

impl Database {
    /// Record a test result in history
    pub fn record_test_history(
        &self,
        test_name: &str,
        classname: Option<&str>,
        task_name: &str,
        status: &str,
        duration_ms: Option<i64>,
        run_id: &str,
    ) -> Result<()> {
        let now = Utc::now();

        self.conn.execute(
            "INSERT INTO test_history (test_name, classname, task_name, status, duration_ms, run_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                test_name,
                classname,
                task_name,
                status,
                duration_ms,
                run_id,
                now.to_rfc3339(),
            ],
        )?;

        // Update flaky stats
        self.update_flaky_stats(test_name, classname, task_name, status)?;

        Ok(())
    }

    /// Update flaky statistics for a test
    fn update_flaky_stats(
        &self,
        test_name: &str,
        classname: Option<&str>,
        task_name: &str,
        status: &str,
    ) -> Result<()> {
        let now = Utc::now();
        let is_pass = status == "passed";

        // Try to update existing entry
        let updated = self.conn.execute(
            "UPDATE flaky_tests SET
                total_runs = total_runs + 1,
                pass_count = pass_count + ?1,
                fail_count = fail_count + ?2,
                flaky_score = CAST(pass_count + ?1 AS REAL) / CAST(total_runs + 1 AS REAL) * 100,
                updated_at = ?3
             WHERE test_name = ?4",
            params![
                if is_pass { 1 } else { 0 },
                if is_pass { 0 } else { 1 },
                now.to_rfc3339(),
                test_name,
            ],
        )?;

        // If no row updated, insert new entry
        if updated == 0 {
            let flaky_score = if is_pass { 100.0 } else { 0.0 };
            self.conn.execute(
                "INSERT INTO flaky_tests (test_name, classname, task_name, flaky_score, total_runs, pass_count, fail_count, quarantined, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 1, ?5, ?6, 0, ?7)",
                params![
                    test_name,
                    classname,
                    task_name,
                    flaky_score,
                    if is_pass { 1 } else { 0 },
                    if is_pass { 0 } else { 1 },
                    now.to_rfc3339(),
                ],
            )?;
        }

        Ok(())
    }

    /// Get flaky tests (tests with pass rate between 20% and 80%)
    pub fn get_flaky_tests(&self, limit: i32) -> Result<Vec<FlakyTest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, test_name, classname, task_name, flaky_score, total_runs, pass_count, fail_count, quarantined, updated_at
             FROM flaky_tests
             WHERE total_runs >= 3 AND flaky_score > 20 AND flaky_score < 80
             ORDER BY ABS(flaky_score - 50) ASC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(FlakyTest {
                id: row.get(0)?,
                test_name: row.get(1)?,
                classname: row.get(2)?,
                task_name: row.get(3)?,
                flaky_score: row.get(4)?,
                total_runs: row.get(5)?,
                pass_count: row.get(6)?,
                fail_count: row.get(7)?,
                quarantined: row.get::<_, i32>(8)? != 0,
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get quarantined tests
    pub fn get_quarantined_tests(&self) -> Result<Vec<FlakyTest>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, test_name, classname, task_name, flaky_score, total_runs, pass_count, fail_count, quarantined, updated_at
             FROM flaky_tests
             WHERE quarantined = 1
             ORDER BY updated_at DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(FlakyTest {
                id: row.get(0)?,
                test_name: row.get(1)?,
                classname: row.get(2)?,
                task_name: row.get(3)?,
                flaky_score: row.get(4)?,
                total_runs: row.get(5)?,
                pass_count: row.get(6)?,
                fail_count: row.get(7)?,
                quarantined: true,
                updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Quarantine a test
    pub fn quarantine_test(&self, test_name: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE flaky_tests SET quarantined = 1, updated_at = ?1 WHERE test_name = ?2",
            params![now.to_rfc3339(), test_name],
        )?;
        Ok(())
    }

    /// Remove a test from quarantine
    pub fn unquarantine_test(&self, test_name: &str) -> Result<()> {
        let now = Utc::now();
        self.conn.execute(
            "UPDATE flaky_tests SET quarantined = 0, updated_at = ?1 WHERE test_name = ?2",
            params![now.to_rfc3339(), test_name],
        )?;
        Ok(())
    }

    /// Check if a test is quarantined
    pub fn is_test_quarantined(&self, test_name: &str) -> Result<bool> {
        let quarantined: i32 = self.conn.query_row(
            "SELECT quarantined FROM flaky_tests WHERE test_name = ?1",
            params![test_name],
            |row| row.get(0),
        ).unwrap_or(0);

        Ok(quarantined != 0)
    }

    /// Get test history
    pub fn get_test_history(&self, test_name: &str, limit: i32) -> Result<Vec<TestHistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, test_name, classname, task_name, status, duration_ms, run_id, created_at
             FROM test_history
             WHERE test_name = ?1
             ORDER BY created_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![test_name, limit], |row| {
            Ok(TestHistoryEntry {
                id: row.get(0)?,
                test_name: row.get(1)?,
                classname: row.get(2)?,
                task_name: row.get(3)?,
                status: row.get(4)?,
                duration_ms: row.get(5)?,
                run_id: row.get(6)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(7)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Calculate flaky score for a test
    /// Returns a score between 0-100 where 50 is the most flaky
    pub fn calculate_flaky_score(&self, test_name: &str) -> Result<f64> {
        let (pass_count, total): (i32, i32) = self.conn.query_row(
            "SELECT pass_count, total_runs FROM flaky_tests WHERE test_name = ?1",
            params![test_name],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).unwrap_or((0, 0));

        if total == 0 {
            return Ok(100.0); // No history, assume stable
        }

        let pass_rate = (pass_count as f64) / (total as f64) * 100.0;
        Ok(pass_rate)
    }

    /// Get flaky test stats
    pub fn get_flaky_stats(&self) -> Result<FlakyStats> {
        let total_tracked: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM flaky_tests",
            [],
            |row| row.get(0),
        )?;

        let flaky_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM flaky_tests WHERE total_runs >= 3 AND flaky_score > 20 AND flaky_score < 80",
            [],
            |row| row.get(0),
        )?;

        let quarantined_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM flaky_tests WHERE quarantined = 1",
            [],
            |row| row.get(0),
        )?;

        let avg_flaky_score: f64 = self.conn.query_row(
            "SELECT COALESCE(AVG(ABS(flaky_score - 50)), 50) FROM flaky_tests WHERE total_runs >= 3",
            [],
            |row| row.get(0),
        ).unwrap_or(50.0);

        Ok(FlakyStats {
            total_tracked: total_tracked as u64,
            flaky_count: flaky_count as u64,
            quarantined_count: quarantined_count as u64,
            avg_stability_score: 100.0 - avg_flaky_score,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakyStats {
    pub total_tracked: u64,
    pub flaky_count: u64,
    pub quarantined_count: u64,
    pub avg_stability_score: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_flaky_detection() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        // Record alternating pass/fail
        for i in 0..10 {
            let status = if i % 2 == 0 { "passed" } else { "failed" };
            db.record_test_history(
                "flaky_test",
                Some("TestClass"),
                "test-task",
                status,
                Some(100),
                &format!("run-{}", i),
            ).unwrap();
        }

        // Check flaky score - should be around 50%
        let score = db.calculate_flaky_score("flaky_test").unwrap();
        assert!(score >= 40.0 && score <= 60.0);

        // Get flaky tests
        let flaky_tests = db.get_flaky_tests(10).unwrap();
        assert!(!flaky_tests.is_empty());
    }

    #[test]
    fn test_quarantine() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::open(&db_path).unwrap();

        // Record test history first
        db.record_test_history("test1", None, "task1", "passed", Some(100), "run-1").unwrap();

        // Quarantine
        db.quarantine_test("test1").unwrap();
        assert!(db.is_test_quarantined("test1").unwrap());

        // Unquarantine
        db.unquarantine_test("test1").unwrap();
        assert!(!db.is_test_quarantined("test1").unwrap());
    }
}
