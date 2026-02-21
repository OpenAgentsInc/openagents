use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanoutMessage {
    pub topic: String,
    pub sequence: u64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub published_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExternalFanoutHook {
    pub backend: String,
    pub status: String,
    pub note: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FanoutError {
    #[error("invalid fanout topic")]
    InvalidTopic,
}

#[async_trait]
pub trait FanoutDriver: Send + Sync {
    async fn publish(&self, topic: &str, message: FanoutMessage) -> Result<(), FanoutError>;
    async fn poll(
        &self,
        topic: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<FanoutMessage>, FanoutError>;
    fn driver_name(&self) -> &'static str;
    fn external_hooks(&self) -> Vec<ExternalFanoutHook>;
}

#[derive(Clone)]
pub struct FanoutHub {
    driver: Arc<dyn FanoutDriver>,
}

impl FanoutHub {
    #[must_use]
    pub fn memory(capacity: usize) -> Self {
        Self {
            driver: Arc::new(MemoryFanoutDriver::new(capacity)),
        }
    }

    pub async fn publish(&self, topic: &str, message: FanoutMessage) -> Result<(), FanoutError> {
        self.driver.publish(topic, message).await
    }

    pub async fn poll(
        &self,
        topic: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<FanoutMessage>, FanoutError> {
        self.driver.poll(topic, after_sequence, limit).await
    }

    #[must_use]
    pub fn driver_name(&self) -> &'static str {
        self.driver.driver_name()
    }

    #[must_use]
    pub fn external_hooks(&self) -> Vec<ExternalFanoutHook> {
        self.driver.external_hooks()
    }
}

struct MemoryFanoutDriver {
    topics: RwLock<HashMap<String, TopicQueue>>,
    capacity: usize,
}

struct TopicQueue {
    messages: VecDeque<FanoutMessage>,
}

impl MemoryFanoutDriver {
    fn new(capacity: usize) -> Self {
        Self {
            topics: RwLock::new(HashMap::new()),
            capacity: capacity.max(1),
        }
    }
}

#[async_trait]
impl FanoutDriver for MemoryFanoutDriver {
    async fn publish(&self, topic: &str, mut message: FanoutMessage) -> Result<(), FanoutError> {
        let normalized = topic.trim();
        if normalized.is_empty() {
            return Err(FanoutError::InvalidTopic);
        }
        message.topic = normalized.to_string();
        let mut topics = self.topics.write().await;
        let queue = topics
            .entry(normalized.to_string())
            .or_insert_with(|| TopicQueue {
                messages: VecDeque::new(),
            });
        queue.messages.push_back(message);
        while queue.messages.len() > self.capacity {
            let _ = queue.messages.pop_front();
        }
        Ok(())
    }

    async fn poll(
        &self,
        topic: &str,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<FanoutMessage>, FanoutError> {
        let normalized = topic.trim();
        if normalized.is_empty() {
            return Err(FanoutError::InvalidTopic);
        }
        let effective_limit = limit.max(1).min(500);
        let topics = self.topics.read().await;
        let messages = topics.get(normalized).map_or_else(Vec::new, |queue| {
            queue
                .messages
                .iter()
                .filter(|message| message.sequence > after_sequence)
                .take(effective_limit)
                .cloned()
                .collect()
        });
        Ok(messages)
    }

    fn driver_name(&self) -> &'static str {
        "memory"
    }

    fn external_hooks(&self) -> Vec<ExternalFanoutHook> {
        vec![
            ExternalFanoutHook {
                backend: "nats".to_string(),
                status: "not_configured".to_string(),
                note: "reserved seam for future broker-backed fanout driver".to_string(),
            },
            ExternalFanoutHook {
                backend: "redis".to_string(),
                status: "not_configured".to_string(),
                note: "reserved seam for future broker-backed fanout driver".to_string(),
            },
            ExternalFanoutHook {
                backend: "postgres_notify".to_string(),
                status: "not_configured".to_string(),
                note: "reserved seam for future broker-backed fanout driver".to_string(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use serde_json::json;

    use super::{FanoutHub, FanoutMessage};

    #[tokio::test]
    async fn memory_fanout_preserves_order() -> Result<()> {
        let fanout = FanoutHub::memory(16);
        for seq in 1..=3 {
            fanout
                .publish(
                    "run:1:events",
                    FanoutMessage {
                        topic: String::new(),
                        sequence: seq,
                        kind: "run.step".to_string(),
                        payload: json!({"seq": seq}),
                        published_at: chrono::Utc::now(),
                    },
                )
                .await?;
        }

        let messages = fanout.poll("run:1:events", 0, 10).await?;
        let observed = messages
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>();
        if observed != vec![1, 2, 3] {
            return Err(anyhow!("unexpected order: {:?}", observed));
        }
        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_enforces_bounded_queue() -> Result<()> {
        let fanout = FanoutHub::memory(2);
        for seq in 1..=3 {
            fanout
                .publish(
                    "run:2:events",
                    FanoutMessage {
                        topic: String::new(),
                        sequence: seq,
                        kind: "run.step".to_string(),
                        payload: json!({"seq": seq}),
                        published_at: chrono::Utc::now(),
                    },
                )
                .await?;
        }

        let messages = fanout.poll("run:2:events", 0, 10).await?;
        let observed = messages
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>();
        if observed != vec![2, 3] {
            return Err(anyhow!("expected bounded queue to keep newest entries"));
        }
        Ok(())
    }

    #[test]
    fn memory_fanout_exposes_external_driver_hooks() {
        let fanout = FanoutHub::memory(4);
        let hooks = fanout.external_hooks();
        assert_eq!(hooks.len(), 3);
    }
}
