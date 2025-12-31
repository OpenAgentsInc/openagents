//! Autopilot Agent Durable Object
//!
//! Provides persistent memory and lifecycle management for an agent instance.

use serde::{Deserialize, Serialize};
use worker::*;

#[durable_object]
pub struct AgentDo {
    state: State,
    #[allow(dead_code)]
    env: Env,
}

impl DurableObject for AgentDo {
    fn new(state: State, env: Env) -> Self {
        let sql = state.storage().sql();
        if let Err(err) = init_schema(&sql) {
            console_log!("AgentDo schema init failed: {:?}", err);
        }

        Self { state, env }
    }

    async fn fetch(&self, mut req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        match (req.method(), path) {
            (Method::Post, "/init") => self.handle_init(&mut req).await,
            (Method::Get, "/status") => self.handle_status(),
            (Method::Post, "/tick") => self.handle_tick(),
            _ => Response::error("Not Found", 404),
        }
    }
}

impl AgentDo {
    async fn handle_init(&self, req: &mut Request) -> Result<Response> {
        let body = req.text().await?;
        let init: InitRequest = serde_json::from_str(&body)
            .map_err(|e| Error::RustError(format!("Invalid init payload: {}", e)))?;

        let now = now_ts();
        let sql = self.state.storage().sql();

        upsert_config(&sql, "user_id", &init.user_id, now)?;
        upsert_config(&sql, "agent_id", &init.agent_id.to_string(), now)?;
        if let Some(name) = init.name.as_ref() {
            upsert_config(&sql, "name", name, now)?;
        }
        upsert_config(&sql, "initialized_at", &now.to_string(), now)?;

        Response::from_json(&serde_json::json!({
            "ok": true,
            "initialized_at": now,
        }))
    }

    fn handle_status(&self) -> Result<Response> {
        let sql = self.state.storage().sql();
        let config = load_config(&sql)?;
        let initialized = config.user_id.is_some() && config.agent_id.is_some();

        Response::from_json(&AgentStatusResponse {
            initialized,
            user_id: config.user_id,
            agent_id: config.agent_id,
            name: config.name,
            last_tick_at: config.last_tick_at,
            status: "active".to_string(),
        })
    }

    fn handle_tick(&self) -> Result<Response> {
        let now = now_ts();
        let sql = self.state.storage().sql();
        upsert_config(&sql, "last_tick_at", &now.to_string(), now)?;

        Response::from_json(&serde_json::json!({
            "ok": true,
            "timestamp": now,
        }))
    }
}

#[derive(Debug, Deserialize)]
struct InitRequest {
    user_id: String,
    agent_id: i64,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct AgentStatusResponse {
    initialized: bool,
    user_id: Option<String>,
    agent_id: Option<i64>,
    name: Option<String>,
    last_tick_at: Option<i64>,
    status: String,
}

#[derive(Debug, Default)]
struct AgentConfig {
    user_id: Option<String>,
    agent_id: Option<i64>,
    name: Option<String>,
    last_tick_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ConfigRow {
    key: String,
    value: String,
}

fn init_schema(sql: &SqlStorage) -> Result<()> {
    let statements = [
        "CREATE TABLE IF NOT EXISTS agent_config (key TEXT PRIMARY KEY, value TEXT, updated_at INTEGER);",
        "CREATE TABLE IF NOT EXISTS conversations (id TEXT PRIMARY KEY, started_at INTEGER, summary TEXT, tokens_used INTEGER);",
        "CREATE TABLE IF NOT EXISTS messages (id TEXT PRIMARY KEY, conversation_id TEXT, role TEXT, content TEXT, created_at INTEGER);",
        "CREATE TABLE IF NOT EXISTS file_context (path TEXT PRIMARY KEY, content_hash TEXT, last_read_at INTEGER, summary TEXT);",
        "CREATE TABLE IF NOT EXISTS learned_patterns (id TEXT PRIMARY KEY, pattern_type TEXT, description TEXT, confidence REAL);",
        "CREATE TABLE IF NOT EXISTS nostr_events (id TEXT PRIMARY KEY, pubkey TEXT, kind INTEGER, created_at INTEGER, content TEXT);",
        "CREATE TABLE IF NOT EXISTS goals (id TEXT PRIMARY KEY, description TEXT, priority INTEGER, status TEXT, progress REAL);",
        "CREATE TABLE IF NOT EXISTS agent_peers (npub TEXT PRIMARY KEY, name TEXT, relationship TEXT, trust_score REAL);",
    ];

    for statement in statements {
        sql.exec(statement, None)?;
    }

    Ok(())
}

fn upsert_config(sql: &SqlStorage, key: &str, value: &str, now: i64) -> Result<()> {
    let bindings = vec![key.into(), value.into(), now.into()];
    sql.exec(
        "INSERT INTO agent_config (key, value, updated_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        Some(bindings),
    )?;
    Ok(())
}

fn load_config(sql: &SqlStorage) -> Result<AgentConfig> {
    let cursor = sql.exec("SELECT key, value FROM agent_config", None)?;
    let rows: Vec<ConfigRow> = cursor.to_array()?;
    let mut config = AgentConfig::default();

    for row in rows {
        match row.key.as_str() {
            "user_id" => config.user_id = Some(row.value),
            "agent_id" => config.agent_id = row.value.parse::<i64>().ok(),
            "name" => config.name = Some(row.value),
            "last_tick_at" => config.last_tick_at = row.value.parse::<i64>().ok(),
            _ => {}
        }
    }

    Ok(config)
}

fn now_ts() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}
