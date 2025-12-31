//! Export sessions from PostgreSQL to .rlog files.
//!
//! This module provides functionality to export stored sessions
//! from the database to the Recorder format for analysis, training,
//! or backup purposes.

#[cfg(feature = "export")]
pub mod db {
    use anyhow::{Context, Result};
    use chrono::{DateTime, Utc};
    use sqlx::PgPool;
    use std::io::Write;
    use std::path::Path;
    use uuid::Uuid;

    /// Session metadata from database
    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct DbSession {
        pub id: Uuid,
        pub session_id: String,
        pub user_id: Uuid,
        pub project_id: Option<Uuid>,
        pub mode: String,
        pub model: Option<String>,
        pub repo: Option<String>,
        pub repo_sha: Option<String>,
        pub branch: Option<String>,
        pub dirty: Option<bool>,
        pub sandbox_id: Option<String>,
        pub runner: Option<String>,
        pub skills: serde_json::Value,
        pub mcp_servers: serde_json::Value,
        pub toolset: serde_json::Value,
        pub budget: Option<String>,
        pub duration: Option<String>,
        pub classification: Option<String>,
        pub status: String,
        pub total_input_tokens: i64,
        pub total_output_tokens: i64,
        pub total_cost_usd_cents: i32,
        pub total_tool_calls: i32,
        pub notes: Option<String>,
        pub extra: serde_json::Value,
        pub started_at: DateTime<Utc>,
        pub completed_at: Option<DateTime<Utc>>,
    }

    /// Session event from database
    #[derive(Debug, Clone, sqlx::FromRow)]
    pub struct DbSessionEvent {
        pub id: Uuid,
        pub line_number: i32,
        pub line_type: String,
        pub raw_content: String,
        pub content: Option<String>,
        pub result: Option<String>,
        pub call_id: Option<String>,
        pub step: Option<i32>,
        pub event_timestamp: Option<DateTime<Utc>>,
        pub latency_ms: Option<i32>,
        pub input_tokens: Option<i32>,
        pub output_tokens: Option<i32>,
        pub cost_usd_cents: Option<i32>,
        pub created_at: DateTime<Utc>,
    }

    /// Load a session by its database UUID
    pub async fn load_session(db_pool: &PgPool, session_uuid: Uuid) -> Result<DbSession> {
        sqlx::query_as::<_, DbSession>(
            r#"
            SELECT id, session_id, user_id, project_id, mode, model,
                   repo, repo_sha, branch, dirty, sandbox_id, runner,
                   skills, mcp_servers, toolset, budget, duration, classification,
                   status, total_input_tokens, total_output_tokens,
                   total_cost_usd_cents, total_tool_calls, notes, extra,
                   started_at, completed_at
            FROM sessions
            WHERE id = $1
            "#,
        )
        .bind(session_uuid)
        .fetch_one(db_pool)
        .await
        .context("Failed to load session")
    }

    /// Load a session by its human-readable ID (sess_YYYYMMDD_NNN)
    pub async fn load_session_by_id(db_pool: &PgPool, session_id: &str) -> Result<DbSession> {
        sqlx::query_as::<_, DbSession>(
            r#"
            SELECT id, session_id, user_id, project_id, mode, model,
                   repo, repo_sha, branch, dirty, sandbox_id, runner,
                   skills, mcp_servers, toolset, budget, duration, classification,
                   status, total_input_tokens, total_output_tokens,
                   total_cost_usd_cents, total_tool_calls, notes, extra,
                   started_at, completed_at
            FROM sessions
            WHERE session_id = $1
            "#,
        )
        .bind(session_id)
        .fetch_one(db_pool)
        .await
        .context("Failed to load session")
    }

    /// Load all events for a session
    pub async fn load_session_events(
        db_pool: &PgPool,
        session_uuid: Uuid,
    ) -> Result<Vec<DbSessionEvent>> {
        sqlx::query_as::<_, DbSessionEvent>(
            r#"
            SELECT id, line_number, line_type, raw_content,
                   content, result, call_id, step, event_timestamp,
                   latency_ms, input_tokens, output_tokens, cost_usd_cents,
                   created_at
            FROM session_events
            WHERE session_id = $1
            ORDER BY line_number
            "#,
        )
        .bind(session_uuid)
        .fetch_all(db_pool)
        .await
        .context("Failed to load session events")
    }

    /// Build YAML header from session metadata
    pub fn build_header(session: &DbSession) -> String {
        let mut header = String::new();
        header.push_str("---\n");
        header.push_str("format: rlog/1\n");
        header.push_str(&format!("id: {}\n", session.session_id));
        header.push_str(&format!("mode: {}\n", session.mode));

        if let Some(ref model) = session.model {
            header.push_str(&format!("model: {}\n", model));
        }

        // Repo context
        if let Some(ref repo) = session.repo {
            header.push_str(&format!("repo: {}\n", repo));
        }
        if let Some(ref sha) = session.repo_sha {
            header.push_str(&format!("repo_sha: {}\n", sha));
        }
        if let Some(ref branch) = session.branch {
            header.push_str(&format!("branch: {}\n", branch));
        }
        if let Some(dirty) = session.dirty {
            header.push_str(&format!("dirty: {}\n", dirty));
        }

        // Sandbox
        if let Some(ref sandbox) = session.sandbox_id {
            header.push_str(&format!("sandbox_id: {}\n", sandbox));
        }
        if let Some(ref runner) = session.runner {
            header.push_str(&format!("runner: {}\n", runner));
        }

        // Capabilities
        if let serde_json::Value::Array(ref skills) = session.skills
            && !skills.is_empty()
        {
            let skill_strs: Vec<String> = skills
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            header.push_str(&format!("skills: [{}]\n", skill_strs.join(", ")));
        }
        if let serde_json::Value::Array(ref mcp) = session.mcp_servers
            && !mcp.is_empty()
        {
            let mcp_strs: Vec<String> = mcp
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            header.push_str(&format!("mcp: [{}]\n", mcp_strs.join(", ")));
        }

        // Limits
        if let Some(ref budget) = session.budget {
            header.push_str(&format!("budget: {}\n", budget));
        }
        if let Some(ref duration) = session.duration {
            header.push_str(&format!("duration: {}\n", duration));
        }

        // Classification
        if let Some(ref class) = session.classification {
            header.push_str(&format!("classification: {}\n", class));
        }

        // Notes
        if let Some(ref notes) = session.notes {
            header.push_str(&format!("notes: \"{}\"\n", notes.replace('"', "\\\"")));
        }

        header.push_str("---\n");
        header
    }

    /// Export a session to a .rlog file
    pub async fn export_session(
        db_pool: &PgPool,
        session_uuid: Uuid,
        output_path: &Path,
    ) -> Result<()> {
        // Load session and events
        let session = load_session(db_pool, session_uuid).await?;
        let events = load_session_events(db_pool, session_uuid).await?;

        // Build output
        let mut output = build_header(&session);
        output.push('\n');

        // Add events (using raw_content which is already in Recorder format)
        for event in &events {
            output.push_str(&event.raw_content);
            output.push('\n');
        }

        // Add final metrics comment
        let duration_secs = session
            .completed_at
            .map(|end| (end - session.started_at).num_seconds())
            .unwrap_or(0);

        let cost_usd = session.total_cost_usd_cents as f64 / 100.0;

        output.push_str(&format!(
            "\n# tokens_in={} tokens_out={} tools={} duration={}s cost=${:.4}\n",
            session.total_input_tokens,
            session.total_output_tokens,
            session.total_tool_calls,
            duration_secs,
            cost_usd
        ));

        // Write to file
        let mut file = std::fs::File::create(output_path)
            .with_context(|| format!("Failed to create output file: {}", output_path.display()))?;
        file.write_all(output.as_bytes())
            .with_context(|| format!("Failed to write to file: {}", output_path.display()))?;

        Ok(())
    }

    /// Export all sessions for a user within a date range
    pub async fn export_sessions_for_user(
        db_pool: &PgPool,
        user_id: Uuid,
        from: Option<DateTime<Utc>>,
        to: Option<DateTime<Utc>>,
        output_dir: &Path,
    ) -> Result<Vec<std::path::PathBuf>> {
        // Build query with optional date filters
        let sessions: Vec<DbSession> = if let (Some(from), Some(to)) = (from, to) {
            sqlx::query_as::<_, DbSession>(
                r#"
                SELECT id, session_id, user_id, project_id, mode, model,
                       repo, repo_sha, branch, dirty, sandbox_id, runner,
                       skills, mcp_servers, toolset, budget, duration, classification,
                       status, total_input_tokens, total_output_tokens,
                       total_cost_usd_cents, total_tool_calls, notes, extra,
                       started_at, completed_at
                FROM sessions
                WHERE user_id = $1 AND started_at >= $2 AND started_at <= $3
                ORDER BY started_at
                "#,
            )
            .bind(user_id)
            .bind(from)
            .bind(to)
            .fetch_all(db_pool)
            .await?
        } else {
            sqlx::query_as::<_, DbSession>(
                r#"
                SELECT id, session_id, user_id, project_id, mode, model,
                       repo, repo_sha, branch, dirty, sandbox_id, runner,
                       skills, mcp_servers, toolset, budget, duration, classification,
                       status, total_input_tokens, total_output_tokens,
                       total_cost_usd_cents, total_tool_calls, notes, extra,
                       started_at, completed_at
                FROM sessions
                WHERE user_id = $1
                ORDER BY started_at
                "#,
            )
            .bind(user_id)
            .fetch_all(db_pool)
            .await?
        };

        // Create output directory if needed
        std::fs::create_dir_all(output_dir)?;

        let mut exported = Vec::new();

        for session in sessions {
            let filename = format!("{}.rlog", session.session_id);
            let output_path = output_dir.join(&filename);

            export_session(db_pool, session.id, &output_path).await?;
            exported.push(output_path);
        }

        Ok(exported)
    }

    /// List all sessions (for CLI)
    pub async fn list_sessions(
        db_pool: &PgPool,
        user_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<DbSession>> {
        if let Some(uid) = user_id {
            sqlx::query_as::<_, DbSession>(
                r#"
                SELECT id, session_id, user_id, project_id, mode, model,
                       repo, repo_sha, branch, dirty, sandbox_id, runner,
                       skills, mcp_servers, toolset, budget, duration, classification,
                       status, total_input_tokens, total_output_tokens,
                       total_cost_usd_cents, total_tool_calls, notes, extra,
                       started_at, completed_at
                FROM sessions
                WHERE user_id = $1
                ORDER BY started_at DESC
                LIMIT $2
                "#,
            )
            .bind(uid)
            .bind(limit)
            .fetch_all(db_pool)
            .await
            .context("Failed to list sessions")
        } else {
            sqlx::query_as::<_, DbSession>(
                r#"
                SELECT id, session_id, user_id, project_id, mode, model,
                       repo, repo_sha, branch, dirty, sandbox_id, runner,
                       skills, mcp_servers, toolset, budget, duration, classification,
                       status, total_input_tokens, total_output_tokens,
                       total_cost_usd_cents, total_tool_calls, notes, extra,
                       started_at, completed_at
                FROM sessions
                ORDER BY started_at DESC
                LIMIT $1
                "#,
            )
            .bind(limit)
            .fetch_all(db_pool)
            .await
            .context("Failed to list sessions")
        }
    }
}

// Re-export when feature is enabled
#[cfg(feature = "export")]
pub use db::*;
