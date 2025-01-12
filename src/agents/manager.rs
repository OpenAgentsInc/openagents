use crate::agents::agent::{Agent, AgentInstance, InstanceStatus};
use sqlx::PgPool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::json;
use sqlx::types::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const MAX_RETRIES: u32 = 3;
const DEFAULT_MEMORY_LIMIT: u64 = 512; // MB
const DEFAULT_CPU_LIMIT: f64 = 100.0; // Percentage

pub struct AgentManager {
    pool: PgPool,
    instances: Arc<RwLock<HashMap<Uuid, AgentInstance>>>,
    instance_states: Arc<RwLock<HashMap<Uuid, serde_json::Value>>>,
    instance_metrics: Arc<RwLock<HashMap<Uuid, serde_json::Value>>>,
}

impl AgentManager {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            instances: Arc::new(RwLock::new(HashMap::new())),
            instance_states: Arc::new(RwLock::new(HashMap::new())),
            instance_metrics: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // Agent Management
    pub async fn create_agent(
        &self,
        name: &str,
        description: &str,
        pubkey: &str,
        config: serde_json::Value,
    ) -> Result<Agent> {
        // Validate pubkey
        if pubkey.len() != 64 {
            return Err(anyhow!("Invalid pubkey length"));
        }

        // Validate and normalize config
        let config = self.validate_agent_config(config)?;

        let agent = Agent {
            id: Uuid::new_v4(),
            name: name.to_string(),
            description: description.to_string(),
            pubkey: pubkey.to_string(),
            enabled: true,
            config,
            created_at: Utc::now().timestamp(),
        };

        sqlx::query!(
            r#"
            INSERT INTO agents (id, name, description, pubkey, enabled, config, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
            agent.id,
            agent.name,
            agent.description,
            agent.pubkey,
            agent.enabled,
            agent.config as serde_json::Value
        )
        .execute(&self.pool)
        .await?;

        Ok(agent)
    }

    pub async fn get_agent(&self, id: Uuid) -> Result<Agent> {
        let record = sqlx::query!(
            r#"
            SELECT id, name, description, pubkey, enabled, config, created_at
            FROM agents WHERE id = $1
            "#,
            id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(Agent {
            id: record.id,
            name: record.name,
            description: record.description,
            pubkey: record.pubkey,
            enabled: record.enabled,
            config: record.config,
            created_at: record.created_at.timestamp(),
        })
    }

    // Instance Management
    pub async fn create_instance(&self, agent_id: Uuid) -> Result<AgentInstance> {
        // Get agent and validate
        let agent = self.get_agent(agent_id).await?;
        if !agent.enabled {
            return Err(anyhow!("Agent is disabled"));
        }

        // Check instance limits
        let instance_count = sqlx::query!(
            "SELECT COUNT(*) as count FROM agent_instances WHERE agent_id = $1 AND status != 'Stopped'",
            agent_id
        )
        .fetch_one(&self.pool)
        .await?;

        let max_instances = agent.config["max_instances"]
            .as_u64()
            .unwrap_or(1) as i64;

        if instance_count.count.unwrap_or(0) >= max_instances {
            return Err(anyhow!("Maximum instance limit reached"));
        }

        // Create instance
        let instance = AgentInstance {
            id: Uuid::new_v4(),
            agent_id,
            status: InstanceStatus::Starting,
            created_at: Utc::now().timestamp(),
            ended_at: None,
        };

        sqlx::query!(
            r#"
            INSERT INTO agent_instances (id, agent_id, status, created_at)
            VALUES ($1, $2, $3, NOW())
            "#,
            instance.id,
            instance.agent_id,
            format!("{:?}", instance.status),
        )
        .execute(&self.pool)
        .await?;

        // Initialize instance state if specified
        if let Some(initial_state) = agent.config.get("initial_state") {
            self.set_instance_state(instance.id, initial_state.clone()).await?;
        }

        // Initialize metrics
        self.update_instance_metrics(
            instance.id,
            json!({
                "memory_usage": 0,
                "cpu_usage": 0.0,
                "task_count": 0,
                "error_count": 0,
                "uptime": 0
            }),
        )
        .await?;

        // Cache instance
        self.instances.write().await.insert(instance.id, instance.clone());

        Ok(instance)
    }

    pub async fn update_instance_status(
        &self,
        instance_id: Uuid,
        status: InstanceStatus,
    ) -> Result<bool> {
        // Validate instance exists
        let instance = sqlx::query!(
            "SELECT id FROM agent_instances WHERE id = $1",
            instance_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if instance.is_none() {
            return Ok(false);
        }

        // Update status
        sqlx::query!(
            r#"
            UPDATE agent_instances 
            SET status = $1,
                ended_at = CASE WHEN $1 IN ('Stopped', 'Error') THEN NOW() ELSE NULL END
            WHERE id = $2
            "#,
            format!("{:?}", status),
            instance_id
        )
        .execute(&self.pool)
        .await?;

        // Update cache
        if let Some(instance) = self.instances.write().await.get_mut(&instance_id) {
            instance.status = status;
        }

        Ok(true)
    }

    // State Management
    pub async fn set_instance_state(
        &self,
        instance_id: Uuid,
        state: serde_json::Value,
    ) -> Result<bool> {
        // Validate instance exists
        let instance = sqlx::query!(
            "SELECT id FROM agent_instances WHERE id = $1",
            instance_id
        )
        .fetch_optional(&self.pool)
        .await?;

        if instance.is_none() {
            return Ok(false);
        }

        // Clear existing state
        sqlx::query!(
            "DELETE FROM agent_states WHERE instance_id = $1",
            instance_id
        )
        .execute(&self.pool)
        .await?;

        // Insert new state
        if let Some(obj) = state.as_object() {
            for (key, value) in obj {
                sqlx::query!(
                    r#"
                    INSERT INTO agent_states (instance_id, state_key, state_value)
                    VALUES ($1, $2, $3)
                    "#,
                    instance_id,
                    key,
                    value as serde_json::Value
                )
                .execute(&self.pool)
                .await?;
            }
        }

        // Update cache
        self.instance_states
            .write()
            .await
            .insert(instance_id, state);

        Ok(true)
    }

    pub async fn get_instance_state(&self, instance_id: Uuid) -> Result<Option<serde_json::Value>> {
        let records = sqlx::query!(
            r#"
            SELECT state_key, state_value
            FROM agent_states
            WHERE instance_id = $1
            "#,
            instance_id
        )
        .fetch_all(&self.pool)
        .await?;

        if records.is_empty() {
            return Ok(None);
        }

        let mut state = serde_json::Map::new();
        for record in records {
            state.insert(record.state_key, record.state_value);
        }

        Ok(Some(serde_json::Value::Object(state)))
    }

    // Resource Management
    pub async fn update_instance_metrics(
        &self,
        instance_id: Uuid,
        metrics: serde_json::Value,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO agent_metrics 
                (instance_id, memory_usage, cpu_usage, task_count, error_count, uptime)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            instance_id,
            metrics["memory_usage"].as_i64().unwrap_or(0) as i32,
            metrics["cpu_usage"].as_f64().unwrap_or(0.0),
            metrics["task_count"].as_i64().unwrap_or(0) as i32,
            metrics["error_count"].as_i64().unwrap_or(0) as i32,
            metrics["uptime"].as_i64().unwrap_or(0) as i32
        )
        .execute(&self.pool)
        .await?;

        // Update cache
        self.instance_metrics
            .write()
            .await
            .insert(instance_id, metrics);

        Ok(())
    }

    pub async fn check_resource_limits(&self, instance_id: Uuid) -> Result<bool> {
        let instance = match self.instances.read().await.get(&instance_id) {
            Some(i) => i.clone(),
            None => return Ok(false),
        };

        let agent = self.get_agent(instance.agent_id).await?;
        let metrics = self
            .instance_metrics
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .unwrap_or_else(|| json!({}));

        let memory_limit = agent.config["memory_limit"]
            .as_u64()
            .unwrap_or(DEFAULT_MEMORY_LIMIT);
        let cpu_limit = agent.config["cpu_limit"]
            .as_f64()
            .unwrap_or(DEFAULT_CPU_LIMIT);

        let memory_usage = metrics["memory_usage"].as_u64().unwrap_or(0);
        let cpu_usage = metrics["cpu_usage"].as_f64().unwrap_or(0.0);

        Ok(memory_usage <= memory_limit && cpu_usage <= cpu_limit)
    }

    // Helper Methods
    fn validate_agent_config(&self, config: serde_json::Value) -> Result<serde_json::Value> {
        let mut config = config.as_object().ok_or(anyhow!("Invalid config format"))?.clone();

        // Ensure required fields with defaults
        if !config.contains_key("version") {
            config.insert("version".into(), json!("1.0.0"));
        }
        if !config.contains_key("memory_limit") {
            config.insert("memory_limit".into(), json!(DEFAULT_MEMORY_LIMIT));
        }
        if !config.contains_key("cpu_limit") {
            config.insert("cpu_limit".into(), json!(DEFAULT_CPU_LIMIT));
        }
        if !config.contains_key("max_instances") {
            config.insert("max_instances".into(), json!(1));
        }

        // Validate limits
        let memory_limit = config["memory_limit"].as_u64().ok_or(anyhow!("Invalid memory_limit"))?;
        if memory_limit == 0 || memory_limit > 4096 {
            return Err(anyhow!("memory_limit must be between 1 and 4096"));
        }

        let cpu_limit = config["cpu_limit"].as_f64().ok_or(anyhow!("Invalid cpu_limit"))?;
        if cpu_limit <= 0.0 || cpu_limit > 400.0 {
            return Err(anyhow!("cpu_limit must be between 0 and 400"));
        }

        let max_instances = config["max_instances"].as_u64().ok_or(anyhow!("Invalid max_instances"))?;
        if max_instances == 0 || max_instances > 100 {
            return Err(anyhow!("max_instances must be between 1 and 100"));
        }

        Ok(serde_json::Value::Object(config))
    }
}