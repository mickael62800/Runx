use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS runs (
    id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    status TEXT NOT NULL DEFAULT 'running',
    total_tasks INTEGER NOT NULL DEFAULT 0,
    passed INTEGER NOT NULL DEFAULT 0,
    failed INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS task_results (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_name TEXT NOT NULL,
    category TEXT,
    status TEXT NOT NULL,
    duration_ms INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    output TEXT,
    FOREIGN KEY (run_id) REFERENCES runs(id)
);

CREATE TABLE IF NOT EXISTS test_cases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_result_id TEXT NOT NULL,
    name TEXT NOT NULL,
    classname TEXT,
    status TEXT NOT NULL,
    duration_ms INTEGER,
    error_message TEXT,
    error_type TEXT,
    FOREIGN KEY (task_result_id) REFERENCES task_results(id)
);

CREATE INDEX IF NOT EXISTS idx_task_results_run_id ON task_results(run_id);
CREATE INDEX IF NOT EXISTS idx_test_cases_task_result_id ON test_cases(task_result_id);
CREATE INDEX IF NOT EXISTS idx_runs_started_at ON runs(started_at);
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: String,
    pub total_tasks: i32,
    pub passed: i32,
    pub failed: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: String,
    pub run_id: String,
    pub task_name: String,
    pub category: Option<String>,
    pub status: String,
    pub duration_ms: i64,
    pub started_at: DateTime<Utc>,
    pub output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: i64,
    pub task_result_id: String,
    pub name: String,
    pub classname: Option<String>,
    pub status: String,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub error_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummary {
    pub run: Run,
    pub tasks: Vec<TaskResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_runs: i32,
    pub total_tasks_executed: i32,
    pub overall_pass_rate: f64,
    pub avg_duration_ms: i64,
    pub recent_runs: Vec<Run>,
    pub pass_rate_history: Vec<PassRatePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassRatePoint {
    pub date: String,
    pub pass_rate: f64,
    pub run_count: i32,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    pub fn open_default() -> Result<Self> {
        let path = std::env::current_dir()?.join(".runx.db");
        Self::open(&path)
    }

    // === Runs ===

    pub fn create_run(&self, id: &str, total_tasks: i32) -> Result<Run> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO runs (id, started_at, status, total_tasks) VALUES (?1, ?2, 'running', ?3)",
            params![id, now.to_rfc3339(), total_tasks],
        )?;

        Ok(Run {
            id: id.to_string(),
            started_at: now,
            finished_at: None,
            status: "running".to_string(),
            total_tasks,
            passed: 0,
            failed: 0,
        })
    }

    pub fn finish_run(&self, id: &str, passed: i32, failed: i32) -> Result<()> {
        let now = Utc::now();
        let status = if failed > 0 { "failed" } else { "passed" };
        self.conn.execute(
            "UPDATE runs SET finished_at = ?1, status = ?2, passed = ?3, failed = ?4 WHERE id = ?5",
            params![now.to_rfc3339(), status, passed, failed, id],
        )?;
        Ok(())
    }

    pub fn get_run(&self, id: &str) -> Result<Option<Run>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, started_at, finished_at, status, total_tasks, passed, failed FROM runs WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Run {
                id: row.get(0)?,
                started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?)?.with_timezone(&Utc),
                finished_at: row.get::<_, Option<String>>(2)?
                    .map(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .flatten()
                    .map(|dt| dt.with_timezone(&Utc)),
                status: row.get(3)?,
                total_tasks: row.get(4)?,
                passed: row.get(5)?,
                failed: row.get(6)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_recent_runs(&self, limit: i32) -> Result<Vec<Run>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, started_at, finished_at, status, total_tasks, passed, failed
             FROM runs ORDER BY started_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            Ok(Run {
                id: row.get(0)?,
                started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(1)?).unwrap().with_timezone(&Utc),
                finished_at: row.get::<_, Option<String>>(2)?
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|dt| dt.with_timezone(&Utc)),
                status: row.get(3)?,
                total_tasks: row.get(4)?,
                passed: row.get(5)?,
                failed: row.get(6)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // === Task Results ===

    pub fn insert_task_result(&self, result: &TaskResult) -> Result<()> {
        self.conn.execute(
            "INSERT INTO task_results (id, run_id, task_name, category, status, duration_ms, started_at, output)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                result.id,
                result.run_id,
                result.task_name,
                result.category,
                result.status,
                result.duration_ms,
                result.started_at.to_rfc3339(),
                result.output,
            ],
        )?;
        Ok(())
    }

    pub fn get_task_results_for_run(&self, run_id: &str) -> Result<Vec<TaskResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, run_id, task_name, category, status, duration_ms, started_at, output
             FROM task_results WHERE run_id = ?1 ORDER BY started_at"
        )?;

        let rows = stmt.query_map(params![run_id], |row| {
            Ok(TaskResult {
                id: row.get(0)?,
                run_id: row.get(1)?,
                task_name: row.get(2)?,
                category: row.get(3)?,
                status: row.get(4)?,
                duration_ms: row.get(5)?,
                started_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(6)?).unwrap().with_timezone(&Utc),
                output: row.get(7)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // === Test Cases ===

    pub fn insert_test_cases(&self, task_result_id: &str, cases: &[TestCase]) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "INSERT INTO test_cases (task_result_id, name, classname, status, duration_ms, error_message, error_type)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"
        )?;

        for case in cases {
            stmt.execute(params![
                task_result_id,
                case.name,
                case.classname,
                case.status,
                case.duration_ms,
                case.error_message,
                case.error_type,
            ])?;
        }
        Ok(())
    }

    pub fn get_test_cases_for_task(&self, task_result_id: &str) -> Result<Vec<TestCase>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, task_result_id, name, classname, status, duration_ms, error_message, error_type
             FROM test_cases WHERE task_result_id = ?1"
        )?;

        let rows = stmt.query_map(params![task_result_id], |row| {
            Ok(TestCase {
                id: row.get(0)?,
                task_result_id: row.get(1)?,
                name: row.get(2)?,
                classname: row.get(3)?,
                status: row.get(4)?,
                duration_ms: row.get(5)?,
                error_message: row.get(6)?,
                error_type: row.get(7)?,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
    }

    // === Stats ===

    pub fn get_dashboard_stats(&self) -> Result<DashboardStats> {
        // Total runs
        let total_runs: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM runs",
            [],
            |row| row.get(0),
        )?;

        // Total tasks executed
        let total_tasks_executed: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM task_results",
            [],
            |row| row.get(0),
        )?;

        // Overall pass rate
        let (total_passed, total_failed): (i32, i32) = self.conn.query_row(
            "SELECT COALESCE(SUM(passed), 0), COALESCE(SUM(failed), 0) FROM runs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let overall_pass_rate = if total_passed + total_failed > 0 {
            (total_passed as f64) / ((total_passed + total_failed) as f64) * 100.0
        } else {
            0.0
        };

        // Average duration
        let avg_duration_ms: i64 = self.conn.query_row(
            "SELECT COALESCE(AVG(duration_ms), 0) FROM task_results",
            [],
            |row| row.get(0),
        ).unwrap_or(0);

        // Recent runs
        let recent_runs = self.get_recent_runs(20)?;

        // Pass rate history (last 7 days)
        let mut stmt = self.conn.prepare(
            "SELECT DATE(started_at) as date,
                    AVG(CAST(passed AS FLOAT) / CAST(passed + failed AS FLOAT)) * 100 as pass_rate,
                    COUNT(*) as run_count
             FROM runs
             WHERE started_at >= datetime('now', '-7 days') AND (passed + failed) > 0
             GROUP BY DATE(started_at)
             ORDER BY date"
        )?;

        let pass_rate_history: Vec<PassRatePoint> = stmt.query_map([], |row| {
            Ok(PassRatePoint {
                date: row.get(0)?,
                pass_rate: row.get(1)?,
                run_count: row.get(2)?,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(DashboardStats {
            total_runs,
            total_tasks_executed,
            overall_pass_rate,
            avg_duration_ms,
            recent_runs,
            pass_rate_history,
        })
    }

    pub fn get_run_summary(&self, run_id: &str) -> Result<Option<RunSummary>> {
        if let Some(run) = self.get_run(run_id)? {
            let tasks = self.get_task_results_for_run(run_id)?;
            Ok(Some(RunSummary { run, tasks }))
        } else {
            Ok(None)
        }
    }
}
