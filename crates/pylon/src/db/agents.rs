//! Agent state persistence for host mode

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use super::PylonDb;

/// Agent lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecycleState {
    Embryonic,
    Active,
    Dormant,
    Terminated,
}

impl LifecycleState {
    pub fn as_str(&self) -> &'static str {
        match self {
            LifecycleState::Embryonic => "embryonic",
            LifecycleState::Active => "active",
            LifecycleState::Dormant => "dormant",
            LifecycleState::Terminated => "terminated",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "embryonic" => Some(LifecycleState::Embryonic),
            "active" => Some(LifecycleState::Active),
            "dormant" => Some(LifecycleState::Dormant),
            "terminated" => Some(LifecycleState::Terminated),
            _ => None,
        }
    }
}

/// An agent record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub npub: String,
    pub name: String,
    pub lifecycle_state: LifecycleState,
    pub balance_sats: u64,
    pub tick_count: u64,
    pub last_tick_at: Option<u64>,
    pub memory_json: Option<String>,
    pub goals_json: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Tick history record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickRecord {
    pub id: i64,
    pub agent_npub: String,
    pub tick_number: u64,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub actions_json: Option<String>,
    pub cost_sats: Option<u64>,
    pub duration_ms: Option<u64>,
    pub created_at: u64,
}

impl PylonDb {
    /// Create or update an agent
    pub fn upsert_agent(&self, agent: &Agent) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "INSERT INTO agents (npub, name, lifecycle_state, balance_sats, tick_count, last_tick_at, memory_json, goals_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(npub) DO UPDATE SET
                name = excluded.name,
                lifecycle_state = excluded.lifecycle_state,
                balance_sats = excluded.balance_sats,
                tick_count = excluded.tick_count,
                last_tick_at = excluded.last_tick_at,
                memory_json = excluded.memory_json,
                goals_json = excluded.goals_json,
                updated_at = ?9",
            params![
                agent.npub,
                agent.name,
                agent.lifecycle_state.as_str(),
                agent.balance_sats as i64,
                agent.tick_count as i64,
                agent.last_tick_at.map(|v| v as i64),
                agent.memory_json,
                agent.goals_json,
                now,
            ],
        )?;
        Ok(())
    }

    /// Get an agent by npub
    pub fn get_agent(&self, npub: &str) -> anyhow::Result<Option<Agent>> {
        let agent = self
            .conn()
            .query_row(
                "SELECT npub, name, lifecycle_state, balance_sats, tick_count, last_tick_at,
                        memory_json, goals_json, created_at, updated_at
                 FROM agents WHERE npub = ?",
                [npub],
                |row| {
                    Ok(Agent {
                        npub: row.get(0)?,
                        name: row.get(1)?,
                        lifecycle_state: LifecycleState::from_str(&row.get::<_, String>(2)?)
                            .unwrap_or(LifecycleState::Embryonic),
                        balance_sats: row.get::<_, i64>(3)? as u64,
                        tick_count: row.get::<_, i64>(4)? as u64,
                        last_tick_at: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                        memory_json: row.get(6)?,
                        goals_json: row.get(7)?,
                        created_at: row.get::<_, i64>(8)? as u64,
                        updated_at: row.get::<_, i64>(9)? as u64,
                    })
                },
            )
            .optional()?;

        Ok(agent)
    }

    /// List agents by lifecycle state
    pub fn list_agents_by_state(&self, state: LifecycleState) -> anyhow::Result<Vec<Agent>> {
        let mut stmt = self.conn().prepare(
            "SELECT npub, name, lifecycle_state, balance_sats, tick_count, last_tick_at,
                    memory_json, goals_json, created_at, updated_at
             FROM agents WHERE lifecycle_state = ? ORDER BY updated_at DESC",
        )?;

        let agents = stmt
            .query_map([state.as_str()], |row| {
                Ok(Agent {
                    npub: row.get(0)?,
                    name: row.get(1)?,
                    lifecycle_state: LifecycleState::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(LifecycleState::Embryonic),
                    balance_sats: row.get::<_, i64>(3)? as u64,
                    tick_count: row.get::<_, i64>(4)? as u64,
                    last_tick_at: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    memory_json: row.get(6)?,
                    goals_json: row.get(7)?,
                    created_at: row.get::<_, i64>(8)? as u64,
                    updated_at: row.get::<_, i64>(9)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(agents)
    }

    /// List all agents
    pub fn list_all_agents(&self) -> anyhow::Result<Vec<Agent>> {
        let mut stmt = self.conn().prepare(
            "SELECT npub, name, lifecycle_state, balance_sats, tick_count, last_tick_at,
                    memory_json, goals_json, created_at, updated_at
             FROM agents ORDER BY updated_at DESC",
        )?;

        let agents = stmt
            .query_map([], |row| {
                Ok(Agent {
                    npub: row.get(0)?,
                    name: row.get(1)?,
                    lifecycle_state: LifecycleState::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or(LifecycleState::Embryonic),
                    balance_sats: row.get::<_, i64>(3)? as u64,
                    tick_count: row.get::<_, i64>(4)? as u64,
                    last_tick_at: row.get::<_, Option<i64>>(5)?.map(|v| v as u64),
                    memory_json: row.get(6)?,
                    goals_json: row.get(7)?,
                    created_at: row.get::<_, i64>(8)? as u64,
                    updated_at: row.get::<_, i64>(9)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(agents)
    }

    /// Update agent lifecycle state
    pub fn update_agent_state(&self, npub: &str, state: LifecycleState) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "UPDATE agents SET lifecycle_state = ?1, updated_at = ?2 WHERE npub = ?3",
            params![state.as_str(), now, npub],
        )?;
        Ok(())
    }

    /// Update agent balance
    pub fn update_agent_balance(&self, npub: &str, balance_sats: u64) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn().execute(
            "UPDATE agents SET balance_sats = ?1, updated_at = ?2 WHERE npub = ?3",
            params![balance_sats as i64, now, npub],
        )?;
        Ok(())
    }

    /// Record a tick execution
    pub fn record_tick(
        &self,
        agent_npub: &str,
        tick_number: u64,
        prompt_tokens: Option<u32>,
        completion_tokens: Option<u32>,
        actions_json: Option<&str>,
        cost_sats: Option<u64>,
        duration_ms: Option<u64>,
    ) -> anyhow::Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        // Insert tick record
        self.conn().execute(
            "INSERT INTO tick_history (agent_npub, tick_number, prompt_tokens, completion_tokens, actions_json, cost_sats, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                agent_npub,
                tick_number as i64,
                prompt_tokens.map(|v| v as i64),
                completion_tokens.map(|v| v as i64),
                actions_json,
                cost_sats.map(|v| v as i64),
                duration_ms.map(|v| v as i64),
                now,
            ],
        )?;

        // Update agent's tick count and last tick time
        self.conn().execute(
            "UPDATE agents SET tick_count = tick_count + 1, last_tick_at = ?1, updated_at = ?1 WHERE npub = ?2",
            params![now, agent_npub],
        )?;

        Ok(())
    }

    /// Get tick history for an agent
    pub fn get_tick_history(
        &self,
        agent_npub: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<TickRecord>> {
        let mut stmt = self.conn().prepare(
            "SELECT id, agent_npub, tick_number, prompt_tokens, completion_tokens, actions_json, cost_sats, duration_ms, created_at
             FROM tick_history
             WHERE agent_npub = ?
             ORDER BY tick_number DESC
             LIMIT ?",
        )?;

        let records = stmt
            .query_map(params![agent_npub, limit as i64], |row| {
                Ok(TickRecord {
                    id: row.get(0)?,
                    agent_npub: row.get(1)?,
                    tick_number: row.get::<_, i64>(2)? as u64,
                    prompt_tokens: row.get::<_, Option<i64>>(3)?.map(|v| v as u32),
                    completion_tokens: row.get::<_, Option<i64>>(4)?.map(|v| v as u32),
                    actions_json: row.get(5)?,
                    cost_sats: row.get::<_, Option<i64>>(6)?.map(|v| v as u64),
                    duration_ms: row.get::<_, Option<i64>>(7)?.map(|v| v as u64),
                    created_at: row.get::<_, i64>(8)? as u64,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// Delete an agent and its tick history
    pub fn delete_agent(&self, npub: &str) -> anyhow::Result<()> {
        // Delete tick history first (foreign key)
        self.conn()
            .execute("DELETE FROM tick_history WHERE agent_npub = ?", [npub])?;

        // Delete agent
        self.conn()
            .execute("DELETE FROM agents WHERE npub = ?", [npub])?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    #[test]
    fn test_upsert_and_get_agent() {
        let db = PylonDb::open_in_memory().unwrap();

        let agent = Agent {
            npub: "npub1test".to_string(),
            name: "TestAgent".to_string(),
            lifecycle_state: LifecycleState::Embryonic,
            balance_sats: 1000,
            tick_count: 0,
            last_tick_at: None,
            memory_json: None,
            goals_json: None,
            created_at: now(),
            updated_at: now(),
        };

        db.upsert_agent(&agent).unwrap();

        let retrieved = db.get_agent("npub1test").unwrap().unwrap();
        assert_eq!(retrieved.name, "TestAgent");
        assert_eq!(retrieved.lifecycle_state, LifecycleState::Embryonic);
        assert_eq!(retrieved.balance_sats, 1000);
    }

    #[test]
    fn test_update_state() {
        let db = PylonDb::open_in_memory().unwrap();

        let agent = Agent {
            npub: "npub1test2".to_string(),
            name: "TestAgent2".to_string(),
            lifecycle_state: LifecycleState::Embryonic,
            balance_sats: 0,
            tick_count: 0,
            last_tick_at: None,
            memory_json: None,
            goals_json: None,
            created_at: now(),
            updated_at: now(),
        };

        db.upsert_agent(&agent).unwrap();
        db.update_agent_state("npub1test2", LifecycleState::Active)
            .unwrap();

        let retrieved = db.get_agent("npub1test2").unwrap().unwrap();
        assert_eq!(retrieved.lifecycle_state, LifecycleState::Active);
    }

    #[test]
    fn test_record_tick() {
        let db = PylonDb::open_in_memory().unwrap();

        let agent = Agent {
            npub: "npub1test3".to_string(),
            name: "TestAgent3".to_string(),
            lifecycle_state: LifecycleState::Active,
            balance_sats: 100,
            tick_count: 0,
            last_tick_at: None,
            memory_json: None,
            goals_json: None,
            created_at: now(),
            updated_at: now(),
        };

        db.upsert_agent(&agent).unwrap();
        db.record_tick(
            "npub1test3",
            1,
            Some(100),
            Some(50),
            Some("[\"Post\"]"),
            Some(5),
            Some(1200),
        )
        .unwrap();

        let retrieved = db.get_agent("npub1test3").unwrap().unwrap();
        assert_eq!(retrieved.tick_count, 1);
        assert!(retrieved.last_tick_at.is_some());

        let history = db.get_tick_history("npub1test3", 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].tick_number, 1);
        assert_eq!(history[0].prompt_tokens, Some(100));
    }
}
