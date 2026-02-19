use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tokio::sync::mpsc;

const DEFAULT_STATS_URL: &str = "https://nexus.openagents.com/api/stats";

#[derive(Clone, Debug)]
pub(crate) enum NexusStatus {
    Idle,
    Refreshing,
    Error(String),
}

impl NexusStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            NexusStatus::Idle => "Idle",
            NexusStatus::Refreshing => "Refreshing",
            NexusStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            NexusStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct NexusKindCount {
    pub(crate) kind: u16,
    pub(crate) count: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct NexusEventStats {
    pub(crate) total: u64,
    pub(crate) last_24h: u64,
    #[serde(default)]
    pub(crate) by_kind: Vec<NexusKindCount>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct NexusJobStats {
    pub(crate) pending: u64,
    pub(crate) completed_24h: u64,
    #[serde(default)]
    pub(crate) by_kind: Vec<NexusKindCount>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct NexusRlmStats {
    #[serde(default)]
    pub(crate) subqueries_total: u64,
    #[serde(default)]
    pub(crate) subqueries_24h: u64,
    #[serde(default)]
    pub(crate) results_total: u64,
    #[serde(default)]
    pub(crate) results_24h: u64,
    #[serde(default)]
    pub(crate) providers_active: u64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct NexusStats {
    #[serde(default)]
    pub(crate) events: NexusEventStats,
    #[serde(default)]
    pub(crate) jobs: NexusJobStats,
    #[serde(default)]
    pub(crate) rlm: NexusRlmStats,
    #[serde(default)]
    pub(crate) timestamp: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct NexusSnapshot {
    pub(crate) stats: NexusStats,
}

#[derive(Debug, Clone)]
pub(crate) enum NexusEvent {
    Snapshot(NexusSnapshot),
    Error(String),
}

#[derive(Debug)]
pub(crate) enum NexusCommand {
    Refresh { stats_url: String },
}

pub(crate) struct NexusRuntime {
    cmd_tx: mpsc::Sender<NexusCommand>,
    pub(crate) event_rx: mpsc::Receiver<NexusEvent>,
}

impl NexusRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NexusCommand>(8);
        let (event_tx, event_rx) = mpsc::channel::<NexusEvent>(16);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_nexus_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh(&self, stats_url: String) {
        let _ = self.cmd_tx.try_send(NexusCommand::Refresh { stats_url });
    }
}

impl Default for NexusRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct NexusState {
    pub(crate) runtime: NexusRuntime,
    pub(crate) status: NexusStatus,
    pub(crate) snapshot: Option<NexusSnapshot>,
    pub(crate) status_message: Option<String>,
    pub(crate) last_refresh: Option<u64>,
    pub(crate) stats_url: String,
}

impl NexusState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: NexusRuntime::new(),
            status: NexusStatus::Idle,
            snapshot: None,
            status_message: None,
            last_refresh: None,
            stats_url: DEFAULT_STATS_URL.to_string(),
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = NexusStatus::Refreshing;
        self.status_message = Some(format!("Fetching stats from {}", self.stats_url));
        self.runtime.refresh(self.stats_url.clone());
    }

    pub(crate) fn set_stats_url(&mut self, stats_url: String) {
        self.stats_url = stats_url;
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: NexusSnapshot) {
        self.snapshot = Some(snapshot);
        self.last_refresh = Some(now());
        self.status = NexusStatus::Idle;
        self.status_message = None;
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.last_refresh = Some(now());
        self.status = NexusStatus::Error(message.clone());
        self.status_message = Some(message);
    }
}

impl Default for NexusState {
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

async fn run_nexus_loop(
    mut cmd_rx: mpsc::Receiver<NexusCommand>,
    event_tx: mpsc::Sender<NexusEvent>,
) {
    let client = reqwest::Client::new();

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            NexusCommand::Refresh { stats_url } => {
                let response = client.get(&stats_url).send().await;
                match response {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            match resp.json::<NexusStats>().await {
                                Ok(stats) => {
                                    let snapshot = NexusSnapshot { stats };
                                    let _ = event_tx.send(NexusEvent::Snapshot(snapshot)).await;
                                }
                                Err(err) => {
                                    let _ = event_tx
                                        .send(NexusEvent::Error(format!(
                                            "Failed to parse stats: {}",
                                            err
                                        )))
                                        .await;
                                }
                            }
                        } else {
                            let _ = event_tx
                                .send(NexusEvent::Error(format!(
                                    "Stats HTTP error: {}",
                                    resp.status()
                                )))
                                .await;
                        }
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(NexusEvent::Error(format!("Stats fetch failed: {}", err)))
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
    fn nexus_status_labels() {
        assert_eq!(NexusStatus::Idle.label(), "Idle");
        assert_eq!(NexusStatus::Refreshing.label(), "Refreshing");
    }
}
