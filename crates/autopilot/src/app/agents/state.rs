use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;

use crate::app::config::{AgentKindConfig, AgentSelection, AllAgentSettings};
use crate::app::config::agents::migrate_from_model_option;
use crate::app::ModelOption;

use super::backend::AgentAvailability;
use super::registry::AgentRegistry;
use super::backend::ModelInfo;
use super::registry::AgentStatus;
use super::AgentKind;

#[derive(Clone, Debug)]
pub(crate) enum AgentBackendsStatus {
    Idle,
    Refreshing,
    Error(String),
}

impl AgentBackendsStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            AgentBackendsStatus::Idle => "Idle",
            AgentBackendsStatus::Refreshing => "Refreshing",
            AgentBackendsStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            AgentBackendsStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentBackendsSnapshot {
    pub(crate) statuses: Vec<AgentStatus>,
    pub(crate) availability: HashMap<AgentKind, AgentAvailability>,
    pub(crate) models: HashMap<AgentKind, Vec<ModelInfo>>,
    pub(crate) default_models: HashMap<AgentKind, Option<String>>,
    pub(crate) available_kinds: Vec<AgentKind>,
    pub(crate) default_kind: Option<AgentKind>,
    pub(crate) refreshed_at: u64,
}

#[derive(Debug)]
pub(crate) enum AgentBackendsEvent {
    Snapshot(AgentBackendsSnapshot),
}

#[derive(Debug)]
pub(crate) enum AgentBackendsCommand {
    Refresh,
}

pub(crate) struct AgentBackendsRuntime {
    cmd_tx: mpsc::Sender<AgentBackendsCommand>,
    pub(crate) event_rx: mpsc::Receiver<AgentBackendsEvent>,
}

impl AgentBackendsRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<AgentBackendsCommand>(8);
        let (event_tx, event_rx) = mpsc::channel::<AgentBackendsEvent>(16);

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_agent_backends_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    pub(crate) fn refresh(&self) {
        let _ = self.cmd_tx.try_send(AgentBackendsCommand::Refresh);
    }
}

impl Default for AgentBackendsRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct AgentBackendsState {
    pub(crate) runtime: AgentBackendsRuntime,
    pub(crate) status: AgentBackendsStatus,
    pub(crate) snapshot: Option<AgentBackendsSnapshot>,
    pub(crate) settings: AllAgentSettings,
    pub(crate) last_refresh: Option<u64>,
    pub(crate) status_message: Option<String>,
}

impl AgentBackendsState {
    pub(crate) fn new(selected_model: ModelOption) -> Self {
        let selection = migrate_from_model_option(selected_model.model_id());
        let mut settings = AllAgentSettings::default();
        settings.selected = selection;
        if let Some(model_id) = settings.selected.model_id.clone() {
            settings.codex.default_model = Some(model_id);
        }

        Self {
            runtime: AgentBackendsRuntime::new(),
            status: AgentBackendsStatus::Idle,
            snapshot: None,
            settings,
            last_refresh: None,
            status_message: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = AgentBackendsStatus::Refreshing;
        self.status_message = Some("Refreshing agent backends...".to_string());
        self.runtime.refresh();
    }

    pub(crate) fn set_snapshot(&mut self, snapshot: AgentBackendsSnapshot) {
        self.last_refresh = Some(snapshot.refreshed_at);
        self.snapshot = Some(snapshot);
        self.status = AgentBackendsStatus::Idle;
        self.status_message = None;
    }

    pub(crate) fn set_error(&mut self, message: String) {
        self.last_refresh = Some(now());
        self.status = AgentBackendsStatus::Error(message.clone());
        self.status_message = Some(message);
    }

    pub(crate) fn kinds(&self) -> Vec<AgentKind> {
        AgentKind::all().to_vec()
    }

    pub(crate) fn models_for_kind(&self, kind: AgentKind) -> Vec<ModelInfo> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.models.get(&kind))
            .map(|models| models.clone())
            .unwrap_or_default()
    }

    pub(crate) fn status_for_kind(&self, kind: AgentKind) -> Option<AgentStatus> {
        self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .statuses
                .iter()
                .find(|status| status.kind == kind)
                .cloned()
        })
    }

    pub(crate) fn availability_for_kind(&self, kind: AgentKind) -> Option<AgentAvailability> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.availability.get(&kind))
            .cloned()
    }

    pub(crate) fn default_model_for_kind(&self, kind: AgentKind) -> Option<String> {
        self.snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.default_models.get(&kind))
            .and_then(|value| value.clone())
    }

    pub(crate) fn available_kinds(&self) -> Vec<AgentKind> {
        self.snapshot
            .as_ref()
            .map(|snapshot| snapshot.available_kinds.clone())
            .unwrap_or_default()
    }

    pub(crate) fn default_kind(&self) -> Option<AgentKind> {
        self.snapshot.as_ref().and_then(|snapshot| snapshot.default_kind)
    }

    pub(crate) fn selection_indices(&self) -> (usize, usize) {
        let kinds = self.kinds();
        let selected_kind = self.settings.selected.kind();
        let selected_idx = kinds
            .iter()
            .position(|kind| *kind == selected_kind)
            .unwrap_or(0);

        let models = self.models_for_kind(selected_kind);
        let model_idx = match &self.settings.selected.model_id {
            None => 0,
            Some(model_id) => models
                .iter()
                .position(|model| model.id == *model_id)
                .map(|idx| idx + 1)
                .unwrap_or(0),
        };

        (selected_idx, model_idx)
    }

    pub(crate) fn model_index_for_kind(&self, kind: AgentKind) -> usize {
        let models = self.models_for_kind(kind);
        let configured = match kind {
            AgentKind::Codex => self.settings.codex.default_model.as_ref(),
        };
        match configured {
            None => 0,
            Some(model_id) => models
                .iter()
                .position(|model| model.id == *model_id)
                .map(|idx| idx + 1)
                .unwrap_or(0),
        }
    }

    pub(crate) fn set_selection(&mut self, kind: AgentKind, model_id: Option<String>) {
        let agent_kind = AgentKindConfig::from(kind);
        self.settings.selected = match &model_id {
            Some(id) => AgentSelection::with_model(agent_kind, id.clone()),
            None => AgentSelection::new(agent_kind),
        };
        self.settings.codex.default_model = model_id;
    }
}

impl Default for AgentBackendsState {
    fn default() -> Self {
        Self::new(ModelOption::Default)
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn run_agent_backends_loop(
    mut cmd_rx: mpsc::Receiver<AgentBackendsCommand>,
    event_tx: mpsc::Sender<AgentBackendsEvent>,
) {
    let mut registry = AgentRegistry::new();

    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            AgentBackendsCommand::Refresh => {
                registry.refresh_availability();
                let mut statuses = registry.status_summary();
                statuses.sort_by_key(|status| kind_sort_key(status.kind));

                let mut availability = HashMap::new();
                let mut models = HashMap::new();
                let mut default_models = HashMap::new();
                for kind in registry.all_kinds() {
                    if let Some(info) = registry.get_availability(kind) {
                        availability.insert(kind, info.clone());
                    }
                    let default_id = registry
                        .get(kind)
                        .and_then(|backend| backend.default_model_id())
                        .map(|id| id.to_string());
                    default_models.insert(kind, default_id);
                    let list = registry.models_for(kind).await;
                    models.insert(kind, list);
                }
                let available_kinds = registry.available_kinds();
                let default_kind = registry.default_kind();

                let snapshot = AgentBackendsSnapshot {
                    statuses,
                    availability,
                    models,
                    default_models,
                    available_kinds,
                    default_kind,
                    refreshed_at: now(),
                };
                let _ = event_tx.send(AgentBackendsEvent::Snapshot(snapshot)).await;
            }
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn kind_sort_key(kind: AgentKind) -> usize {
    match kind {
        AgentKind::Codex => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_backend_status_labels() {
        assert_eq!(AgentBackendsStatus::Idle.label(), "Idle");
        assert_eq!(AgentBackendsStatus::Refreshing.label(), "Refreshing");
        assert_eq!(AgentBackendsStatus::Error("oops".to_string()).label(), "Error");
    }
}
