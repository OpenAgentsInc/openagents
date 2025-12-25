use crate::hooks::{Hook, HookResult, SessionEvent, ToolCall, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetric {
    pub action_type: ActionType,
    pub tool_name: Option<String>,
    pub duration_ms: u64,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub success: bool,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionType {
    ToolCall,
    Thinking,
    Response,
    Planning,
    Delegation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApmSnapshot {
    pub apm: f64,
    pub actions_last_minute: u32,
    pub total_actions: u64,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub session_duration_secs: u64,
    pub avg_action_duration_ms: u64,
}

pub struct ApmTracker {
    actions: RwLock<VecDeque<ActionMetric>>,
    session_start: Instant,
    total_actions: AtomicU64,
    total_tokens_in: AtomicU64,
    total_tokens_out: AtomicU64,
    total_duration_ms: AtomicU64,
    window_size: Duration,
}

impl Default for ApmTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ApmTracker {
    pub fn new() -> Self {
        Self::with_window(Duration::from_secs(60))
    }

    pub fn with_window(window_size: Duration) -> Self {
        Self {
            actions: RwLock::new(VecDeque::new()),
            session_start: Instant::now(),
            total_actions: AtomicU64::new(0),
            total_tokens_in: AtomicU64::new(0),
            total_tokens_out: AtomicU64::new(0),
            total_duration_ms: AtomicU64::new(0),
            window_size,
        }
    }

    pub async fn record_action(&self, metric: ActionMetric) {
        self.total_actions.fetch_add(1, Ordering::SeqCst);
        self.total_tokens_in
            .fetch_add(metric.tokens_in, Ordering::SeqCst);
        self.total_tokens_out
            .fetch_add(metric.tokens_out, Ordering::SeqCst);
        self.total_duration_ms
            .fetch_add(metric.duration_ms, Ordering::SeqCst);

        let mut actions = self.actions.write().await;
        actions.push_back(metric);

        let cutoff = self
            .session_start
            .elapsed()
            .saturating_sub(self.window_size)
            .as_millis() as u64;

        while let Some(front) = actions.front() {
            if front.timestamp_ms < cutoff {
                actions.pop_front();
            } else {
                break;
            }
        }
    }

    pub async fn calculate_apm(&self) -> f64 {
        let actions = self.actions.read().await;
        let window_minutes = self.window_size.as_secs_f64() / 60.0;

        if window_minutes > 0.0 {
            actions.len() as f64 / window_minutes
        } else {
            0.0
        }
    }

    pub async fn actions_in_window(&self) -> u32 {
        self.actions.read().await.len() as u32
    }

    pub fn total_actions(&self) -> u64 {
        self.total_actions.load(Ordering::SeqCst)
    }

    pub fn total_tokens_in(&self) -> u64 {
        self.total_tokens_in.load(Ordering::SeqCst)
    }

    pub fn total_tokens_out(&self) -> u64 {
        self.total_tokens_out.load(Ordering::SeqCst)
    }

    pub fn session_duration(&self) -> Duration {
        self.session_start.elapsed()
    }

    pub async fn snapshot(&self) -> ApmSnapshot {
        let total = self.total_actions();
        let duration_ms = self.total_duration_ms.load(Ordering::SeqCst);

        ApmSnapshot {
            apm: self.calculate_apm().await,
            actions_last_minute: self.actions_in_window().await,
            total_actions: total,
            total_tokens_in: self.total_tokens_in(),
            total_tokens_out: self.total_tokens_out(),
            session_duration_secs: self.session_duration().as_secs(),
            avg_action_duration_ms: if total > 0 { duration_ms / total } else { 0 },
        }
    }

    pub async fn reset(&self) {
        self.actions.write().await.clear();
        self.total_actions.store(0, Ordering::SeqCst);
        self.total_tokens_in.store(0, Ordering::SeqCst);
        self.total_tokens_out.store(0, Ordering::SeqCst);
        self.total_duration_ms.store(0, Ordering::SeqCst);
    }
}

pub struct TrajectoryLogger {
    tracker: Arc<ApmTracker>,
    pending_calls: RwLock<std::collections::HashMap<String, Instant>>,
}

impl TrajectoryLogger {
    pub fn new(tracker: Arc<ApmTracker>) -> Self {
        Self {
            tracker,
            pending_calls: RwLock::new(std::collections::HashMap::new()),
        }
    }

    pub fn tracker(&self) -> &Arc<ApmTracker> {
        &self.tracker
    }
}

#[async_trait]
impl Hook for TrajectoryLogger {
    fn name(&self) -> &str {
        "trajectory-logger"
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        let call_key = format!("{}:{}", call.session_id, call.name);
        let mut pending = self.pending_calls.write().await;
        pending.insert(call_key, Instant::now());
        HookResult::Continue
    }

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        let call_key = format!("{}:{}", call.session_id, call.name);
        let start_time = {
            let mut pending = self.pending_calls.write().await;
            pending.remove(&call_key)
        };

        let duration_ms = start_time
            .map(|s| s.elapsed().as_millis() as u64)
            .unwrap_or(0);

        let tokens_in = call
            .parameters
            .get("tokens_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let tokens_out = call
            .parameters
            .get("tokens_out")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let metric = ActionMetric {
            action_type: ActionType::ToolCall,
            tool_name: Some(call.name.clone()),
            duration_ms,
            tokens_in,
            tokens_out,
            success: !output.is_error,
            timestamp_ms: self.tracker.session_start.elapsed().as_millis() as u64,
        };

        self.tracker.record_action(metric).await;
        HookResult::Continue
    }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        match event {
            SessionEvent::Idle { session_id } => {
                let snapshot = self.tracker.snapshot().await;
                tracing::info!(
                    "Session {} idle | APM: {:.1} | Total actions: {}",
                    session_id,
                    snapshot.apm,
                    snapshot.total_actions
                );
            }
            SessionEvent::Error { error, .. } => {
                tracing::warn!("Session error during trajectory logging: {}", error);
            }
            _ => {}
        }
        HookResult::Continue
    }
}

pub struct ThinkingTracker {
    tracker: Arc<ApmTracker>,
}

impl ThinkingTracker {
    pub fn new(tracker: Arc<ApmTracker>) -> Self {
        Self { tracker }
    }

    pub async fn record_thinking(&self, duration_ms: u64, tokens_out: u64) {
        let metric = ActionMetric {
            action_type: ActionType::Thinking,
            tool_name: None,
            duration_ms,
            tokens_in: 0,
            tokens_out,
            success: true,
            timestamp_ms: self.tracker.session_start.elapsed().as_millis() as u64,
        };
        self.tracker.record_action(metric).await;
    }

    pub async fn record_response(&self, duration_ms: u64, tokens_in: u64, tokens_out: u64) {
        let metric = ActionMetric {
            action_type: ActionType::Response,
            tool_name: None,
            duration_ms,
            tokens_in,
            tokens_out,
            success: true,
            timestamp_ms: self.tracker.session_start.elapsed().as_millis() as u64,
        };
        self.tracker.record_action(metric).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metric(action_type: ActionType, timestamp_ms: u64) -> ActionMetric {
        ActionMetric {
            action_type,
            tool_name: Some("test".to_string()),
            duration_ms: 100,
            tokens_in: 50,
            tokens_out: 25,
            success: true,
            timestamp_ms,
        }
    }

    #[tokio::test]
    async fn test_apm_tracker_record() {
        let tracker = ApmTracker::new();

        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 0))
            .await;
        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 100))
            .await;

        assert_eq!(tracker.total_actions(), 2);
        assert_eq!(tracker.total_tokens_in(), 100);
        assert_eq!(tracker.total_tokens_out(), 50);
    }

    #[tokio::test]
    async fn test_apm_calculation() {
        let tracker = ApmTracker::with_window(Duration::from_secs(60));

        for i in 0..10 {
            tracker
                .record_action(create_test_metric(ActionType::ToolCall, i * 1000))
                .await;
        }

        let apm = tracker.calculate_apm().await;
        assert!(apm > 0.0);
        assert_eq!(tracker.actions_in_window().await, 10);
    }

    #[tokio::test]
    async fn test_snapshot() {
        let tracker = ApmTracker::new();

        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 0))
            .await;

        let snapshot = tracker.snapshot().await;

        assert_eq!(snapshot.total_actions, 1);
        assert_eq!(snapshot.total_tokens_in, 50);
        assert_eq!(snapshot.total_tokens_out, 25);
        assert!(snapshot.session_duration_secs < 2);
    }

    #[tokio::test]
    async fn test_reset() {
        let tracker = ApmTracker::new();

        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 0))
            .await;
        tracker.reset().await;

        assert_eq!(tracker.total_actions(), 0);
        assert_eq!(tracker.actions_in_window().await, 0);
    }

    #[tokio::test]
    async fn test_trajectory_logger_hook() {
        let tracker = Arc::new(ApmTracker::new());
        let logger = TrajectoryLogger::new(tracker.clone());

        let mut call = ToolCall {
            name: "Read".to_string(),
            parameters: std::collections::HashMap::new(),
            session_id: "session-1".to_string(),
        };

        logger.before_tool(&mut call).await;

        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut output = ToolOutput {
            content: "file contents".to_string(),
            is_error: false,
        };

        logger.after_tool(&call, &mut output).await;

        assert_eq!(tracker.total_actions(), 1);

        let snapshot = tracker.snapshot().await;
        assert!(snapshot.avg_action_duration_ms >= 10);
    }

    #[tokio::test]
    async fn test_thinking_tracker() {
        let tracker = Arc::new(ApmTracker::new());
        let thinking = ThinkingTracker::new(tracker.clone());

        thinking.record_thinking(500, 100).await;
        thinking.record_response(200, 50, 75).await;

        assert_eq!(tracker.total_actions(), 2);
        assert_eq!(tracker.total_tokens_out(), 175);
    }

    #[test]
    fn test_action_type_serialization() {
        let action = ActionType::ToolCall;
        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("ToolCall"));

        let parsed: ActionType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, ActionType::ToolCall);
    }

    #[tokio::test]
    async fn test_window_expiration() {
        let tracker = ApmTracker::with_window(Duration::from_millis(100));

        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 0))
            .await;

        assert_eq!(tracker.actions_in_window().await, 1);

        tokio::time::sleep(Duration::from_millis(150)).await;

        tracker
            .record_action(create_test_metric(ActionType::ToolCall, 200))
            .await;

        assert!(tracker.total_actions() >= 1);
    }

    #[tokio::test]
    async fn test_concurrent_recording() {
        let tracker = Arc::new(ApmTracker::new());

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let t = tracker.clone();
                tokio::spawn(async move {
                    t.record_action(create_test_metric(ActionType::ToolCall, i * 10))
                        .await;
                })
            })
            .collect();

        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(tracker.total_actions(), 10);
    }
}
