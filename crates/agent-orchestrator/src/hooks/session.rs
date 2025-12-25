use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::{Hook, HookResult, SessionEvent};

type RecoveryCallback = Box<dyn Fn(&str, u32) + Send + Sync>;
type SessionCallback = Box<dyn Fn(&str) + Send + Sync>;
type ErrorCallback = Box<dyn Fn(&str, &str) + Send + Sync>;

pub struct SessionRecoveryHook {
    max_retries: u32,
    retry_count: Arc<AtomicU32>,
    on_recovery: Option<RecoveryCallback>,
}

impl SessionRecoveryHook {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            retry_count: Arc::new(AtomicU32::new(0)),
            on_recovery: None,
        }
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, u32) + Send + Sync + 'static,
    {
        self.on_recovery = Some(Box::new(callback));
        self
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.retry_count.store(0, Ordering::SeqCst);
    }
}

impl Default for SessionRecoveryHook {
    fn default() -> Self {
        Self::new(3)
    }
}

#[async_trait]
impl Hook for SessionRecoveryHook {
    fn name(&self) -> &str {
        "session-recovery"
    }

    fn priority(&self) -> i32 {
        100
    }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        match event {
            SessionEvent::Error { session_id, error } => {
                let current = self.retry_count.fetch_add(1, Ordering::SeqCst);

                if current < self.max_retries {
                    tracing::warn!(
                        session_id = session_id,
                        error = error,
                        retry = current + 1,
                        max_retries = self.max_retries,
                        "Session error, attempting recovery"
                    );

                    if let Some(ref callback) = self.on_recovery {
                        callback(session_id, current + 1);
                    }

                    HookResult::Continue
                } else {
                    tracing::error!(
                        session_id = session_id,
                        error = error,
                        "Max retries exceeded, blocking session"
                    );
                    HookResult::Block {
                        message: format!(
                            "Session recovery failed after {} attempts: {}",
                            self.max_retries, error
                        ),
                    }
                }
            }
            SessionEvent::Created { .. } | SessionEvent::Idle { .. } => {
                self.retry_count.store(0, Ordering::SeqCst);
                HookResult::Continue
            }
            SessionEvent::Aborted { .. } => HookResult::Continue,
        }
    }
}

pub struct SessionNotificationHook {
    on_idle: Option<SessionCallback>,
    on_error: Option<ErrorCallback>,
}

impl SessionNotificationHook {
    pub fn new() -> Self {
        Self {
            on_idle: None,
            on_error: None,
        }
    }

    pub fn on_idle<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.on_idle = Some(Box::new(callback));
        self
    }

    pub fn on_error<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str) + Send + Sync + 'static,
    {
        self.on_error = Some(Box::new(callback));
        self
    }
}

impl Default for SessionNotificationHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for SessionNotificationHook {
    fn name(&self) -> &str {
        "session-notification"
    }

    fn priority(&self) -> i32 {
        10
    }

    async fn on_session(&self, event: &SessionEvent) -> HookResult {
        match event {
            SessionEvent::Idle { session_id } => {
                if let Some(ref callback) = self.on_idle {
                    callback(session_id);
                }
            }
            SessionEvent::Error { session_id, error } => {
                if let Some(ref callback) = self.on_error {
                    callback(session_id, error);
                }
            }
            _ => {}
        }
        HookResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn session_recovery_increments_on_error() {
        let hook = SessionRecoveryHook::new(3);

        let event = SessionEvent::Error {
            session_id: "test".to_string(),
            error: "network failure".to_string(),
        };

        hook.on_session(&event).await;
        assert_eq!(hook.retry_count(), 1);

        hook.on_session(&event).await;
        assert_eq!(hook.retry_count(), 2);
    }

    #[tokio::test]
    async fn session_recovery_blocks_after_max_retries() {
        let hook = SessionRecoveryHook::new(2);

        let event = SessionEvent::Error {
            session_id: "test".to_string(),
            error: "network failure".to_string(),
        };

        let result = hook.on_session(&event).await;
        assert!(!result.is_blocked());

        let result = hook.on_session(&event).await;
        assert!(!result.is_blocked());

        let result = hook.on_session(&event).await;
        assert!(result.is_blocked());
    }

    #[tokio::test]
    async fn session_recovery_resets_on_success() {
        let hook = SessionRecoveryHook::new(3);

        let error_event = SessionEvent::Error {
            session_id: "test".to_string(),
            error: "network failure".to_string(),
        };
        hook.on_session(&error_event).await;
        hook.on_session(&error_event).await;
        assert_eq!(hook.retry_count(), 2);

        let idle_event = SessionEvent::Idle {
            session_id: "test".to_string(),
        };
        hook.on_session(&idle_event).await;
        assert_eq!(hook.retry_count(), 0);
    }

    #[tokio::test]
    async fn session_recovery_calls_callback() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let hook = SessionRecoveryHook::new(3).with_callback(move |_session_id, _retry| {
            called_clone.store(true, Ordering::SeqCst);
        });

        let event = SessionEvent::Error {
            session_id: "test".to_string(),
            error: "network failure".to_string(),
        };

        hook.on_session(&event).await;
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn session_notification_calls_on_idle() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let hook = SessionNotificationHook::new().on_idle(move |_session_id| {
            called_clone.store(true, Ordering::SeqCst);
        });

        let event = SessionEvent::Idle {
            session_id: "test".to_string(),
        };

        hook.on_session(&event).await;
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn session_notification_calls_on_error() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let hook = SessionNotificationHook::new().on_error(move |_session_id, _error| {
            called_clone.store(true, Ordering::SeqCst);
        });

        let event = SessionEvent::Error {
            session_id: "test".to_string(),
            error: "something went wrong".to_string(),
        };

        hook.on_session(&event).await;
        assert!(called.load(Ordering::SeqCst));
    }
}
