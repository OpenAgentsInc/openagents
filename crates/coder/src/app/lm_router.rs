use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lm_router::backends::auto_detect_router;
use lm_router::Error as LmRouterError;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub(crate) enum LmRouterStatus {
    Idle,
    Refreshing,
    NoBackends,
    Error(String),
}

impl LmRouterStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            LmRouterStatus::Idle => "Idle",
            LmRouterStatus::Refreshing => "Refreshing",
            LmRouterStatus::NoBackends => "No backends",
            LmRouterStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            LmRouterStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LmBackendHealth {
    pub(crate) name: String,
    pub(crate) healthy: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct LmRouterSnapshot {
    pub(crate) default_model: Option<String>,
    pub(crate) backends: Vec<LmBackendHealth>,
    pub(crate) models: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum LmRouterEvent {
    Snapshot(LmRouterSnapshot),
    NoBackends(String),
    Error(String),
}

#[derive(Debug)]
pub(crate) enum LmRouterCommand {
    Refresh,
}

pub(crate) struct LmRouterRuntime {
    cmd_tx: mpsc::Sender<LmRouterCommand>,
    pub(crate) event_rx: mpsc::Receiver<LmRouterEvent>,
}

impl LmRouterRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<LmRouterCommand>(8);
        let (event_tx, event_rx) = mpsc::channel::<LmRouterEvent>(16);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_lm_router_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh(&self) {
        let _ = self.cmd_tx.try_send(LmRouterCommand::Refresh);
    }
}

impl Default for LmRouterRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct LmRouterState {
    pub(crate) runtime: LmRouterRuntime,
    pub(crate) status: LmRouterStatus,
    pub(crate) snapshot: Option<LmRouterSnapshot>,
    pub(crate) status_message: Option<String>,
    pub(crate) last_refresh: Option<u64>,
}

impl LmRouterState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: LmRouterRuntime::new(),
            status: LmRouterStatus::Idle,
            snapshot: None,
            status_message: None,
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = LmRouterStatus::Refreshing;
        self.status_message = Some("Refreshing LM router status...".to_string());
        self.runtime.refresh();
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: LmRouterSnapshot) {
        self.snapshot = Some(snapshot);
        self.last_refresh = Some(now());
        self.status = LmRouterStatus::Idle;
        self.status_message = None;
    }

    pub(crate) fn set_no_backends(&mut self, message: String) {
        self.snapshot = None;
        self.last_refresh = Some(now());
        self.status = LmRouterStatus::NoBackends;
        self.status_message = Some(message);
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.last_refresh = Some(now());
        self.status = LmRouterStatus::Error(message.clone());
        self.status_message = Some(message);
    }
}

impl Default for LmRouterState {
    fn default() -> Self {
        Self::new()
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn run_lm_router_loop(
    mut cmd_rx: mpsc::Receiver<LmRouterCommand>,
    event_tx: mpsc::Sender<LmRouterEvent>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            LmRouterCommand::Refresh => {
                match auto_detect_router().await {
                    Ok(auto_router) => {
                        let health = auto_router.router.health_check().await;
                        let mut backends = Vec::new();
                        for name in auto_router.backends {
                            let healthy = health.get(&name).copied().unwrap_or(false);
                            backends.push(LmBackendHealth { name, healthy });
                        }
                        let snapshot = LmRouterSnapshot {
                            default_model: auto_router.default_model,
                            backends,
                            models: auto_router.router.available_models(),
                        };
                        let _ = event_tx.send(LmRouterEvent::Snapshot(snapshot)).await;
                    }
                    Err(LmRouterError::BackendNotFound(message)) => {
                        let _ = event_tx.send(LmRouterEvent::NoBackends(message)).await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(LmRouterEvent::Error(format!(
                                "LM router failed: {}",
                                err
                            )))
                            .await;
                    }
                }
            }
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lm_router_status_labels() {
        assert_eq!(LmRouterStatus::Idle.label(), "Idle");
        assert_eq!(LmRouterStatus::Refreshing.label(), "Refreshing");
        assert_eq!(LmRouterStatus::NoBackends.label(), "No backends");
    }
}
