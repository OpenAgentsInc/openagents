use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FanoutTopicWindow {
    pub topic: String,
    pub topic_class: String,
    pub qos_tier: String,
    pub replay_budget_events: u64,
    pub oldest_sequence: u64,
    pub head_sequence: u64,
    pub queue_depth: usize,
    pub dropped_messages: u64,
    pub publish_rate_limit_per_second: u32,
    pub max_payload_bytes: usize,
    pub publish_rate_limited_count: u64,
    pub frame_size_rejected_count: u64,
    pub stale_cursor_budget_exceeded_count: u64,
    pub stale_cursor_retention_floor_count: u64,
    pub last_violation_reason: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QosTier {
    Hot,
    Warm,
    Cold,
}

impl QosTier {
    fn as_str(self) -> &'static str {
        match self {
            Self::Hot => "hot",
            Self::Warm => "warm",
            Self::Cold => "cold",
        }
    }
}

#[derive(Clone, Debug)]
pub struct FanoutTierLimits {
    pub qos_tier: QosTier,
    pub replay_budget_events: u64,
    pub max_publish_per_second: u32,
    pub max_payload_bytes: usize,
}

impl FanoutTierLimits {
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            qos_tier: self.qos_tier,
            replay_budget_events: self.replay_budget_events.max(1),
            max_publish_per_second: self.max_publish_per_second.max(1),
            max_payload_bytes: self.max_payload_bytes.max(1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FanoutLimitConfig {
    pub run_events: FanoutTierLimits,
    pub worker_lifecycle: FanoutTierLimits,
    pub codex_worker_events: FanoutTierLimits,
    pub fallback: FanoutTierLimits,
}

impl Default for FanoutLimitConfig {
    fn default() -> Self {
        Self {
            run_events: FanoutTierLimits {
                qos_tier: QosTier::Warm,
                replay_budget_events: 20_000,
                max_publish_per_second: 240,
                max_payload_bytes: 256 * 1024,
            },
            worker_lifecycle: FanoutTierLimits {
                qos_tier: QosTier::Warm,
                replay_budget_events: 10_000,
                max_publish_per_second: 180,
                max_payload_bytes: 64 * 1024,
            },
            codex_worker_events: FanoutTierLimits {
                qos_tier: QosTier::Hot,
                replay_budget_events: 3_000,
                max_publish_per_second: 240,
                max_payload_bytes: 128 * 1024,
            },
            fallback: FanoutTierLimits {
                qos_tier: QosTier::Cold,
                replay_budget_events: 500,
                max_publish_per_second: 90,
                max_payload_bytes: 64 * 1024,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TopicClass {
    RunEvents,
    WorkerLifecycle,
    CodexWorkerEvents,
    Other,
}

impl TopicClass {
    fn from_topic(topic: &str) -> Self {
        if topic == "runtime.codex_worker_events" {
            return Self::CodexWorkerEvents;
        }
        if topic.starts_with("run:") && topic.ends_with(":events") {
            return Self::RunEvents;
        }
        if topic.starts_with("worker:") && topic.ends_with(":lifecycle") {
            return Self::WorkerLifecycle;
        }
        Self::Other
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::RunEvents => "run_events",
            Self::WorkerLifecycle => "worker_lifecycle",
            Self::CodexWorkerEvents => "codex_worker_events",
            Self::Other => "fallback",
        }
    }
}

impl FanoutLimitConfig {
    fn limits_for_topic(&self, topic: &str) -> (TopicClass, FanoutTierLimits) {
        let topic_class = TopicClass::from_topic(topic);
        let limits = match topic_class {
            TopicClass::RunEvents => self.run_events.clone(),
            TopicClass::WorkerLifecycle => self.worker_lifecycle.clone(),
            TopicClass::CodexWorkerEvents => self.codex_worker_events.clone(),
            TopicClass::Other => self.fallback.clone(),
        }
        .normalized();
        (topic_class, limits)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FanoutError {
    #[error("invalid fanout topic")]
    InvalidTopic,
    #[error(
        "publish rate limited for topic {topic} ({topic_class}): limit={max_publish_per_second} retry_after_ms={retry_after_ms} reason={reason_code}"
    )]
    PublishRateLimited {
        topic: String,
        topic_class: String,
        reason_code: String,
        max_publish_per_second: u32,
        retry_after_ms: u64,
    },
    #[error(
        "frame payload too large for topic {topic} ({topic_class}): payload_bytes={payload_bytes} max_payload_bytes={max_payload_bytes} reason={reason_code}"
    )]
    FramePayloadTooLarge {
        topic: String,
        topic_class: String,
        reason_code: String,
        payload_bytes: usize,
        max_payload_bytes: usize,
    },
    #[error(
        "stale cursor for topic {topic}: requested={requested_cursor} oldest={oldest_available_cursor} head={head_cursor}"
    )]
    StaleCursor {
        topic: String,
        requested_cursor: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
        qos_tier: String,
    },
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
    async fn topic_window(&self, topic: &str) -> Result<Option<FanoutTopicWindow>, FanoutError>;
    async fn topic_windows(&self, limit: usize) -> Result<Vec<FanoutTopicWindow>, FanoutError>;
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
        Self::memory_with_limits(capacity, FanoutLimitConfig::default())
    }

    #[must_use]
    pub fn memory_with_limits(capacity: usize, limits: FanoutLimitConfig) -> Self {
        Self {
            driver: Arc::new(MemoryFanoutDriver::new(capacity, limits)),
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

    pub async fn topic_window(
        &self,
        topic: &str,
    ) -> Result<Option<FanoutTopicWindow>, FanoutError> {
        self.driver.topic_window(topic).await
    }

    pub async fn topic_windows(&self, limit: usize) -> Result<Vec<FanoutTopicWindow>, FanoutError> {
        self.driver.topic_windows(limit).await
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
    limits: FanoutLimitConfig,
}

struct TopicQueue {
    messages: VecDeque<FanoutMessage>,
    dropped_messages: u64,
    topic_class: TopicClass,
    qos_tier: QosTier,
    replay_budget_events: u64,
    max_publish_per_second: u32,
    max_payload_bytes: usize,
    publish_window_start: Instant,
    publish_window_count: u32,
    publish_rate_limited_count: u64,
    frame_size_rejected_count: u64,
    stale_cursor_budget_exceeded_count: u64,
    stale_cursor_retention_floor_count: u64,
    last_violation_reason: Option<String>,
}

impl MemoryFanoutDriver {
    fn new(capacity: usize, limits: FanoutLimitConfig) -> Self {
        Self {
            topics: RwLock::new(HashMap::new()),
            capacity: capacity.max(1),
            limits,
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
        let queue = topics.entry(normalized.to_string()).or_insert_with(|| {
            let (topic_class, limits) = self.limits.limits_for_topic(normalized);
            TopicQueue {
                messages: VecDeque::new(),
                dropped_messages: 0,
                topic_class,
                qos_tier: limits.qos_tier,
                replay_budget_events: limits.replay_budget_events,
                max_publish_per_second: limits.max_publish_per_second,
                max_payload_bytes: limits.max_payload_bytes,
                publish_window_start: Instant::now(),
                publish_window_count: 0,
                publish_rate_limited_count: 0,
                frame_size_rejected_count: 0,
                stale_cursor_budget_exceeded_count: 0,
                stale_cursor_retention_floor_count: 0,
                last_violation_reason: None,
            }
        });
        let payload_bytes = estimate_payload_bytes(&message.payload);
        if payload_bytes > queue.max_payload_bytes {
            queue.frame_size_rejected_count = queue.frame_size_rejected_count.saturating_add(1);
            let reason_code = "khala_frame_payload_too_large";
            queue.last_violation_reason = Some(reason_code.to_string());
            return Err(FanoutError::FramePayloadTooLarge {
                topic: normalized.to_string(),
                topic_class: queue.topic_class.as_str().to_string(),
                reason_code: reason_code.to_string(),
                payload_bytes,
                max_payload_bytes: queue.max_payload_bytes,
            });
        }
        let now = Instant::now();
        if now.duration_since(queue.publish_window_start) >= Duration::from_secs(1) {
            queue.publish_window_start = now;
            queue.publish_window_count = 0;
        }
        if queue.publish_window_count >= queue.max_publish_per_second {
            queue.publish_rate_limited_count = queue.publish_rate_limited_count.saturating_add(1);
            let reason_code = "khala_publish_rate_limited";
            queue.last_violation_reason = Some(reason_code.to_string());
            let elapsed = now.duration_since(queue.publish_window_start);
            let retry_after_ms = 1_000_u64.saturating_sub(elapsed.as_millis().min(1_000) as u64);
            return Err(FanoutError::PublishRateLimited {
                topic: normalized.to_string(),
                topic_class: queue.topic_class.as_str().to_string(),
                reason_code: reason_code.to_string(),
                max_publish_per_second: queue.max_publish_per_second,
                retry_after_ms,
            });
        }
        queue.publish_window_count = queue.publish_window_count.saturating_add(1);
        queue.messages.push_back(message);
        while queue.messages.len() > self.capacity {
            let _ = queue.messages.pop_front();
            queue.dropped_messages = queue.dropped_messages.saturating_add(1);
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
        let Some(queue) = topics.get(normalized) else {
            return Ok(Vec::new());
        };
        if let Some((oldest_sequence, head_sequence)) = queue_seq_bounds(queue) {
            let oldest_available_cursor = oldest_sequence.saturating_sub(1);
            let replay_lag = head_sequence.saturating_sub(after_sequence);
            let replay_budget_events = queue.replay_budget_events;
            let qos_tier = queue.qos_tier.as_str().to_string();
            let mut reason_codes = Vec::new();
            if after_sequence < oldest_available_cursor {
                reason_codes.push("retention_floor_breach".to_string());
            }
            if after_sequence < head_sequence && replay_lag > replay_budget_events {
                reason_codes.push("replay_budget_exceeded".to_string());
            }
            if !reason_codes.is_empty() {
                drop(topics);
                let mut topics = self.topics.write().await;
                if let Some(queue_mut) = topics.get_mut(normalized) {
                    for code in &reason_codes {
                        if code == "retention_floor_breach" {
                            queue_mut.stale_cursor_retention_floor_count = queue_mut
                                .stale_cursor_retention_floor_count
                                .saturating_add(1);
                        } else if code == "replay_budget_exceeded" {
                            queue_mut.stale_cursor_budget_exceeded_count = queue_mut
                                .stale_cursor_budget_exceeded_count
                                .saturating_add(1);
                        }
                    }
                    queue_mut.last_violation_reason = reason_codes.last().cloned();
                }
                return Err(FanoutError::StaleCursor {
                    topic: normalized.to_string(),
                    requested_cursor: after_sequence,
                    oldest_available_cursor,
                    head_cursor: head_sequence,
                    reason_codes,
                    replay_lag,
                    replay_budget_events,
                    qos_tier,
                });
            }
            if after_sequence > head_sequence {
                return Ok(Vec::new());
            }
        }
        let mut messages = queue
            .messages
            .iter()
            .filter(|message| message.sequence > after_sequence)
            .cloned()
            .collect::<Vec<_>>();
        // Transport delivery can arrive out-of-order in multi-node topologies; sort by
        // authoritative sequence so clients receive logical ordering by (topic, seq).
        messages.sort_by(|a, b| a.sequence.cmp(&b.sequence));
        messages.truncate(effective_limit);
        Ok(messages)
    }

    async fn topic_window(&self, topic: &str) -> Result<Option<FanoutTopicWindow>, FanoutError> {
        let normalized = topic.trim();
        if normalized.is_empty() {
            return Err(FanoutError::InvalidTopic);
        }

        let topics = self.topics.read().await;
        let Some(queue) = topics.get(normalized) else {
            return Ok(None);
        };
        let Some((oldest_sequence, head_sequence)) = queue_seq_bounds(queue) else {
            return Ok(None);
        };
        Ok(Some(FanoutTopicWindow {
            topic: normalized.to_string(),
            topic_class: queue.topic_class.as_str().to_string(),
            qos_tier: queue.qos_tier.as_str().to_string(),
            replay_budget_events: queue.replay_budget_events,
            oldest_sequence,
            head_sequence,
            queue_depth: queue.messages.len(),
            dropped_messages: queue.dropped_messages,
            publish_rate_limit_per_second: queue.max_publish_per_second,
            max_payload_bytes: queue.max_payload_bytes,
            publish_rate_limited_count: queue.publish_rate_limited_count,
            frame_size_rejected_count: queue.frame_size_rejected_count,
            stale_cursor_budget_exceeded_count: queue.stale_cursor_budget_exceeded_count,
            stale_cursor_retention_floor_count: queue.stale_cursor_retention_floor_count,
            last_violation_reason: queue.last_violation_reason.clone(),
        }))
    }

    async fn topic_windows(&self, limit: usize) -> Result<Vec<FanoutTopicWindow>, FanoutError> {
        let capped_limit = limit.max(1).min(200);
        let topics = self.topics.read().await;
        let mut windows = topics
            .iter()
            .filter_map(|(topic, queue)| {
                let (oldest_sequence, head_sequence) = queue_seq_bounds(queue)?;
                Some(FanoutTopicWindow {
                    topic: topic.clone(),
                    topic_class: queue.topic_class.as_str().to_string(),
                    qos_tier: queue.qos_tier.as_str().to_string(),
                    replay_budget_events: queue.replay_budget_events,
                    oldest_sequence,
                    head_sequence,
                    queue_depth: queue.messages.len(),
                    dropped_messages: queue.dropped_messages,
                    publish_rate_limit_per_second: queue.max_publish_per_second,
                    max_payload_bytes: queue.max_payload_bytes,
                    publish_rate_limited_count: queue.publish_rate_limited_count,
                    frame_size_rejected_count: queue.frame_size_rejected_count,
                    stale_cursor_budget_exceeded_count: queue.stale_cursor_budget_exceeded_count,
                    stale_cursor_retention_floor_count: queue.stale_cursor_retention_floor_count,
                    last_violation_reason: queue.last_violation_reason.clone(),
                })
            })
            .collect::<Vec<_>>();
        windows.sort_by(|a, b| b.head_sequence.cmp(&a.head_sequence));
        windows.truncate(capped_limit);
        Ok(windows)
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

fn estimate_payload_bytes(payload: &serde_json::Value) -> usize {
    serde_json::to_vec(payload).map_or(0, |bytes| bytes.len())
}

fn queue_seq_bounds(queue: &TopicQueue) -> Option<(u64, u64)> {
    let mut iter = queue.messages.iter();
    let first = iter.next()?;
    let mut min_seq = first.sequence;
    let mut max_seq = first.sequence;
    for message in iter {
        min_seq = min_seq.min(message.sequence);
        max_seq = max_seq.max(message.sequence);
    }
    Some((min_seq, max_seq))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, anyhow};
    use serde_json::json;

    use super::{
        FanoutError, FanoutHub, FanoutLimitConfig, FanoutMessage, FanoutTierLimits, QosTier,
    };

    fn tier_limits(
        max_publish_per_second: u32,
        max_payload_bytes: usize,
        replay_budget_events: u64,
    ) -> FanoutTierLimits {
        FanoutTierLimits {
            qos_tier: QosTier::Warm,
            replay_budget_events,
            max_publish_per_second,
            max_payload_bytes,
        }
    }

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

        let messages = fanout.poll("run:2:events", 1, 10).await?;
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

    #[tokio::test]
    async fn memory_fanout_rejects_stale_cursor() -> Result<()> {
        let fanout = FanoutHub::memory(2);
        for seq in 1..=4 {
            fanout
                .publish(
                    "run:stale:events",
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

        let stale = fanout.poll("run:stale:events", 0, 10).await;
        match stale {
            Err(FanoutError::StaleCursor {
                requested_cursor,
                oldest_available_cursor,
                reason_codes,
                replay_budget_events,
                qos_tier,
                ..
            }) => {
                assert_eq!(requested_cursor, 0);
                assert_eq!(oldest_available_cursor, 2);
                assert!(reason_codes.contains(&"retention_floor_breach".to_string()));
                assert_eq!(replay_budget_events, 20_000);
                assert_eq!(qos_tier, "warm");
            }
            other => return Err(anyhow!("expected stale cursor error, got {:?}", other)),
        }

        let boundary = fanout.poll("run:stale:events", 2, 10).await?;
        let observed = boundary
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>();
        if observed != vec![3, 4] {
            return Err(anyhow!(
                "unexpected replay boundary behavior: {:?}",
                observed
            ));
        }

        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_reports_topic_window() -> Result<()> {
        let fanout = FanoutHub::memory(3);
        for seq in 1..=5 {
            fanout
                .publish(
                    "run:window:events",
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

        let window = fanout
            .topic_window("run:window:events")
            .await?
            .ok_or_else(|| anyhow!("expected topic window"))?;
        assert_eq!(window.oldest_sequence, 3);
        assert_eq!(window.head_sequence, 5);
        assert_eq!(window.queue_depth, 3);
        assert_eq!(window.dropped_messages, 2);
        assert_eq!(window.qos_tier, "warm");
        assert_eq!(window.replay_budget_events, 20_000);

        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_topic_windows_reports_ranked_queue_state() -> Result<()> {
        let fanout = FanoutHub::memory(2);
        for seq in 1..=3 {
            fanout
                .publish(
                    "run:a:events",
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
        fanout
            .publish(
                "run:b:events",
                FanoutMessage {
                    topic: String::new(),
                    sequence: 1,
                    kind: "run.step".to_string(),
                    payload: json!({"seq": 1}),
                    published_at: chrono::Utc::now(),
                },
            )
            .await?;

        let windows = fanout.topic_windows(10).await?;
        assert!(!windows.is_empty());
        let first = windows
            .first()
            .ok_or_else(|| anyhow!("missing first window"))?;
        assert_eq!(first.topic, "run:a:events");
        assert_eq!(first.queue_depth, 2);
        assert_eq!(first.dropped_messages, 1);
        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_enforces_publish_rate_limits_per_topic_class() -> Result<()> {
        let fanout = FanoutHub::memory_with_limits(
            8,
            FanoutLimitConfig {
                run_events: tier_limits(1, 1024, 100),
                worker_lifecycle: tier_limits(1, 1024, 100),
                codex_worker_events: tier_limits(1, 1024, 100),
                fallback: tier_limits(1, 1024, 100),
            },
        );
        fanout
            .publish(
                "run:burst:events",
                FanoutMessage {
                    topic: String::new(),
                    sequence: 1,
                    kind: "run.step".to_string(),
                    payload: json!({"seq": 1}),
                    published_at: chrono::Utc::now(),
                },
            )
            .await?;
        let denied = fanout
            .publish(
                "run:burst:events",
                FanoutMessage {
                    topic: String::new(),
                    sequence: 2,
                    kind: "run.step".to_string(),
                    payload: json!({"seq": 2}),
                    published_at: chrono::Utc::now(),
                },
            )
            .await;
        match denied {
            Err(FanoutError::PublishRateLimited {
                topic,
                reason_code,
                max_publish_per_second,
                ..
            }) => {
                assert_eq!(topic, "run:burst:events");
                assert_eq!(reason_code, "khala_publish_rate_limited");
                assert_eq!(max_publish_per_second, 1);
            }
            other => return Err(anyhow!("expected publish rate limit error, got {other:?}")),
        }

        let window = fanout
            .topic_window("run:burst:events")
            .await?
            .ok_or_else(|| anyhow!("missing topic window after publish rate limit"))?;
        assert_eq!(window.publish_rate_limited_count, 1);
        assert_eq!(
            window.last_violation_reason.as_deref(),
            Some("khala_publish_rate_limited")
        );

        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_enforces_payload_size_limits() -> Result<()> {
        let fanout = FanoutHub::memory_with_limits(
            8,
            FanoutLimitConfig {
                run_events: tier_limits(10, 32, 100),
                worker_lifecycle: tier_limits(10, 32, 100),
                codex_worker_events: tier_limits(10, 32, 100),
                fallback: tier_limits(10, 32, 100),
            },
        );
        fanout
            .publish(
                "run:frame:events",
                FanoutMessage {
                    topic: String::new(),
                    sequence: 1,
                    kind: "run.step".to_string(),
                    payload: json!({"ok": true}),
                    published_at: chrono::Utc::now(),
                },
            )
            .await?;
        let denied = fanout
            .publish(
                "run:frame:events",
                FanoutMessage {
                    topic: String::new(),
                    sequence: 2,
                    kind: "run.step".to_string(),
                    payload: json!({"payload": "this payload is intentionally too large"}),
                    published_at: chrono::Utc::now(),
                },
            )
            .await;
        match denied {
            Err(FanoutError::FramePayloadTooLarge {
                topic,
                reason_code,
                max_payload_bytes,
                ..
            }) => {
                assert_eq!(topic, "run:frame:events");
                assert_eq!(reason_code, "khala_frame_payload_too_large");
                assert_eq!(max_payload_bytes, 32);
            }
            other => return Err(anyhow!("expected payload too large error, got {other:?}")),
        }

        let window = fanout
            .topic_window("run:frame:events")
            .await?
            .ok_or_else(|| anyhow!("missing topic window after payload rejection"))?;
        assert_eq!(window.frame_size_rejected_count, 1);
        assert_eq!(
            window.last_violation_reason.as_deref(),
            Some("khala_frame_payload_too_large")
        );

        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_returns_logical_seq_order_when_transport_order_is_mixed() -> Result<()> {
        let fanout = FanoutHub::memory(8);
        for seq in [2_u64, 1_u64, 3_u64] {
            fanout
                .publish(
                    "run:logical-order:events",
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

        let messages = fanout.poll("run:logical-order:events", 0, 10).await?;
        let sequences = messages
            .iter()
            .map(|message| message.sequence)
            .collect::<Vec<_>>();
        assert_eq!(sequences, vec![1, 2, 3]);

        let window = fanout
            .topic_window("run:logical-order:events")
            .await?
            .ok_or_else(|| anyhow!("missing topic window"))?;
        assert_eq!(window.oldest_sequence, 1);
        assert_eq!(window.head_sequence, 3);

        Ok(())
    }

    #[tokio::test]
    async fn memory_fanout_rejects_cursor_when_replay_budget_is_exceeded() -> Result<()> {
        let fanout = FanoutHub::memory_with_limits(
            64,
            FanoutLimitConfig {
                run_events: FanoutTierLimits {
                    qos_tier: QosTier::Hot,
                    replay_budget_events: 5,
                    max_publish_per_second: 500,
                    max_payload_bytes: 1024,
                },
                worker_lifecycle: tier_limits(500, 1024, 100),
                codex_worker_events: tier_limits(500, 1024, 100),
                fallback: tier_limits(500, 1024, 100),
            },
        );
        for seq in 1..=25_u64 {
            fanout
                .publish(
                    "run:budget:events",
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

        let stale = fanout.poll("run:budget:events", 10, 10).await;
        match stale {
            Err(FanoutError::StaleCursor {
                reason_codes,
                replay_lag,
                replay_budget_events,
                qos_tier,
                ..
            }) => {
                assert!(reason_codes.contains(&"replay_budget_exceeded".to_string()));
                assert_eq!(replay_lag, 15);
                assert_eq!(replay_budget_events, 5);
                assert_eq!(qos_tier, "hot");
            }
            other => {
                return Err(anyhow!(
                    "expected replay budget stale cursor error, got {other:?}"
                ));
            }
        }

        let window = fanout
            .topic_window("run:budget:events")
            .await?
            .ok_or_else(|| anyhow!("expected window after replay budget stale"))?;
        assert_eq!(window.stale_cursor_budget_exceeded_count, 1);
        assert_eq!(
            window.last_violation_reason.as_deref(),
            Some("replay_budget_exceeded")
        );

        Ok(())
    }
}
