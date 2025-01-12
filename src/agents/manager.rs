use crate::agents::agent::{Agent, AgentInstance, InstanceStatus};
use sqlx::PgPool;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::json;
use sqlx::types::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

const DEFAULT_MEMORY_LIMIT: u64 = 512; // MB
const DEFAULT_CPU_LIMIT: f64 = 100.0; // Percentage

// ... [rest of the file stays the same] ...