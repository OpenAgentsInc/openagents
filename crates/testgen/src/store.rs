//! TestGen Store
//!
//! SQLite persistence layer for TestGen configs, runs, and evolution history.

use crate::error::{Result, TestGenError};
use crate::types::{
    ModelType, TestCategory, TestGenBestConfig, TestGenConfig, TestGenConfigInput,
    TestGenRun, TestGenRunInput, TestGenStats,
};
use rusqlite::{params, Connection, Row};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// TestGen storage backed by SQLite
pub struct TestGenStore {
    conn: Connection,
}

impl TestGenStore {
    /// Open or create database at path
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    /// Open in-memory database (for testing)
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.ensure_schema()?;
        Ok(store)
    }

    /// Create tables if they don't exist
    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute_batch(SCHEMA_SQL)?;
        Ok(())
    }

    // ========================================================================
    // Config Operations
    // ========================================================================

    /// Save a config (deduplicates by hash)
    pub fn save_config(&self, input: &TestGenConfigInput) -> Result<TestGenConfig> {
        let hash = hash_config(input);

        // Check for existing
        if let Some(existing) = self.get_config_by_hash(&hash)? {
            return Ok(existing);
        }

        let category_order = input
            .category_order
            .as_ref()
            .map(|v| {
                serde_json::to_string(
                    &v.iter().map(|c| c.as_str()).collect::<Vec<_>>(),
                )
                .unwrap_or_default()
            })
            .unwrap_or_else(|| {
                r#"["anti_cheat","existence","correctness","boundary","integration"]"#.to_string()
            });

        let category_prompts = input
            .category_prompts
            .as_ref()
            .map(|p| serde_json::to_string(p).unwrap_or_default());

        self.conn.execute(
            r#"INSERT INTO testgen_configs (
                version, temperature, max_tokens, min_tests_per_category,
                max_tests_per_category, max_rounds_per_category,
                environment_weight, anti_cheat_weight, precision_weight,
                category_order, category_prompts, anti_cheat_prompt, reflection_prompt,
                primary_model, reflection_model,
                min_comprehensiveness_score, target_comprehensiveness_score,
                config_hash, is_current
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 0)"#,
            params![
                input.version.as_deref().unwrap_or("1.0.0"),
                input.temperature.unwrap_or(0.3),
                input.max_tokens.unwrap_or(2048),
                input.min_tests_per_category.unwrap_or(2),
                input.max_tests_per_category.unwrap_or(5),
                input.max_rounds_per_category.unwrap_or(3),
                input.environment_weight.unwrap_or(0.7),
                input.anti_cheat_weight.unwrap_or(0.8),
                input.precision_weight.unwrap_or(0.6),
                category_order,
                category_prompts,
                input.anti_cheat_prompt.as_deref(),
                input.reflection_prompt.as_deref(),
                model_type_to_str(input.primary_model.unwrap_or(ModelType::Local)),
                model_type_to_str(input.reflection_model.unwrap_or(ModelType::Local)),
                input.min_comprehensiveness_score.unwrap_or(7.0),
                input.target_comprehensiveness_score.unwrap_or(8.5),
                hash,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_config_by_id(id)?
            .ok_or_else(|| TestGenError::ConfigNotFound(format!("Config {} not found after insert", id)))
    }

    /// Get config by ID
    pub fn get_config_by_id(&self, id: i64) -> Result<Option<TestGenConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_configs WHERE id = ?1")?;

        let result = stmt.query_row(params![id], row_to_config);
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get config by hash
    pub fn get_config_by_hash(&self, hash: &str) -> Result<Option<TestGenConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_configs WHERE config_hash = ?1")?;

        let result = stmt.query_row(params![hash], row_to_config);
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get current config (optionally for a task type)
    pub fn get_current_config(&self, task_type: Option<&str>) -> Result<Option<TestGenConfig>> {
        // First check task-specific best config
        if let Some(tt) = task_type {
            if tt != "_global_" {
                let mut stmt = self
                    .conn
                    .prepare("SELECT config_id FROM testgen_best_configs WHERE task_type = ?1")?;

                if let Ok(config_id) = stmt.query_row(params![tt], |row| row.get::<_, i64>(0)) {
                    if let Some(config) = self.get_config_by_id(config_id)? {
                        return Ok(Some(config));
                    }
                }
            }
        }

        // Fall back to global current config
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_configs WHERE is_current = 1")?;

        let result = stmt.query_row([], row_to_config);
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set current config
    pub fn set_current_config(&self, config_id: i64) -> Result<()> {
        self.conn
            .execute("UPDATE testgen_configs SET is_current = 0", [])?;
        self.conn.execute(
            "UPDATE testgen_configs SET is_current = 1 WHERE id = ?1",
            params![config_id],
        )?;
        Ok(())
    }

    /// Get all configs
    pub fn get_all_configs(&self) -> Result<Vec<TestGenConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_configs ORDER BY created_at DESC")?;

        let rows = stmt.query_map([], row_to_config)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TestGenError::from)
    }

    // ========================================================================
    // Run Operations
    // ========================================================================

    /// Save a run
    pub fn save_run(&self, input: &TestGenRunInput) -> Result<TestGenRun> {
        self.conn.execute(
            r#"INSERT INTO testgen_runs (
                run_id, session_id, config_id, task_id,
                total_tests, comprehensiveness_score, duration_ms, total_tokens,
                category_balance, anti_cheat_coverage, parameter_discovery,
                reflection_effectiveness, token_efficiency,
                meta_model, proposed_change, change_accepted, score, is_best
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, 0)"#,
            params![
                input.run_id,
                input.session_id,
                input.config_id,
                input.task_id,
                input.total_tests,
                input.comprehensiveness_score,
                input.duration_ms as i64,
                input.total_tokens,
                input.category_balance,
                input.anti_cheat_coverage,
                input.parameter_discovery,
                input.reflection_effectiveness,
                input.token_efficiency,
                input.meta_model,
                input.proposed_change,
                input.change_accepted,
                input.score,
            ],
        )?;

        let id = self.conn.last_insert_rowid();
        self.get_run_by_id(id)
    }

    /// Get run by ID
    pub fn get_run_by_id(&self, id: i64) -> Result<TestGenRun> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_runs WHERE id = ?1")?;

        stmt.query_row(params![id], row_to_run)
            .map_err(|_| TestGenError::RunNotFound(id.to_string()))
    }

    /// Get run by run_id
    pub fn get_run_by_run_id(&self, run_id: &str) -> Result<Option<TestGenRun>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_runs WHERE run_id = ?1")?;

        let result = stmt.query_row(params![run_id], row_to_run);
        match result {
            Ok(run) => Ok(Some(run)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get recent runs
    pub fn get_recent_runs(&self, limit: u32) -> Result<Vec<TestGenRun>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_runs ORDER BY created_at DESC LIMIT ?1")?;

        let rows = stmt.query_map(params![limit], row_to_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TestGenError::from)
    }

    /// Get run history for a task
    pub fn get_run_history(&self, task_id: &str, limit: u32) -> Result<Vec<TestGenRun>> {
        let mut stmt = self.conn.prepare(
            "SELECT * FROM testgen_runs WHERE task_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![task_id, limit], row_to_run)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(TestGenError::from)
    }

    // ========================================================================
    // Best Config Operations
    // ========================================================================

    /// Get best config for a task type
    pub fn get_best_config(&self, task_type: &str) -> Result<Option<TestGenBestConfig>> {
        let mut stmt = self
            .conn
            .prepare("SELECT * FROM testgen_best_configs WHERE task_type = ?1")?;

        let result = stmt.query_row(params![task_type], row_to_best_config);
        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Update best config for a task type
    pub fn update_best_config(
        &self,
        task_type: &str,
        config_id: i64,
        run_id: i64,
        score: i32,
    ) -> Result<()> {
        self.conn.execute(
            r#"INSERT INTO testgen_best_configs (task_type, config_id, run_id, score, pass_count, total_runs, is_override)
            VALUES (?1, ?2, ?3, ?4, 1, 1, ?5)
            ON CONFLICT(task_type) DO UPDATE SET
                config_id = ?2,
                run_id = ?3,
                score = ?4,
                pass_count = pass_count + 1,
                total_runs = total_runs + 1,
                is_override = ?5,
                updated_at = datetime('now')
            WHERE score < ?4"#,
            params![task_type, config_id, run_id, score, task_type != "_global_"],
        )?;
        Ok(())
    }

    // ========================================================================
    // Evolution Operations
    // ========================================================================

    /// Save evolution record
    pub fn save_evolution(
        &self,
        from_config_id: Option<i64>,
        to_config_id: Option<i64>,
        changes: &serde_json::Value,
        reasoning: &str,
        expected_improvement: Option<&str>,
    ) -> Result<i64> {
        self.conn.execute(
            r#"INSERT INTO testgen_evolution (from_config_id, to_config_id, changes, reasoning, expected_improvement)
            VALUES (?1, ?2, ?3, ?4, ?5)"#,
            params![
                from_config_id,
                to_config_id,
                changes.to_string(),
                reasoning,
                expected_improvement,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Update evolution with actual results
    pub fn update_evolution_result(
        &self,
        id: i64,
        actual_improvement: f64,
        quality_delta: f64,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE testgen_evolution SET actual_improvement = ?1, quality_delta = ?2 WHERE id = ?3",
            params![actual_improvement, quality_delta, id],
        )?;
        Ok(())
    }

    // ========================================================================
    // Stats Operations
    // ========================================================================

    /// Get aggregate statistics
    pub fn get_stats(&self) -> Result<TestGenStats> {
        let mut stmt = self.conn.prepare(
            r#"SELECT
                COUNT(*) as total_runs,
                AVG(score) as average_score,
                MAX(score) as best_score,
                AVG(comprehensiveness_score) as avg_comprehensiveness,
                AVG(token_efficiency) as avg_token_efficiency
            FROM testgen_runs"#,
        )?;

        let (total_runs, average_score, best_score, avg_comp, avg_eff) =
            stmt.query_row([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<f64>>(1)?,
                    row.get::<_, Option<i32>>(2)?,
                    row.get::<_, Option<f64>>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                ))
            })?;

        let total_configs: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM testgen_configs", [], |r| r.get(0))?;

        let evolution_count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM testgen_evolution", [], |r| r.get(0))?;

        Ok(TestGenStats {
            total_runs: total_runs as u64,
            total_configs: total_configs as u64,
            average_score: average_score.unwrap_or(0.0),
            best_score: best_score.unwrap_or(0),
            average_comprehensiveness: avg_comp.unwrap_or(0.0),
            average_token_efficiency: avg_eff.unwrap_or(0.0),
            config_evolution_count: evolution_count as u64,
        })
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn hash_config(input: &TestGenConfigInput) -> String {
    let data = serde_json::json!({
        "temperature": input.temperature,
        "max_tokens": input.max_tokens,
        "min_tests_per_category": input.min_tests_per_category,
        "max_tests_per_category": input.max_tests_per_category,
        "max_rounds_per_category": input.max_rounds_per_category,
        "environment_weight": input.environment_weight,
        "anti_cheat_weight": input.anti_cheat_weight,
        "precision_weight": input.precision_weight,
        "category_order": input.category_order,
        "primary_model": input.primary_model,
        "reflection_model": input.reflection_model,
    });

    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..8]) // First 16 hex chars
}

fn model_type_to_str(m: ModelType) -> &'static str {
    match m {
        ModelType::Local => "local",
        ModelType::Claude => "claude",
    }
}

fn str_to_model_type(s: &str) -> ModelType {
    match s {
        "claude" => ModelType::Claude,
        _ => ModelType::Local,
    }
}

fn row_to_config(row: &Row) -> rusqlite::Result<TestGenConfig> {
    let category_order_str: String = row.get("category_order")?;
    let category_order: Vec<TestCategory> =
        serde_json::from_str::<Vec<String>>(&category_order_str)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| match s.as_str() {
                "anti_cheat" => Some(TestCategory::AntiCheat),
                "existence" => Some(TestCategory::Existence),
                "correctness" => Some(TestCategory::Correctness),
                "boundary" => Some(TestCategory::Boundary),
                "integration" => Some(TestCategory::Integration),
                "format" => Some(TestCategory::Format),
                "happy_path" => Some(TestCategory::HappyPath),
                "edge_case" => Some(TestCategory::EdgeCase),
                "invalid_input" => Some(TestCategory::InvalidInput),
                _ => None,
            })
            .collect();

    let category_prompts_str: Option<String> = row.get("category_prompts")?;
    let category_prompts: Option<HashMap<TestCategory, String>> =
        category_prompts_str.and_then(|s| serde_json::from_str(&s).ok());

    Ok(TestGenConfig {
        id: row.get("id")?,
        version: row.get("version")?,
        temperature: row.get("temperature")?,
        max_tokens: row.get("max_tokens")?,
        min_tests_per_category: row.get("min_tests_per_category")?,
        max_tests_per_category: row.get("max_tests_per_category")?,
        max_rounds_per_category: row.get("max_rounds_per_category")?,
        environment_weight: row.get("environment_weight")?,
        anti_cheat_weight: row.get("anti_cheat_weight")?,
        precision_weight: row.get("precision_weight")?,
        category_order,
        category_prompts,
        anti_cheat_prompt: row.get("anti_cheat_prompt")?,
        reflection_prompt: row.get("reflection_prompt")?,
        primary_model: str_to_model_type(row.get::<_, String>("primary_model")?.as_str()),
        reflection_model: str_to_model_type(row.get::<_, String>("reflection_model")?.as_str()),
        min_comprehensiveness_score: row.get("min_comprehensiveness_score")?,
        target_comprehensiveness_score: row.get("target_comprehensiveness_score")?,
        config_hash: row.get("config_hash")?,
        is_current: row.get::<_, i32>("is_current")? != 0,
        created_at: row.get("created_at")?,
    })
}

fn row_to_run(row: &Row) -> rusqlite::Result<TestGenRun> {
    Ok(TestGenRun {
        id: row.get("id")?,
        run_id: row.get("run_id")?,
        session_id: row.get("session_id")?,
        config_id: row.get("config_id")?,
        task_id: row.get("task_id")?,
        total_tests: row.get("total_tests")?,
        comprehensiveness_score: row.get("comprehensiveness_score")?,
        duration_ms: row.get::<_, i64>("duration_ms")? as u64,
        total_tokens: row.get("total_tokens")?,
        category_balance: row.get("category_balance")?,
        anti_cheat_coverage: row.get("anti_cheat_coverage")?,
        parameter_discovery: row.get("parameter_discovery")?,
        reflection_effectiveness: row.get("reflection_effectiveness")?,
        token_efficiency: row.get("token_efficiency")?,
        meta_model: row.get("meta_model")?,
        proposed_change: row.get("proposed_change")?,
        change_accepted: row.get::<_, i32>("change_accepted")? != 0,
        score: row.get("score")?,
        is_best: row.get::<_, i32>("is_best")? != 0,
        created_at: row.get("created_at")?,
    })
}

fn row_to_best_config(row: &Row) -> rusqlite::Result<TestGenBestConfig> {
    Ok(TestGenBestConfig {
        task_type: row.get("task_type")?,
        config_id: row.get("config_id")?,
        run_id: row.get("run_id")?,
        score: row.get("score")?,
        pass_count: row.get("pass_count")?,
        total_runs: row.get("total_runs")?,
        is_override: row.get::<_, i32>("is_override")? != 0,
        updated_at: row.get("updated_at")?,
    })
}

// ============================================================================
// Schema SQL
// ============================================================================

const SCHEMA_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS testgen_configs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL,
    temperature REAL NOT NULL DEFAULT 0.3,
    max_tokens INTEGER NOT NULL DEFAULT 2048,
    min_tests_per_category INTEGER NOT NULL DEFAULT 2,
    max_tests_per_category INTEGER NOT NULL DEFAULT 5,
    max_rounds_per_category INTEGER NOT NULL DEFAULT 3,
    environment_weight REAL NOT NULL DEFAULT 0.7,
    anti_cheat_weight REAL NOT NULL DEFAULT 0.8,
    precision_weight REAL NOT NULL DEFAULT 0.6,
    category_order JSON NOT NULL DEFAULT '["anti_cheat","existence","correctness","boundary","integration"]',
    category_prompts JSON,
    anti_cheat_prompt TEXT,
    reflection_prompt TEXT,
    primary_model TEXT NOT NULL DEFAULT 'local',
    reflection_model TEXT NOT NULL DEFAULT 'local',
    min_comprehensiveness_score REAL NOT NULL DEFAULT 7.0,
    target_comprehensiveness_score REAL NOT NULL DEFAULT 8.5,
    config_hash TEXT NOT NULL,
    is_current INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(config_hash)
);

CREATE TABLE IF NOT EXISTS testgen_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL UNIQUE,
    session_id TEXT NOT NULL,
    config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
    task_id TEXT NOT NULL,
    total_tests INTEGER NOT NULL,
    comprehensiveness_score REAL,
    duration_ms INTEGER NOT NULL,
    total_tokens INTEGER NOT NULL,
    category_balance REAL,
    anti_cheat_coverage REAL,
    parameter_discovery REAL,
    reflection_effectiveness REAL,
    token_efficiency REAL,
    meta_model TEXT,
    proposed_change TEXT,
    change_accepted INTEGER DEFAULT 0,
    score INTEGER NOT NULL,
    is_best INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS testgen_best_configs (
    task_type TEXT PRIMARY KEY,
    config_id INTEGER NOT NULL REFERENCES testgen_configs(id),
    run_id INTEGER NOT NULL REFERENCES testgen_runs(id),
    score INTEGER NOT NULL,
    pass_count INTEGER DEFAULT 0,
    total_runs INTEGER DEFAULT 0,
    is_override INTEGER DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS testgen_evolution (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    from_config_id INTEGER REFERENCES testgen_configs(id),
    to_config_id INTEGER REFERENCES testgen_configs(id),
    changes JSON NOT NULL,
    reasoning TEXT NOT NULL,
    expected_improvement TEXT,
    actual_improvement REAL,
    quality_delta REAL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_tg_configs_current ON testgen_configs(is_current) WHERE is_current = 1;
CREATE INDEX IF NOT EXISTS idx_tg_configs_hash ON testgen_configs(config_hash);
CREATE INDEX IF NOT EXISTS idx_tg_runs_task ON testgen_runs(task_id);
CREATE INDEX IF NOT EXISTS idx_tg_runs_config ON testgen_runs(config_id);
CREATE INDEX IF NOT EXISTS idx_tg_runs_score ON testgen_runs(score);
CREATE INDEX IF NOT EXISTS idx_tg_runs_created ON testgen_runs(created_at);
CREATE INDEX IF NOT EXISTS idx_tg_runs_session ON testgen_runs(session_id);
CREATE INDEX IF NOT EXISTS idx_tg_best_task_type ON testgen_best_configs(task_type);
CREATE INDEX IF NOT EXISTS idx_tg_evolution_from ON testgen_evolution(from_config_id);
CREATE INDEX IF NOT EXISTS idx_tg_evolution_to ON testgen_evolution(to_config_id);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_open_in_memory() {
        let store = TestGenStore::open_in_memory().unwrap();
        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_runs, 0);
        assert_eq!(stats.total_configs, 0);
    }

    #[test]
    fn test_config_roundtrip() {
        let store = TestGenStore::open_in_memory().unwrap();

        let input = TestGenConfigInput {
            version: Some("1.0.0".to_string()),
            temperature: Some(0.3),
            min_tests_per_category: Some(2),
            max_tests_per_category: Some(5),
            ..Default::default()
        };

        let saved = store.save_config(&input).unwrap();
        assert!(saved.id > 0);
        assert_eq!(saved.temperature, 0.3);
        assert_eq!(saved.min_tests_per_category, 2);

        let loaded = store.get_config_by_id(saved.id).unwrap().unwrap();
        assert_eq!(loaded.temperature, saved.temperature);
        assert_eq!(loaded.config_hash, saved.config_hash);
    }

    #[test]
    fn test_config_deduplication() {
        let store = TestGenStore::open_in_memory().unwrap();

        let input = TestGenConfigInput {
            version: Some("1.0.0".to_string()),
            temperature: Some(0.3),
            ..Default::default()
        };

        let first = store.save_config(&input).unwrap();
        let second = store.save_config(&input).unwrap();

        // Same hash should return same config
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn test_run_roundtrip() {
        let store = TestGenStore::open_in_memory().unwrap();

        // First create a config
        let config = store
            .save_config(&TestGenConfigInput::default())
            .unwrap();

        let run_input = TestGenRunInput {
            run_id: "tg-test-123".to_string(),
            session_id: "session-456".to_string(),
            config_id: config.id,
            task_id: "regex-log".to_string(),
            total_tests: 20,
            comprehensiveness_score: Some(8.5),
            duration_ms: 5000,
            total_tokens: 10000,
            category_balance: Some(0.8),
            anti_cheat_coverage: Some(0.9),
            parameter_discovery: Some(0.7),
            reflection_effectiveness: Some(0.6),
            token_efficiency: Some(0.5),
            meta_model: None,
            proposed_change: None,
            change_accepted: false,
            score: 750,
        };

        let saved = store.save_run(&run_input).unwrap();
        assert!(saved.id > 0);
        assert_eq!(saved.run_id, "tg-test-123");
        assert_eq!(saved.score, 750);

        let loaded = store.get_run_by_id(saved.id).unwrap();
        assert_eq!(loaded.total_tests, 20);
    }

    #[test]
    fn test_current_config() {
        let store = TestGenStore::open_in_memory().unwrap();

        let config = store
            .save_config(&TestGenConfigInput {
                version: Some("1.0.0".to_string()),
                ..Default::default()
            })
            .unwrap();

        // No current config initially
        assert!(store.get_current_config(None).unwrap().is_none());

        // Set current
        store.set_current_config(config.id).unwrap();

        // Now should find it
        let current = store.get_current_config(None).unwrap().unwrap();
        assert_eq!(current.id, config.id);
    }

    #[test]
    fn test_stats() {
        let store = TestGenStore::open_in_memory().unwrap();

        let config = store
            .save_config(&TestGenConfigInput::default())
            .unwrap();

        // Add a run
        store
            .save_run(&TestGenRunInput {
                run_id: "tg-test-1".to_string(),
                session_id: "session-1".to_string(),
                config_id: config.id,
                task_id: "task-1".to_string(),
                total_tests: 10,
                comprehensiveness_score: Some(8.0),
                duration_ms: 1000,
                total_tokens: 5000,
                category_balance: Some(0.8),
                anti_cheat_coverage: Some(0.9),
                parameter_discovery: Some(0.7),
                reflection_effectiveness: Some(0.6),
                token_efficiency: Some(0.5),
                meta_model: None,
                proposed_change: None,
                change_accepted: false,
                score: 700,
            })
            .unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_runs, 1);
        assert_eq!(stats.total_configs, 1);
        assert_eq!(stats.best_score, 700);
    }
}
