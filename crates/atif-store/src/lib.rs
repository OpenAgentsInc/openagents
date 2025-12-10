//! # ATIF Store
//!
//! SQLite storage layer for ATIF (Agent Trajectory Interchange Format) trajectories.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use atif_store::TrajectoryStore;
//! use atif::{Agent, Step};
//!
//! let store = TrajectoryStore::new("trajectories.db")?;
//!
//! // Create a new trajectory
//! let agent = Agent { name: "commander".into(), version: "1.0".into(), model_name: Some("apple-fm".into()), extra: None };
//! let session_id = store.create_trajectory(&agent)?;
//!
//! // Add steps
//! let step = Step::user(1, "Hello, world!");
//! store.add_step(&session_id, &step)?;
//! ```

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

// Re-export atif types for convenience
pub use atif::{Agent, FinalMetrics, Metrics, Observation, Step, StepSource, ToolCall, Trajectory};

/// Errors from the trajectory store
#[derive(Error, Debug)]
pub enum StoreError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Trajectory not found: {0}")]
    NotFound(String),

    #[error("Invalid data: {0}")]
    InvalidData(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Metadata for a trajectory (for list views)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryMetadata {
    pub session_id: String,
    pub agent_name: String,
    pub agent_version: String,
    pub model_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: TrajectoryStatus,
    pub total_steps: i64,
}

/// Status of a trajectory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrajectoryStatus {
    InProgress,
    Completed,
    Failed,
}

impl TrajectoryStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::InProgress,
        }
    }
}

/// SQLite-based trajectory storage
pub struct TrajectoryStore {
    conn: Connection,
}

impl TrajectoryStore {
    /// Create a new trajectory store at the given path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Create an in-memory trajectory store (for testing)
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the database schema
    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS trajectories (
                session_id TEXT PRIMARY KEY,
                agent_name TEXT NOT NULL,
                agent_version TEXT NOT NULL,
                model_name TEXT,
                agent_extra_json TEXT,
                created_at TEXT NOT NULL,
                completed_at TEXT,
                status TEXT NOT NULL DEFAULT 'in_progress',
                total_steps INTEGER DEFAULT 0,
                final_metrics_json TEXT,
                notes TEXT,
                extra_json TEXT
            );

            CREATE TABLE IF NOT EXISTS steps (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trajectory_id TEXT NOT NULL REFERENCES trajectories(session_id) ON DELETE CASCADE,
                step_id INTEGER NOT NULL,
                timestamp TEXT,
                source TEXT NOT NULL,
                message TEXT NOT NULL,
                reasoning_content TEXT,
                reasoning_effort_json TEXT,
                model_name TEXT,
                tool_calls_json TEXT,
                observation_json TEXT,
                metrics_json TEXT,
                status TEXT,
                completed_at TEXT,
                error TEXT,
                extra_json TEXT,
                UNIQUE(trajectory_id, step_id)
            );

            CREATE INDEX IF NOT EXISTS idx_steps_trajectory ON steps(trajectory_id);
            CREATE INDEX IF NOT EXISTS idx_trajectories_created ON trajectories(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_trajectories_status ON trajectories(status);
            "#,
        )?;
        Ok(())
    }

    /// Generate a new session ID
    pub fn generate_session_id() -> String {
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%dT%H-%M-%S").to_string();
        let random = uuid::Uuid::new_v4().to_string()[..8].to_string();
        format!("session-{}-{}", timestamp, random)
    }

    /// Create a new trajectory and return its session ID
    pub fn create_trajectory(&self, agent: &Agent) -> Result<String> {
        let session_id = Self::generate_session_id();
        let now = Utc::now().to_rfc3339();

        let agent_extra_json = agent
            .extra
            .as_ref()
            .map(|e| serde_json::to_string(e))
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT INTO trajectories (session_id, agent_name, agent_version, model_name, agent_extra_json, created_at, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'in_progress')
            "#,
            params![
                session_id,
                agent.name,
                agent.version,
                agent.model_name,
                agent_extra_json,
                now,
            ],
        )?;

        Ok(session_id)
    }

    /// Add a step to an existing trajectory
    pub fn add_step(&self, session_id: &str, step: &Step) -> Result<()> {
        let timestamp = step.timestamp.map(|t| t.to_rfc3339());
        let source = match step.source {
            StepSource::User => "user",
            StepSource::Agent => "agent",
            StepSource::System => "system",
        };

        let reasoning_effort_json = step
            .reasoning_effort
            .as_ref()
            .map(|e| serde_json::to_string(e))
            .transpose()?;

        let tool_calls_json = step
            .tool_calls
            .as_ref()
            .map(|tc| serde_json::to_string(tc))
            .transpose()?;

        let observation_json = step
            .observation
            .as_ref()
            .map(|o| serde_json::to_string(o))
            .transpose()?;

        let metrics_json = step
            .metrics
            .as_ref()
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        let extra_json = step
            .extra
            .as_ref()
            .map(|e| serde_json::to_string(e))
            .transpose()?;

        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO steps (
                trajectory_id, step_id, timestamp, source, message,
                reasoning_content, reasoning_effort_json, model_name,
                tool_calls_json, observation_json, metrics_json, error, extra_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            "#,
            params![
                session_id,
                step.step_id,
                timestamp,
                source,
                step.message,
                step.reasoning_content,
                reasoning_effort_json,
                step.model_name,
                tool_calls_json,
                observation_json,
                metrics_json,
                None::<String>, // error field from Step doesn't exist in atif crate yet
                extra_json,
            ],
        )?;

        // Update step count
        self.conn.execute(
            "UPDATE trajectories SET total_steps = (SELECT COUNT(*) FROM steps WHERE trajectory_id = ?1) WHERE session_id = ?1",
            params![session_id],
        )?;

        Ok(())
    }

    /// Update the content of an existing step (for streaming updates)
    pub fn update_step_content(&self, session_id: &str, step_id: i64, content: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE steps SET message = ?3 WHERE trajectory_id = ?1 AND step_id = ?2",
            params![session_id, step_id, content],
        )?;
        Ok(())
    }

    /// Append content to an existing step (for streaming)
    pub fn append_to_step(&self, session_id: &str, step_id: i64, content: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE steps SET message = message || ?3 WHERE trajectory_id = ?1 AND step_id = ?2",
            params![session_id, step_id, content],
        )?;
        Ok(())
    }

    /// Get the full trajectory by session ID
    pub fn get_trajectory(&self, session_id: &str) -> Result<Trajectory> {
        // Get trajectory metadata
        let (agent_name, agent_version, model_name, agent_extra_json, notes, extra_json, final_metrics_json): (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        ) = self.conn.query_row(
            "SELECT agent_name, agent_version, model_name, agent_extra_json, notes, extra_json, final_metrics_json FROM trajectories WHERE session_id = ?1",
            params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
        ).map_err(|_| StoreError::NotFound(session_id.to_string()))?;

        let agent = Agent {
            name: agent_name,
            version: agent_version,
            model_name,
            extra: agent_extra_json
                .map(|s| serde_json::from_str(&s))
                .transpose()?,
        };

        // Get all steps
        let mut stmt = self.conn.prepare(
            r#"
            SELECT step_id, timestamp, source, message, reasoning_content, reasoning_effort_json,
                   model_name, tool_calls_json, observation_json, metrics_json, extra_json
            FROM steps
            WHERE trajectory_id = ?1
            ORDER BY step_id ASC
            "#,
        )?;

        let steps: Vec<Step> = stmt
            .query_map(params![session_id], |row| {
                let step_id: i64 = row.get(0)?;
                let timestamp_str: Option<String> = row.get(1)?;
                let source_str: String = row.get(2)?;
                let message: String = row.get(3)?;
                let reasoning_content: Option<String> = row.get(4)?;
                let reasoning_effort_json: Option<String> = row.get(5)?;
                let model_name: Option<String> = row.get(6)?;
                let tool_calls_json: Option<String> = row.get(7)?;
                let observation_json: Option<String> = row.get(8)?;
                let metrics_json: Option<String> = row.get(9)?;
                let extra_json: Option<String> = row.get(10)?;

                Ok((
                    step_id,
                    timestamp_str,
                    source_str,
                    message,
                    reasoning_content,
                    reasoning_effort_json,
                    model_name,
                    tool_calls_json,
                    observation_json,
                    metrics_json,
                    extra_json,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|row| {
                let source = match row.2.as_str() {
                    "user" => StepSource::User,
                    "agent" => StepSource::Agent,
                    _ => StepSource::System,
                };

                let timestamp = row.1.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|dt| dt.with_timezone(&Utc)));
                let reasoning_effort = row.5.and_then(|s| serde_json::from_str(&s).ok());
                let tool_calls = row.7.and_then(|s| serde_json::from_str(&s).ok());
                let observation = row.8.and_then(|s| serde_json::from_str(&s).ok());
                let metrics = row.9.and_then(|s| serde_json::from_str(&s).ok());
                let extra = row.10.and_then(|s| serde_json::from_str(&s).ok());

                Step {
                    step_id: row.0,
                    timestamp,
                    source,
                    message: row.3,
                    reasoning_content: row.4,
                    reasoning_effort,
                    model_name: row.6,
                    tool_calls,
                    observation,
                    metrics,
                    extra,
                }
            })
            .collect();

        let final_metrics = final_metrics_json
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        let extra = extra_json
            .map(|s| serde_json::from_str(&s))
            .transpose()?;

        Ok(Trajectory {
            schema_version: "ATIF-v1.4".to_string(),
            session_id: session_id.to_string(),
            agent,
            steps,
            notes,
            final_metrics,
            extra,
        })
    }

    /// Get the current step count for a trajectory
    pub fn get_step_count(&self, session_id: &str) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM steps WHERE trajectory_id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .map_err(|_| StoreError::NotFound(session_id.to_string()))?;
        Ok(count)
    }

    /// List trajectories with pagination
    pub fn list_trajectories(&self, limit: usize, offset: usize) -> Result<Vec<TrajectoryMetadata>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, agent_name, agent_version, model_name, created_at, completed_at, status, total_steps
            FROM trajectories
            ORDER BY created_at DESC
            LIMIT ?1 OFFSET ?2
            "#,
        )?;

        let trajectories = stmt
            .query_map(params![limit as i64, offset as i64], |row| {
                let created_at_str: String = row.get(4)?;
                let completed_at_str: Option<String> = row.get(5)?;
                let status_str: String = row.get(6)?;

                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    created_at_str,
                    completed_at_str,
                    status_str,
                    row.get::<_, i64>(7)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|row| {
                let created_at = DateTime::parse_from_rfc3339(&row.4)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let completed_at = row.5.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                });

                TrajectoryMetadata {
                    session_id: row.0,
                    agent_name: row.1,
                    agent_version: row.2,
                    model_name: row.3,
                    created_at,
                    completed_at,
                    status: TrajectoryStatus::from_str(&row.6),
                    total_steps: row.7,
                }
            })
            .collect();

        Ok(trajectories)
    }

    /// Get total trajectory count
    pub fn count_trajectories(&self) -> Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM trajectories", [], |row| row.get(0))?;
        Ok(count)
    }

    /// Mark a trajectory as completed
    pub fn complete_trajectory(
        &self,
        session_id: &str,
        final_metrics: Option<&FinalMetrics>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let metrics_json = final_metrics
            .map(|m| serde_json::to_string(m))
            .transpose()?;

        self.conn.execute(
            "UPDATE trajectories SET status = 'completed', completed_at = ?2, final_metrics_json = ?3 WHERE session_id = ?1",
            params![session_id, now, metrics_json],
        )?;

        Ok(())
    }

    /// Mark a trajectory as failed
    pub fn fail_trajectory(&self, session_id: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "UPDATE trajectories SET status = 'failed', completed_at = ?2 WHERE session_id = ?1",
            params![session_id, now],
        )?;
        Ok(())
    }

    /// Delete a trajectory and all its steps
    pub fn delete_trajectory(&self, session_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM trajectories WHERE session_id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    /// Search trajectories by agent name
    pub fn search_by_agent(&self, agent_name: &str, limit: usize) -> Result<Vec<TrajectoryMetadata>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT session_id, agent_name, agent_version, model_name, created_at, completed_at, status, total_steps
            FROM trajectories
            WHERE agent_name LIKE ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?;

        let pattern = format!("%{}%", agent_name);
        let trajectories = stmt
            .query_map(params![pattern, limit as i64], |row| {
                let created_at_str: String = row.get(4)?;
                let completed_at_str: Option<String> = row.get(5)?;
                let status_str: String = row.get(6)?;

                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    created_at_str,
                    completed_at_str,
                    status_str,
                    row.get::<_, i64>(7)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .map(|row| {
                let created_at = DateTime::parse_from_rfc3339(&row.4)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());
                let completed_at = row.5.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                });

                TrajectoryMetadata {
                    session_id: row.0,
                    agent_name: row.1,
                    agent_version: row.2,
                    model_name: row.3,
                    created_at,
                    completed_at,
                    status: TrajectoryStatus::from_str(&row.6),
                    total_steps: row.7,
                }
            })
            .collect();

        Ok(trajectories)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_trajectory() {
        let store = TrajectoryStore::in_memory().unwrap();

        let agent = Agent {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            model_name: Some("gpt-4".to_string()),
            extra: None,
        };

        let session_id = store.create_trajectory(&agent).unwrap();
        assert!(session_id.starts_with("session-"));

        // Add a user step
        let user_step = Step::user(1, "Hello, world!");
        store.add_step(&session_id, &user_step).unwrap();

        // Add an agent step
        let agent_step = Step::agent(2, "Hello! How can I help you?");
        store.add_step(&session_id, &agent_step).unwrap();

        // Get the trajectory
        let trajectory = store.get_trajectory(&session_id).unwrap();
        assert_eq!(trajectory.agent.name, "test-agent");
        assert_eq!(trajectory.steps.len(), 2);
        assert_eq!(trajectory.steps[0].message, "Hello, world!");
        assert_eq!(trajectory.steps[1].message, "Hello! How can I help you?");
    }

    #[test]
    fn test_list_trajectories() {
        let store = TrajectoryStore::in_memory().unwrap();

        let agent = Agent {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            model_name: None,
            extra: None,
        };

        // Create multiple trajectories
        for _ in 0..5 {
            store.create_trajectory(&agent).unwrap();
        }

        let list = store.list_trajectories(10, 0).unwrap();
        assert_eq!(list.len(), 5);

        let count = store.count_trajectories().unwrap();
        assert_eq!(count, 5);
    }

    #[test]
    fn test_complete_trajectory() {
        let store = TrajectoryStore::in_memory().unwrap();

        let agent = Agent {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            model_name: None,
            extra: None,
        };

        let session_id = store.create_trajectory(&agent).unwrap();

        let metrics = FinalMetrics {
            total_prompt_tokens: Some(100),
            total_completion_tokens: Some(50),
            total_cached_tokens: Some(10),
            total_cost_usd: Some(0.005),
            total_steps: Some(2),
            extra: None,
        };

        store.complete_trajectory(&session_id, Some(&metrics)).unwrap();

        let list = store.list_trajectories(10, 0).unwrap();
        assert_eq!(list[0].status, TrajectoryStatus::Completed);
    }

    #[test]
    fn test_append_to_step() {
        let store = TrajectoryStore::in_memory().unwrap();

        let agent = Agent {
            name: "test-agent".to_string(),
            version: "1.0.0".to_string(),
            model_name: None,
            extra: None,
        };

        let session_id = store.create_trajectory(&agent).unwrap();

        // Add an initial step with empty content
        let step = Step::agent(1, "");
        store.add_step(&session_id, &step).unwrap();

        // Append to it (simulating streaming)
        store.append_to_step(&session_id, 1, "Hello").unwrap();
        store.append_to_step(&session_id, 1, ", ").unwrap();
        store.append_to_step(&session_id, 1, "world!").unwrap();

        // Verify
        let trajectory = store.get_trajectory(&session_id).unwrap();
        assert_eq!(trajectory.steps[0].message, "Hello, world!");
    }
}
