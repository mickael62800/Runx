//! Database schema and migrations

use anyhow::Result;
use rusqlite::Connection;

/// Current schema version (used for documentation/debugging)
#[allow(dead_code)]
const SCHEMA_VERSION: i32 = 3;

/// Run all pending migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    // Create migrations table if it doesn't exist
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )",
        [],
    )?;

    let current_version: i32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    if current_version < 1 {
        migrate_v1(conn)?;
    }

    if current_version < 2 {
        migrate_v2(conn)?;
    }

    if current_version < 3 {
        migrate_v3(conn)?;
    }

    Ok(())
}

/// Initial schema (v0.2.0)
fn migrate_v1(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
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

        INSERT INTO schema_migrations (version, applied_at) VALUES (1, datetime('now'));
        "#,
    )?;

    Ok(())
}

/// v0.3.0 schema additions
fn migrate_v2(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- Test history for flaky detection
        CREATE TABLE IF NOT EXISTS test_history (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            test_name TEXT NOT NULL,
            classname TEXT,
            task_name TEXT NOT NULL,
            status TEXT NOT NULL,
            duration_ms INTEGER,
            run_id TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_test_history_test_name ON test_history(test_name);
        CREATE INDEX IF NOT EXISTS idx_test_history_task_name ON test_history(task_name);
        CREATE INDEX IF NOT EXISTS idx_test_history_created_at ON test_history(created_at);

        -- Flaky tests tracking
        CREATE TABLE IF NOT EXISTS flaky_tests (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            test_name TEXT NOT NULL UNIQUE,
            classname TEXT,
            task_name TEXT NOT NULL,
            flaky_score REAL NOT NULL DEFAULT 0,
            total_runs INTEGER NOT NULL DEFAULT 0,
            pass_count INTEGER NOT NULL DEFAULT 0,
            fail_count INTEGER NOT NULL DEFAULT 0,
            quarantined INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_flaky_tests_score ON flaky_tests(flaky_score DESC);
        CREATE INDEX IF NOT EXISTS idx_flaky_tests_quarantined ON flaky_tests(quarantined);

        -- Task cache
        CREATE TABLE IF NOT EXISTS task_cache (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_name TEXT NOT NULL,
            input_hash TEXT NOT NULL,
            status TEXT NOT NULL,
            duration_ms INTEGER,
            created_at TEXT NOT NULL,
            expires_at TEXT,
            UNIQUE(task_name, input_hash)
        );

        CREATE INDEX IF NOT EXISTS idx_task_cache_lookup ON task_cache(task_name, input_hash);
        CREATE INDEX IF NOT EXISTS idx_task_cache_expires ON task_cache(expires_at);

        -- Coverage results
        CREATE TABLE IF NOT EXISTS coverage_results (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_result_id TEXT NOT NULL,
            line_coverage REAL,
            branch_coverage REAL,
            lines_covered INTEGER,
            lines_total INTEGER,
            threshold_passed INTEGER,
            FOREIGN KEY (task_result_id) REFERENCES task_results(id)
        );

        CREATE INDEX IF NOT EXISTS idx_coverage_task_result ON coverage_results(task_result_id);

        -- Artifacts (screenshots, videos, logs)
        CREATE TABLE IF NOT EXISTS artifacts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_result_id TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            file_path TEXT NOT NULL,
            mime_type TEXT,
            size_bytes INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (task_result_id) REFERENCES task_results(id)
        );

        CREATE INDEX IF NOT EXISTS idx_artifacts_task_result ON artifacts(task_result_id);
        CREATE INDEX IF NOT EXISTS idx_artifacts_type ON artifacts(artifact_type);

        INSERT INTO schema_migrations (version, applied_at) VALUES (2, datetime('now'));
        "#,
    )?;

    Ok(())
}

/// v0.3.1 schema additions - AI test annotations
fn migrate_v3(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        -- AI-generated test annotations
        CREATE TABLE IF NOT EXISTS test_annotations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            test_name TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL,
            purpose TEXT,
            tested_function TEXT,
            test_type TEXT,
            tags TEXT,
            updated_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_test_annotations_name ON test_annotations(test_name);
        CREATE INDEX IF NOT EXISTS idx_test_annotations_type ON test_annotations(test_type);

        INSERT INTO schema_migrations (version, applied_at) VALUES (3, datetime('now'));
        "#,
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    #[test]
    fn test_migrations() {
        let conn = Connection::open_in_memory().unwrap();
        run_migrations(&conn).unwrap();

        // Verify all tables exist
        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(tables.contains(&"runs".to_string()));
        assert!(tables.contains(&"task_results".to_string()));
        assert!(tables.contains(&"test_cases".to_string()));
        assert!(tables.contains(&"test_history".to_string()));
        assert!(tables.contains(&"flaky_tests".to_string()));
        assert!(tables.contains(&"task_cache".to_string()));
        assert!(tables.contains(&"coverage_results".to_string()));
        assert!(tables.contains(&"artifacts".to_string()));
    }
}
