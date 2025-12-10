//! SQLite-backed storage for the HillClimber optimization system.
//!
//! Follows the patterns established by the testgen crate.

use crate::error::{HillClimberError, Result};
use crate::types::{
    BestConfig, HillClimberConfig, HillClimberConfigInput, HillClimberRun, HillClimberRunInput,
    HillClimberStats, TaskStats,
};
use rusqlite::{params, Connection, Row};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;

/// Schema SQL for HillClimber tables.
const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS hillclimber_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    hint TEXT,
    use_skills INTEGER DEFAULT 0,
    max_turns_override INTEGER DEFAULT 30,
    config_hash TEXT NOT NULL,
    is_current INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(task_id, config_hash)
);

CREATE TABLE IF NOT EXISTS hillclimber_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL,
    config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
    passed INTEGER NOT NULL,
    turns INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    step_summary TEXT,
    error_message TEXT,
    meta_model TEXT,
    proposed_change TEXT,
    change_accepted INTEGER DEFAULT 0,
    score INTEGER NOT NULL,
    is_best INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS hillclimber_best_configs (
    task_id TEXT PRIMARY KEY,
    config_id INTEGER NOT NULL REFERENCES hillclimber_configs(id),
    run_id INTEGER NOT NULL REFERENCES hillclimber_runs(id),
    score INTEGER NOT NULL,
    pass_count INTEGER DEFAULT 0,
    total_runs INTEGER DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_hc_configs_task ON hillclimber_configs(task_id);
CREATE INDEX IF NOT EXISTS idx_hc_runs_task ON hillclimber_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_hc_runs_created ON hillclimber_runs(created_at DESC);
"#;

/// SQLite-backed HillClimber store.
pub struct HillClimberStore {
    conn: Connection,
}

impl HillClimberStore {
    /// Open or create a store at the given path.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    /// Open an in-memory store (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    /// Ensure the schema exists.
    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA_SQL)?;
        Ok(())
    }

    // ========================================================================
    // Config Operations
    // ========================================================================

    /// Save a config, returning existing if hash matches.
    pub fn save_config(&self, input: &HillClimberConfigInput) -> Result<HillClimberConfig> {
        let hash = hash_config(input);

        // Check for existing config with same hash
        if let Some(existing) = self.get_config_by_hash(&input.task_id, &hash)? {
            return Ok(existing);
        }

        // Insert new config
        self.conn.execute(
            r#"INSERT INTO hillclimber_configs
               (task_id, hint, use_skills, max_turns_override, config_hash, is_current)
               VALUES (?1, ?2, ?3, ?4, ?5, 0)"#,
            params![
                input.task_id,
                input.hint,
                input.use_skills as i32,
                input.max_turns_override,
                hash,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_config_by_id(id)?
            .ok_or_else(|| HillClimberError::ConfigNotFound(format!("id={}", id)))
    }

    /// Get the current config for a task.
    pub fn get_current_config(&self, task_id: &str) -> Result<Option<HillClimberConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM hillclimber_configs WHERE task_id = ?1 AND is_current = 1",
        )?;

        match stmt.query_row(params![task_id], row_to_config) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set the current config for a task.
    pub fn set_current_config(&self, task_id: &str, config_id: i64) -> Result<()> {
        // Clear current flag for all configs of this task
        self.conn.execute(
            "UPDATE hillclimber_configs SET is_current = 0 WHERE task_id = ?1",
            params![task_id],
        )?;

        // Set current flag for specified config
        self.conn.execute(
            "UPDATE hillclimber_configs SET is_current = 1 WHERE id = ?1",
            params![config_id],
        )?;

        Ok(())
    }

    /// Get a config by ID.
    pub fn get_config_by_id(&self, id: i64) -> Result<Option<HillClimberConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_configs WHERE id = ?1")?;

        match stmt.query_row(params![id], row_to_config) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get a config by hash.
    pub fn get_config_by_hash(
        &self,
        task_id: &str,
        config_hash: &str,
    ) -> Result<Option<HillClimberConfig>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM hillclimber_configs WHERE task_id = ?1 AND config_hash = ?2",
        )?;

        match stmt.query_row(params![task_id, config_hash], row_to_config) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Ensure a default config exists for a task.
    pub fn ensure_default_config(&self, task_id: &str) -> Result<HillClimberConfig> {
        if let Some(current) = self.get_current_config(task_id)? {
            return Ok(current);
        }

        // Create default config
        let default = HillClimberConfigInput {
            task_id: task_id.to_string(),
            hint: None,
            use_skills: false,
            max_turns_override: 30,
        };

        let saved = self.save_config(&default)?;
        self.set_current_config(task_id, saved.id)?;

        Ok(HillClimberConfig {
            is_current: true,
            ..saved
        })
    }

    // ========================================================================
    // Run Operations
    // ========================================================================

    /// Save a run record.
    pub fn save_run(&self, input: &HillClimberRunInput) -> Result<HillClimberRun> {
        let step_summary_json = input
            .step_summary
            .as_ref()
            .map(|s| serde_json::to_string(s))
            .transpose()?;

        self.conn.execute(
            r#"INSERT INTO hillclimber_runs
               (run_id, task_id, config_id, passed, turns, duration_ms,
                step_summary, error_message, meta_model, proposed_change,
                change_accepted, score, is_best)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0)"#,
            params![
                input.run_id,
                input.task_id,
                input.config_id,
                input.passed as i32,
                input.turns,
                input.duration_ms as i64,
                step_summary_json,
                input.error_message,
                input.meta_model,
                input.proposed_change,
                input.change_accepted as i32,
                input.score,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_run_by_id(id)
    }

    /// Get a run by ID.
    pub fn get_run_by_id(&self, id: i64) -> Result<HillClimberRun> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_runs WHERE id = ?1")?;

        stmt.query_row(params![id], row_to_run)
            .map_err(HillClimberError::from)
    }

    /// Get run history for a task.
    pub fn get_run_history(&self, task_id: &str, limit: u32) -> Result<Vec<HillClimberRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM hillclimber_runs WHERE task_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![task_id, limit], row_to_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(HillClimberError::from)
    }

    /// Get the best run for a task.
    pub fn get_best_run(&self, task_id: &str) -> Result<Option<HillClimberRun>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_runs WHERE task_id = ?1 AND is_best = 1")?;

        match stmt.query_row(params![task_id], row_to_run) {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get recent runs across all tasks.
    pub fn get_recent_runs(&self, limit: u32) -> Result<Vec<HillClimberRun>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_runs ORDER BY created_at DESC LIMIT ?1")?;

        let rows = stmt.query_map(params![limit], row_to_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(HillClimberError::from)
    }

    // ========================================================================
    // Best Config Operations
    // ========================================================================

    /// Update best config for a task if this run is better.
    pub fn update_best_config(
        &self,
        task_id: &str,
        config_id: i64,
        run_id: i64,
        score: i32,
        passed: bool,
    ) -> Result<()> {
        // Check if we have an existing best config
        let existing = self.get_best_config_for_task(task_id)?;

        if let Some(existing) = existing {
            if score > existing.score || config_id == existing.config_id {
                // Clear old best flag if changing run
                if run_id != existing.run_id {
                    self.conn.execute(
                        "UPDATE hillclimber_runs SET is_best = 0 WHERE task_id = ?1 AND is_best = 1",
                        params![task_id],
                    )?;
                }

                if score > existing.score {
                    // Update best config record
                    self.conn.execute(
                        r#"UPDATE hillclimber_best_configs
                           SET config_id = ?1, run_id = ?2, score = ?3,
                               pass_count = pass_count + ?4, total_runs = total_runs + 1,
                               updated_at = datetime('now')
                           WHERE task_id = ?5"#,
                        params![config_id, run_id, score, passed as i32, task_id],
                    )?;

                    // Set new best flag
                    self.conn.execute(
                        "UPDATE hillclimber_runs SET is_best = 1 WHERE id = ?1",
                        params![run_id],
                    )?;
                } else {
                    // Same config, just update counts
                    self.conn.execute(
                        r#"UPDATE hillclimber_best_configs
                           SET pass_count = pass_count + ?1, total_runs = total_runs + 1,
                               updated_at = datetime('now')
                           WHERE task_id = ?2"#,
                        params![passed as i32, task_id],
                    )?;
                }
            }
        } else {
            // Insert new best config
            self.conn.execute(
                r#"INSERT INTO hillclimber_best_configs
                   (task_id, config_id, run_id, score, pass_count, total_runs)
                   VALUES (?1, ?2, ?3, ?4, ?5, 1)"#,
                params![task_id, config_id, run_id, score, passed as i32],
            )?;

            // Set best flag on run
            self.conn.execute(
                "UPDATE hillclimber_runs SET is_best = 1 WHERE id = ?1",
                params![run_id],
            )?;
        }

        Ok(())
    }

    /// Get all best configs.
    pub fn get_best_configs(&self) -> Result<Vec<BestConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_best_configs ORDER BY task_id")?;

        let rows = stmt.query_map([], row_to_best_config)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(HillClimberError::from)
    }

    /// Get best config for a task.
    pub fn get_best_config_for_task(&self, task_id: &str) -> Result<Option<BestConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM hillclimber_best_configs WHERE task_id = ?1")?;

        match stmt.query_row(params![task_id], row_to_best_config) {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // ========================================================================
    // Stats
    // ========================================================================

    /// Get aggregate stats.
    pub fn get_stats(&self) -> Result<HillClimberStats> {
        // Total runs and passes
        let (total_runs, total_passes, unique_tasks): (i64, i64, i64) = self.conn.query_row(
            r#"SELECT
                COUNT(*) as total_runs,
                COALESCE(SUM(passed), 0) as total_passes,
                COUNT(DISTINCT task_id) as unique_tasks
               FROM hillclimber_runs"#,
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;

        // Unique configs
        let unique_configs: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM hillclimber_configs",
            [],
            |row| row.get(0),
        )?;

        // Per-task stats
        let mut stmt = self.conn.prepare(
            r#"SELECT
                task_id,
                COUNT(*) as total_runs,
                COALESCE(SUM(passed), 0) as pass_count,
                MAX(score) as best_score,
                AVG(turns) as avg_turns,
                MAX(created_at) as last_run_at
               FROM hillclimber_runs
               GROUP BY task_id"#,
        )?;

        let mut by_task: HashMap<String, TaskStats> = HashMap::new();

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let task_id: String = row.get(0)?;
            let task_total_runs: i64 = row.get(1)?;
            let task_pass_count: i64 = row.get(2)?;
            let best_score: i32 = row.get(3)?;
            let avg_turns: f64 = row.get(4)?;
            let last_run_at: Option<String> = row.get(5)?;

            // Get current and best config IDs
            let current_config_id: Option<i64> = self
                .conn
                .query_row(
                    "SELECT id FROM hillclimber_configs WHERE task_id = ?1 AND is_current = 1",
                    params![&task_id],
                    |row| row.get(0),
                )
                .ok();

            let best_config_id: Option<i64> = self
                .conn
                .query_row(
                    "SELECT config_id FROM hillclimber_best_configs WHERE task_id = ?1",
                    params![&task_id],
                    |row| row.get(0),
                )
                .ok();

            by_task.insert(
                task_id.clone(),
                TaskStats {
                    task_id,
                    total_runs: task_total_runs as u64,
                    pass_count: task_pass_count as u64,
                    pass_rate: if task_total_runs > 0 {
                        task_pass_count as f64 / task_total_runs as f64
                    } else {
                        0.0
                    },
                    best_score,
                    avg_turns,
                    last_run_at,
                    current_config_id,
                    best_config_id,
                },
            );
        }

        Ok(HillClimberStats {
            total_runs: total_runs as u64,
            total_passes: total_passes as u64,
            overall_pass_rate: if total_runs > 0 {
                total_passes as f64 / total_runs as f64
            } else {
                0.0
            },
            unique_tasks: unique_tasks as u64,
            unique_configs: unique_configs as u64,
            by_task,
        })
    }

    /// Get stats for a specific task.
    pub fn get_task_stats(&self, task_id: &str) -> Result<Option<TaskStats>> {
        let result: std::result::Result<(i64, i64, i32, f64, Option<String>), _> =
            self.conn.query_row(
                r#"SELECT
                    COUNT(*) as total_runs,
                    COALESCE(SUM(passed), 0) as pass_count,
                    MAX(score) as best_score,
                    AVG(turns) as avg_turns,
                    MAX(created_at) as last_run_at
                   FROM hillclimber_runs
                   WHERE task_id = ?1
                   GROUP BY task_id"#,
                params![task_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            );

        match result {
            Ok((total_runs, pass_count, best_score, avg_turns, last_run_at)) => {
                let current_config_id: Option<i64> = self
                    .conn
                    .query_row(
                        "SELECT id FROM hillclimber_configs WHERE task_id = ?1 AND is_current = 1",
                        params![task_id],
                        |row| row.get(0),
                    )
                    .ok();

                let best_config_id: Option<i64> = self
                    .conn
                    .query_row(
                        "SELECT config_id FROM hillclimber_best_configs WHERE task_id = ?1",
                        params![task_id],
                        |row| row.get(0),
                    )
                    .ok();

                Ok(Some(TaskStats {
                    task_id: task_id.to_string(),
                    total_runs: total_runs as u64,
                    pass_count: pass_count as u64,
                    pass_rate: if total_runs > 0 {
                        pass_count as f64 / total_runs as f64
                    } else {
                        0.0
                    },
                    best_score,
                    avg_turns,
                    last_run_at,
                    current_config_id,
                    best_config_id,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

// ============================================================================
// Row Mappers
// ============================================================================

fn row_to_config(row: &Row) -> rusqlite::Result<HillClimberConfig> {
    Ok(HillClimberConfig {
        id: row.get(0)?,
        task_id: row.get(1)?,
        hint: row.get(2)?,
        use_skills: row.get::<_, i32>(3)? != 0,
        max_turns_override: row.get(4)?,
        config_hash: row.get(5)?,
        is_current: row.get::<_, i32>(6)? != 0,
        created_at: row.get(7)?,
    })
}

fn row_to_run(row: &Row) -> rusqlite::Result<HillClimberRun> {
    let step_summary_json: Option<String> = row.get(7)?;
    let step_summary = step_summary_json
        .and_then(|s| serde_json::from_str(&s).ok());

    Ok(HillClimberRun {
        id: row.get(0)?,
        run_id: row.get(1)?,
        task_id: row.get(2)?,
        config_id: row.get(3)?,
        passed: row.get::<_, i32>(4)? != 0,
        turns: row.get(5)?,
        duration_ms: row.get::<_, i64>(6)? as u64,
        step_summary,
        error_message: row.get(8)?,
        meta_model: row.get(9)?,
        proposed_change: row.get(10)?,
        change_accepted: row.get::<_, i32>(11)? != 0,
        score: row.get(12)?,
        is_best: row.get::<_, i32>(13)? != 0,
        created_at: row.get(14)?,
    })
}

fn row_to_best_config(row: &Row) -> rusqlite::Result<BestConfig> {
    Ok(BestConfig {
        task_id: row.get(0)?,
        config_id: row.get(1)?,
        run_id: row.get(2)?,
        score: row.get(3)?,
        pass_count: row.get(4)?,
        total_runs: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

// ============================================================================
// Helpers
// ============================================================================

/// Hash a config input for deduplication.
pub fn hash_config(config: &HillClimberConfigInput) -> String {
    let data = serde_json::json!({
        "hint": config.hint,
        "use_skills": config.use_skills,
        "max_turns_override": config.max_turns_override,
    });

    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // 16 hex chars
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_config() {
        let config1 = HillClimberConfigInput {
            task_id: "test".to_string(),
            hint: Some("hint1".to_string()),
            use_skills: true,
            max_turns_override: 30,
        };

        let config2 = HillClimberConfigInput {
            task_id: "test".to_string(),
            hint: Some("hint1".to_string()),
            use_skills: true,
            max_turns_override: 30,
        };

        let config3 = HillClimberConfigInput {
            task_id: "test".to_string(),
            hint: Some("hint2".to_string()),
            use_skills: true,
            max_turns_override: 30,
        };

        assert_eq!(hash_config(&config1), hash_config(&config2));
        assert_ne!(hash_config(&config1), hash_config(&config3));
    }

    #[test]
    fn test_store_config_operations() -> Result<()> {
        let store = HillClimberStore::open_in_memory()?;

        let input = HillClimberConfigInput {
            task_id: "regex-log".to_string(),
            hint: Some("Use iterative refinement".to_string()),
            use_skills: false,
            max_turns_override: 30,
        };

        let config = store.save_config(&input)?;
        assert_eq!(config.task_id, "regex-log");
        assert_eq!(config.hint, Some("Use iterative refinement".to_string()));

        // Saving same config should return existing
        let config2 = store.save_config(&input)?;
        assert_eq!(config.id, config2.id);

        // Set as current
        store.set_current_config("regex-log", config.id)?;
        let current = store.get_current_config("regex-log")?;
        assert!(current.is_some());
        assert_eq!(current.unwrap().id, config.id);

        Ok(())
    }

    #[test]
    fn test_store_run_operations() -> Result<()> {
        let store = HillClimberStore::open_in_memory()?;

        // Create config first
        let config = store.save_config(&HillClimberConfigInput {
            task_id: "test-task".to_string(),
            hint: None,
            use_skills: false,
            max_turns_override: 30,
        })?;

        // Save run
        let run_input = HillClimberRunInput {
            run_id: "hc-test-001".to_string(),
            task_id: "test-task".to_string(),
            config_id: config.id,
            passed: true,
            turns: 10,
            duration_ms: 5000,
            step_summary: Some(vec!["Step 1".to_string(), "Step 2".to_string()]),
            error_message: None,
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score: 1090,
        };

        let run = store.save_run(&run_input)?;
        assert_eq!(run.task_id, "test-task");
        assert!(run.passed);
        assert_eq!(run.turns, 10);
        assert_eq!(run.score, 1090);

        // Get history
        let history = store.get_run_history("test-task", 10)?;
        assert_eq!(history.len(), 1);

        Ok(())
    }

    #[test]
    fn test_ensure_default_config() -> Result<()> {
        let store = HillClimberStore::open_in_memory()?;

        let config = store.ensure_default_config("new-task")?;
        assert_eq!(config.task_id, "new-task");
        assert!(config.hint.is_none());
        assert!(!config.use_skills);
        assert_eq!(config.max_turns_override, 30);
        assert!(config.is_current);

        // Calling again should return same config
        let config2 = store.ensure_default_config("new-task")?;
        assert_eq!(config.id, config2.id);

        Ok(())
    }
}
