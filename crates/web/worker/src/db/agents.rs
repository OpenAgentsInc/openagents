//! Agent database operations for D1

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

/// Agent record from D1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRecord {
    pub user_id: String,
    pub agent_id: i64,
    pub name: Option<String>,
    pub nostr_public_key: Option<String>,
    pub nostr_npub: Option<String>,
    pub status: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MaxIdRow {
    max_id: Option<i64>,
}

/// List agents for a user
pub async fn list_for_user(db: &D1Database, user_id: &str) -> Result<Vec<AgentRecord>> {
    db.prepare(
        "SELECT user_id, agent_id, name, nostr_public_key, nostr_npub,
                status, created_at, updated_at
         FROM agents
         WHERE user_id = ? AND deleted_at IS NULL
         ORDER BY agent_id",
    )
    .bind(&[user_id.into()])?
    .all()
    .await
    .and_then(|result| result.results::<AgentRecord>())
}

/// Get agent by ID for a user
pub async fn get_by_id(
    db: &D1Database,
    user_id: &str,
    agent_id: i64,
) -> Result<Option<AgentRecord>> {
    db.prepare(
        "SELECT user_id, agent_id, name, nostr_public_key, nostr_npub,
                status, created_at, updated_at
         FROM agents
         WHERE user_id = ? AND agent_id = ? AND deleted_at IS NULL",
    )
    .bind(&[user_id.into(), agent_id.into()])?
    .first::<AgentRecord>(None)
    .await
}

/// Create a new agent for a user
pub async fn create(db: &D1Database, user_id: &str, name: Option<&str>) -> Result<AgentRecord> {
    let now = chrono::Utc::now().to_rfc3339();

    let max_row = db
        .prepare("SELECT MAX(agent_id) AS max_id FROM agents WHERE user_id = ?")
        .bind(&[user_id.into()])?
        .first::<MaxIdRow>(None)
        .await?
        .unwrap_or(MaxIdRow { max_id: None });

    let next_id = max_row.max_id.unwrap_or(-1) + 1;

    db.prepare(
        "INSERT INTO agents (
            user_id, agent_id, name, status, created_at, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&[
        user_id.into(),
        next_id.into(),
        name.map(|n| n.into()).unwrap_or(JsValue::NULL),
        "active".into(),
        now.clone().into(),
        now.into(),
    ])?
    .run()
    .await?;

    get_by_id(db, user_id, next_id)
        .await?
        .ok_or_else(|| Error::RustError("Agent not found after insert".to_string()))
}

/// Soft delete an agent
pub async fn soft_delete(db: &D1Database, user_id: &str, agent_id: i64) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();

    db.prepare(
        "UPDATE agents SET
            deleted_at = ?,
            status = ?,
            updated_at = ?
         WHERE user_id = ? AND agent_id = ? AND deleted_at IS NULL",
    )
    .bind(&[
        now.clone().into(),
        "deleted".into(),
        now.into(),
        user_id.into(),
        agent_id.into(),
    ])?
    .run()
    .await?;

    Ok(())
}
