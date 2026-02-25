use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSyncConnectionState {
    Idle,
    Connecting,
    Live,
    Backoff,
}

impl RuntimeSyncConnectionState {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Connecting => "connecting",
            Self::Live => "live",
            Self::Backoff => "backoff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSyncDisconnectReason {
    StreamClosed,
    TokenRefreshDue,
    StaleCursor,
    Unauthorized,
    Forbidden,
    Network,
    Unknown,
}

impl RuntimeSyncDisconnectReason {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StreamClosed => "stream_closed",
            Self::TokenRefreshDue => "token_refresh_due",
            Self::StaleCursor => "stale_cursor",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Network => "network",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSyncHealthSnapshot {
    pub worker_id: String,
    pub state: RuntimeSyncConnectionState,
    pub connect_attempts: u32,
    pub reconnect_attempts: u32,
    pub next_retry_ms: Option<u64>,
    pub last_disconnect_reason: Option<RuntimeSyncDisconnectReason>,
    pub last_error: Option<String>,
    pub token_refresh_after_in_seconds: Option<u64>,
    pub replay_cursor_seq: Option<u64>,
    pub replay_target_seq: Option<u64>,
    pub replay_lag_seq: Option<u64>,
    pub replay_progress_pct: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPlan {
    pub delay: Duration,
    pub reset_cursor: bool,
    pub refresh_token: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSyncLifecycleConfig {
    pub reconnect_base_ms: u64,
    pub reconnect_max_ms: u64,
    pub refresh_retry_ms: u64,
}

impl Default for RuntimeSyncLifecycleConfig {
    fn default() -> Self {
        Self {
            reconnect_base_ms: 250,
            reconnect_max_ms: 8_000,
            refresh_retry_ms: 100,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerState {
    state: RuntimeSyncConnectionState,
    connect_attempts: u32,
    reconnect_attempts: u32,
    next_retry_ms: Option<u64>,
    last_disconnect_reason: Option<RuntimeSyncDisconnectReason>,
    last_error: Option<String>,
    token_refresh_after_in_seconds: Option<u64>,
    replay_cursor_seq: Option<u64>,
    replay_target_seq: Option<u64>,
}

impl Default for WorkerState {
    fn default() -> Self {
        Self {
            state: RuntimeSyncConnectionState::Idle,
            connect_attempts: 0,
            reconnect_attempts: 0,
            next_retry_ms: None,
            last_disconnect_reason: None,
            last_error: None,
            token_refresh_after_in_seconds: None,
            replay_cursor_seq: None,
            replay_target_seq: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeSyncLifecycleManager {
    config: RuntimeSyncLifecycleConfig,
    workers: HashMap<String, WorkerState>,
}

impl RuntimeSyncLifecycleManager {
    pub fn mark_replay_bootstrap(
        &mut self,
        worker_id: &str,
        replay_cursor_seq: u64,
        replay_target_seq: Option<u64>,
    ) {
        let state = self.worker_mut(worker_id);
        state.replay_cursor_seq = Some(replay_cursor_seq);
        let target = replay_target_seq
            .unwrap_or(replay_cursor_seq)
            .max(replay_cursor_seq);
        state.replay_target_seq = Some(state.replay_target_seq.unwrap_or(0).max(target));
    }

    pub fn mark_replay_progress(
        &mut self,
        worker_id: &str,
        replay_cursor_seq: u64,
        replay_target_seq_hint: Option<u64>,
    ) {
        let state = self.worker_mut(worker_id);
        state.replay_cursor_seq = Some(replay_cursor_seq);
        let baseline = state.replay_target_seq.unwrap_or(replay_cursor_seq);
        let hinted = replay_target_seq_hint.unwrap_or(baseline);
        state.replay_target_seq = Some(hinted.max(baseline).max(replay_cursor_seq));
    }

    pub fn mark_connecting(&mut self, worker_id: &str) {
        let state = self.worker_mut(worker_id);
        state.state = RuntimeSyncConnectionState::Connecting;
        state.connect_attempts = state.connect_attempts.saturating_add(1);
        state.next_retry_ms = None;
    }

    pub fn mark_live(&mut self, worker_id: &str, token_refresh_after_in_seconds: Option<u64>) {
        let state = self.worker_mut(worker_id);
        state.state = RuntimeSyncConnectionState::Live;
        state.last_error = None;
        state.next_retry_ms = None;
        state.token_refresh_after_in_seconds = token_refresh_after_in_seconds;
    }

    #[must_use]
    pub fn mark_disconnect(
        &mut self,
        worker_id: &str,
        reason: RuntimeSyncDisconnectReason,
        error: Option<String>,
    ) -> ReconnectPlan {
        let immediate_refresh = reason == RuntimeSyncDisconnectReason::TokenRefreshDue;
        let delay_ms = if immediate_refresh {
            self.config.refresh_retry_ms.max(1)
        } else {
            self.backoff_ms_for(worker_id)
        };
        let state = self.worker_mut(worker_id);
        state.state = RuntimeSyncConnectionState::Backoff;
        state.reconnect_attempts = state.reconnect_attempts.saturating_add(1);
        state.last_disconnect_reason = Some(reason);
        state.last_error = error;
        state.next_retry_ms = Some(delay_ms);
        ReconnectPlan {
            delay: Duration::from_millis(delay_ms),
            reset_cursor: reason == RuntimeSyncDisconnectReason::StaleCursor,
            refresh_token: matches!(
                reason,
                RuntimeSyncDisconnectReason::TokenRefreshDue
                    | RuntimeSyncDisconnectReason::Unauthorized
                    | RuntimeSyncDisconnectReason::Forbidden
            ),
        }
    }

    #[must_use]
    pub fn snapshot(&self, worker_id: &str) -> Option<RuntimeSyncHealthSnapshot> {
        self.workers.get(worker_id).map(|state| {
            let replay_lag_seq = match (state.replay_cursor_seq, state.replay_target_seq) {
                (Some(cursor), Some(target)) => Some(target.saturating_sub(cursor)),
                _ => None,
            };
            let replay_progress_pct = match (state.replay_cursor_seq, state.replay_target_seq) {
                (Some(_), Some(0)) => Some(100),
                (Some(cursor), Some(target)) => Some(
                    ((cursor.min(target).saturating_mul(100)) / target)
                        .min(100)
                        .try_into()
                        .unwrap_or(100),
                ),
                _ => None,
            };
            RuntimeSyncHealthSnapshot {
                worker_id: worker_id.to_string(),
                state: state.state,
                connect_attempts: state.connect_attempts,
                reconnect_attempts: state.reconnect_attempts,
                next_retry_ms: state.next_retry_ms,
                last_disconnect_reason: state.last_disconnect_reason,
                last_error: state.last_error.clone(),
                token_refresh_after_in_seconds: state.token_refresh_after_in_seconds,
                replay_cursor_seq: state.replay_cursor_seq,
                replay_target_seq: state.replay_target_seq,
                replay_lag_seq,
                replay_progress_pct,
            }
        })
    }

    #[must_use]
    pub fn snapshots(&self) -> Vec<RuntimeSyncHealthSnapshot> {
        let mut snapshots = self
            .workers
            .keys()
            .filter_map(|worker_id| self.snapshot(worker_id))
            .collect::<Vec<_>>();
        snapshots.sort_by(|left, right| left.worker_id.cmp(&right.worker_id));
        snapshots
    }

    fn worker_mut(&mut self, worker_id: &str) -> &mut WorkerState {
        self.workers.entry(worker_id.to_string()).or_default()
    }

    fn backoff_ms_for(&self, worker_id: &str) -> u64 {
        let attempt = self
            .workers
            .get(worker_id)
            .map(|state| state.reconnect_attempts.saturating_add(1))
            .unwrap_or(1);
        let exponent = attempt.saturating_sub(1).min(10);
        let scaled = self
            .config
            .reconnect_base_ms
            .max(1)
            .saturating_mul(1_u64 << exponent);
        scaled.min(
            self.config
                .reconnect_max_ms
                .max(self.config.reconnect_base_ms.max(1)),
        )
    }
}

#[must_use]
pub fn classify_disconnect_reason(error: &str) -> RuntimeSyncDisconnectReason {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("token_refresh_due") || normalized.contains("token refresh due") {
        return RuntimeSyncDisconnectReason::TokenRefreshDue;
    }
    if normalized.contains("stale_cursor") {
        return RuntimeSyncDisconnectReason::StaleCursor;
    }
    if normalized.contains("unauthorized")
        || normalized.contains(" 401")
        || normalized.contains("=401")
    {
        return RuntimeSyncDisconnectReason::Unauthorized;
    }
    if normalized.contains("forbidden")
        || normalized.contains(" 403")
        || normalized.contains("=403")
    {
        return RuntimeSyncDisconnectReason::Forbidden;
    }
    if normalized.contains("closed")
        || normalized.contains("connection reset")
        || normalized.contains("timed out")
        || normalized.contains("timeout")
        || normalized.contains("network")
        || normalized.contains("io error")
        || normalized.contains("i/o error")
    {
        return RuntimeSyncDisconnectReason::Network;
    }
    RuntimeSyncDisconnectReason::Unknown
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeSyncConnectionState, RuntimeSyncDisconnectReason, RuntimeSyncLifecycleManager,
        classify_disconnect_reason,
    };

    #[test]
    fn reconnect_backoff_grows_and_caps_across_disconnects() {
        let mut lifecycle = RuntimeSyncLifecycleManager::default();
        let worker = "desktopw:test:shared";

        lifecycle.mark_connecting(worker);
        lifecycle.mark_live(worker, Some(60));

        let first = lifecycle.mark_disconnect(
            worker,
            RuntimeSyncDisconnectReason::Network,
            Some("socket closed".to_string()),
        );
        assert_eq!(first.delay.as_millis(), 250);

        let second = lifecycle.mark_disconnect(
            worker,
            RuntimeSyncDisconnectReason::Network,
            Some("socket closed".to_string()),
        );
        assert_eq!(second.delay.as_millis(), 500);

        for _ in 0..16 {
            let _ = lifecycle.mark_disconnect(
                worker,
                RuntimeSyncDisconnectReason::Network,
                Some("socket closed".to_string()),
            );
        }

        let capped = lifecycle.mark_disconnect(
            worker,
            RuntimeSyncDisconnectReason::Network,
            Some("socket closed".to_string()),
        );
        assert_eq!(capped.delay.as_millis(), 8_000);
    }

    #[test]
    fn token_refresh_disconnect_forces_fast_retry_and_token_refresh() {
        let mut lifecycle = RuntimeSyncLifecycleManager::default();
        let worker = "desktopw:test:refresh";
        lifecycle.mark_connecting(worker);
        lifecycle.mark_live(worker, Some(10));

        let plan = lifecycle.mark_disconnect(
            worker,
            RuntimeSyncDisconnectReason::TokenRefreshDue,
            Some("token_refresh_due".to_string()),
        );
        assert!(plan.refresh_token);
        assert!(!plan.reset_cursor);
        assert_eq!(plan.delay.as_millis(), 100);

        let snapshot = lifecycle
            .snapshot(worker)
            .expect("worker snapshot should exist");
        assert_eq!(snapshot.state, RuntimeSyncConnectionState::Backoff);
        assert_eq!(snapshot.reconnect_attempts, 1);
        assert_eq!(
            snapshot.last_disconnect_reason,
            Some(RuntimeSyncDisconnectReason::TokenRefreshDue)
        );
    }

    #[test]
    fn stale_cursor_disconnect_requests_cursor_reset() {
        let mut lifecycle = RuntimeSyncLifecycleManager::default();
        let plan = lifecycle.mark_disconnect(
            "desktopw:test:stale",
            RuntimeSyncDisconnectReason::StaleCursor,
            Some("stale_cursor".to_string()),
        );
        assert!(plan.reset_cursor);
    }

    #[test]
    fn lifecycle_tracks_connect_and_live_health_state() {
        let mut lifecycle = RuntimeSyncLifecycleManager::default();
        let worker = "desktopw:test:health";

        lifecycle.mark_connecting(worker);
        let connecting = lifecycle
            .snapshot(worker)
            .expect("snapshot should exist after connecting");
        assert_eq!(connecting.state, RuntimeSyncConnectionState::Connecting);
        assert_eq!(connecting.connect_attempts, 1);
        assert_eq!(connecting.reconnect_attempts, 0);

        lifecycle.mark_live(worker, Some(45));
        let live = lifecycle
            .snapshot(worker)
            .expect("snapshot should exist after live");
        assert_eq!(live.state, RuntimeSyncConnectionState::Live);
        assert_eq!(live.token_refresh_after_in_seconds, Some(45));
        assert_eq!(live.replay_cursor_seq, None);
        assert_eq!(live.replay_target_seq, None);
    }

    #[test]
    fn lifecycle_tracks_replay_cursor_target_lag_and_percent() {
        let mut lifecycle = RuntimeSyncLifecycleManager::default();
        let worker = "desktopw:test:replay";

        lifecycle.mark_replay_bootstrap(worker, 10, Some(40));
        let initial = lifecycle
            .snapshot(worker)
            .expect("snapshot should exist after replay bootstrap");
        assert_eq!(initial.replay_cursor_seq, Some(10));
        assert_eq!(initial.replay_target_seq, Some(40));
        assert_eq!(initial.replay_lag_seq, Some(30));
        assert_eq!(initial.replay_progress_pct, Some(25));

        lifecycle.mark_replay_progress(worker, 32, None);
        let mid = lifecycle
            .snapshot(worker)
            .expect("snapshot should exist after replay progress");
        assert_eq!(mid.replay_cursor_seq, Some(32));
        assert_eq!(mid.replay_target_seq, Some(40));
        assert_eq!(mid.replay_lag_seq, Some(8));
        assert_eq!(mid.replay_progress_pct, Some(80));

        lifecycle.mark_replay_progress(worker, 40, Some(40));
        let caught_up = lifecycle
            .snapshot(worker)
            .expect("snapshot should exist after catch-up");
        assert_eq!(caught_up.replay_lag_seq, Some(0));
        assert_eq!(caught_up.replay_progress_pct, Some(100));
    }

    #[test]
    fn disconnect_classifier_maps_core_error_classes() {
        assert_eq!(
            classify_disconnect_reason("sync_token_refresh_due"),
            RuntimeSyncDisconnectReason::TokenRefreshDue
        );
        assert_eq!(
            classify_disconnect_reason("khala stale_cursor; replay bootstrap required"),
            RuntimeSyncDisconnectReason::StaleCursor
        );
        assert_eq!(
            classify_disconnect_reason("status=401 unauthorized"),
            RuntimeSyncDisconnectReason::Unauthorized
        );
        assert_eq!(
            classify_disconnect_reason("status=403 forbidden"),
            RuntimeSyncDisconnectReason::Forbidden
        );
        assert_eq!(
            classify_disconnect_reason("khala websocket closed"),
            RuntimeSyncDisconnectReason::Network
        );
        assert_eq!(
            classify_disconnect_reason("unexpected failure"),
            RuntimeSyncDisconnectReason::Unknown
        );
    }
}
