use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gateway::{CerebrasGateway, Gateway, GatewayError, GatewayHealth, InferenceGateway, ModelInfo};
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub(crate) enum GatewayStatus {
    Idle,
    Refreshing,
    NotConfigured,
    Error(String),
}

impl GatewayStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            GatewayStatus::Idle => "Idle",
            GatewayStatus::Refreshing => "Refreshing",
            GatewayStatus::NotConfigured => "Not configured",
            GatewayStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            GatewayStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GatewaySnapshot {
    pub(crate) provider: String,
    pub(crate) name: String,
    pub(crate) configured: bool,
    pub(crate) health: GatewayHealth,
    pub(crate) models: Vec<ModelInfo>,
}

#[derive(Debug, Clone)]
pub(crate) enum GatewayEvent {
    Snapshot(GatewaySnapshot),
    NotConfigured(String),
    Error(String),
}

#[derive(Debug)]
pub(crate) enum GatewayCommand {
    Refresh,
}

pub(crate) struct GatewayRuntime {
    cmd_tx: mpsc::Sender<GatewayCommand>,
    pub(crate) event_rx: mpsc::Receiver<GatewayEvent>,
}

impl GatewayRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<GatewayCommand>(8);
        let (event_tx, event_rx) = mpsc::channel::<GatewayEvent>(16);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_gateway_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh(&self) {
        let _ = self.cmd_tx.try_send(GatewayCommand::Refresh);
    }
}

impl Default for GatewayRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct GatewayState {
    pub(crate) runtime: GatewayRuntime,
    pub(crate) status: GatewayStatus,
    pub(crate) snapshot: Option<GatewaySnapshot>,
    pub(crate) status_message: Option<String>,
    pub(crate) last_refresh: Option<u64>,
}

impl GatewayState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: GatewayRuntime::new(),
            status: GatewayStatus::Idle,
            snapshot: None,
            status_message: None,
            last_refresh: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = GatewayStatus::Refreshing;
        self.status_message = Some("Refreshing gateway status...".to_string());
        self.runtime.refresh();
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: GatewaySnapshot) {
        self.snapshot = Some(snapshot);
        self.last_refresh = Some(now());
        self.status = GatewayStatus::Idle;
        self.status_message = None;
    }

    pub(crate) fn set_not_configured(&mut self, message: String) {
        self.snapshot = None;
        self.last_refresh = Some(now());
        self.status = GatewayStatus::NotConfigured;
        self.status_message = Some(message);
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.last_refresh = Some(now());
        self.status = GatewayStatus::Error(message.clone());
        self.status_message = Some(message);
    }
}

impl Default for GatewayState {
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

async fn run_gateway_loop(
    mut cmd_rx: mpsc::Receiver<GatewayCommand>,
    event_tx: mpsc::Sender<GatewayEvent>,
) {
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            GatewayCommand::Refresh => {
                let result = CerebrasGateway::from_env();
                let gateway = match result {
                    Ok(gateway) => gateway,
                    Err(GatewayError::NotConfigured(message)) => {
                        let _ = event_tx.send(GatewayEvent::NotConfigured(message)).await;
                        continue;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(GatewayEvent::Error(format!("Gateway init failed: {}", err)))
                            .await;
                        continue;
                    }
                };

                let health = gateway.health().await;
                let models = match gateway.models().await {
                    Ok(models) => models,
                    Err(err) => {
                        let _ = event_tx
                            .send(GatewayEvent::Error(format!(
                                "Model listing failed: {}",
                                err
                            )))
                            .await;
                        continue;
                    }
                };

                let snapshot = GatewaySnapshot {
                    provider: gateway.provider().to_string(),
                    name: gateway.name().to_string(),
                    configured: gateway.is_configured(),
                    health,
                    models,
                };
                let _ = event_tx.send(GatewayEvent::Snapshot(snapshot)).await;
            }
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_status_labels() {
        assert_eq!(GatewayStatus::Idle.label(), "Idle");
        assert_eq!(GatewayStatus::Refreshing.label(), "Refreshing");
        assert_eq!(GatewayStatus::NotConfigured.label(), "Not configured");
    }
}
