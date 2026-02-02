//! Additional database queries

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use super::Database;

/// Coverage result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageResult {
    pub id: i64,
    pub task_result_id: String,
    pub line_coverage: Option<f64>,
    pub branch_coverage: Option<f64>,
    pub lines_covered: Option<i32>,
    pub lines_total: Option<i32>,
    pub threshold_passed: bool,
}

/// Artifact entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub id: i64,
    pub task_result_id: String,
    pub artifact_type: String,
    pub file_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub created_at: DateTime<Utc>,
}

impl Database {
    // === Coverage Results ===

    /// Insert coverage result
    pub fn insert_coverage_result(
        &self,
        task_result_id: &str,
        line_coverage: Option<f64>,
        branch_coverage: Option<f64>,
        lines_covered: Option<i32>,
        lines_total: Option<i32>,
        threshold_passed: bool,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO coverage_results (task_result_id, line_coverage, branch_coverage, lines_covered, lines_total, threshold_passed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                task_result_id,
                line_coverage,
                branch_coverage,
                lines_covered,
                lines_total,
                if threshold_passed { 1 } else { 0 },
            ],
        )?;
        Ok(())
    }

    /// Get coverage result for a task result
    pub fn get_coverage_result(&self, task_result_id: &str) -> Result<Option<CoverageResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_result_id, line_coverage, branch_coverage, lines_covered, lines_total, threshold_passed
             FROM coverage_results WHERE task_result_id = ?1"
        )?;

        let mut rows = stmt.query(params![task_result_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(CoverageResult {
                id: row.get(0)?,
                task_result_id: row.get(1)?,
                line_coverage: row.get(2)?,
                branch_coverage: row.get(3)?,
                lines_covered: row.get(4)?,
                lines_total: row.get(5)?,
                threshold_passed: row.get::<_, i32>(6)? != 0,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get coverage history for a task
    pub fn get_coverage_history(&self, task_name: &str, limit: i32) -> Result<Vec<CoverageResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT cr.id, cr.task_result_id, cr.line_coverage, cr.branch_coverage, cr.lines_covered, cr.lines_total, cr.threshold_passed
             FROM coverage_results cr
             JOIN task_results tr ON cr.task_result_id = tr.id
             WHERE tr.task_name = ?1
             ORDER BY tr.started_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![task_name, limit], |row| {
            Ok(CoverageResult {
                id: row.get(0)?,
                task_result_id: row.get(1)?,
                line_coverage: row.get(2)?,
                branch_coverage: row.get(3)?,
                lines_covered: row.get(4)?,
                lines_total: row.get(5)?,
                threshold_passed: row.get::<_, i32>(6)? != 0,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // === Artifacts ===

    /// Insert an artifact
    pub fn insert_artifact(
        &self,
        task_result_id: &str,
        artifact_type: &str,
        file_path: &str,
        mime_type: Option<&str>,
        size_bytes: Option<i64>,
    ) -> Result<i64> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO artifacts (task_result_id, artifact_type, file_path, mime_type, size_bytes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                task_result_id,
                artifact_type,
                file_path,
                mime_type,
                size_bytes,
                now.to_rfc3339(),
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get artifacts for a task result
    pub fn get_artifacts(&self, task_result_id: &str) -> Result<Vec<Artifact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_result_id, artifact_type, file_path, mime_type, size_bytes, created_at
             FROM artifacts WHERE task_result_id = ?1 ORDER BY created_at"
        )?;

        let rows = stmt.query_map(params![task_result_id], |row| {
            Ok(Artifact {
                id: row.get(0)?,
                task_result_id: row.get(1)?,
                artifact_type: row.get(2)?,
                file_path: row.get(3)?,
                mime_type: row.get(4)?,
                size_bytes: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Get artifacts by type
    pub fn get_artifacts_by_type(&self, artifact_type: &str, limit: i32) -> Result<Vec<Artifact>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_result_id, artifact_type, file_path, mime_type, size_bytes, created_at
             FROM artifacts WHERE artifact_type = ?1 ORDER BY created_at DESC LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![artifact_type, limit], |row| {
            Ok(Artifact {
                id: row.get(0)?,
                task_result_id: row.get(1)?,
                artifact_type: row.get(2)?,
                file_path: row.get(3)?,
                mime_type: row.get(4)?,
                size_bytes: row.get(5)?,
                created_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()),
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // === Run comparison ===

    /// Compare two runs
    pub fn compare_runs(&self, run_id_1: &str, run_id_2: &str) -> Result<RunComparison> {
        let run1 = self.get_run(run_id_1)?;
        let run2 = self.get_run(run_id_2)?;

        let tasks1 = self.get_task_results_for_run(run_id_1)?;
        let tasks2 = self.get_task_results_for_run(run_id_2)?;

        let mut new_failures = Vec::new();
        let mut fixed = Vec::new();
        let mut still_failing = Vec::new();
        let mut duration_changes = Vec::new();

        // Build lookup for run2 tasks
        let tasks2_map: std::collections::HashMap<_, _> = tasks2
            .iter()
            .map(|t| (t.task_name.clone(), t))
            .collect();

        for task1 in &tasks1 {
            if let Some(task2) = tasks2_map.get(&task1.task_name) {
                // Compare status
                if task1.status == "passed" && task2.status == "failed" {
                    new_failures.push(task1.task_name.clone());
                } else if task1.status == "failed" && task2.status == "passed" {
                    fixed.push(task1.task_name.clone());
                } else if task1.status == "failed" && task2.status == "failed" {
                    still_failing.push(task1.task_name.clone());
                }

                // Compare duration
                let duration_diff = task2.duration_ms - task1.duration_ms;
                let pct_change = if task1.duration_ms > 0 {
                    (duration_diff as f64 / task1.duration_ms as f64) * 100.0
                } else {
                    0.0
                };

                if pct_change.abs() > 10.0 {
                    duration_changes.push(DurationChange {
                        task_name: task1.task_name.clone(),
                        old_duration_ms: task1.duration_ms,
                        new_duration_ms: task2.duration_ms,
                        change_percent: pct_change,
                    });
                }
            }
        }

        Ok(RunComparison {
            run1,
            run2,
            new_failures,
            fixed,
            still_failing,
            duration_changes,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunComparison {
    pub run1: Option<super::Run>,
    pub run2: Option<super::Run>,
    pub new_failures: Vec<String>,
    pub fixed: Vec<String>,
    pub still_failing: Vec<String>,
    pub duration_changes: Vec<DurationChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurationChange {
    pub task_name: String,
    pub old_duration_ms: i64,
    pub new_duration_ms: i64,
    pub change_percent: f64,
}
