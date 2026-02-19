use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nostr::nip90::{
    KIND_JOB_CODE_REVIEW, KIND_JOB_IMAGE_GENERATION, KIND_JOB_PATCH_GEN, KIND_JOB_REPO_INDEX,
    KIND_JOB_RLM_SUBQUERY, KIND_JOB_SANDBOX_RUN, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION,
};
use nostr::{generate_secret_key, get_public_key_hex};
use nostr_client::dvm::{DvmClient, DvmProvider};
use tokio::sync::mpsc;

const DEFAULT_RELAY_URL: &str = "wss://nexus.openagents.com/";
const DEFAULT_JOB_KIND: u16 = KIND_JOB_TEXT_GENERATION;
const MAX_PROVIDERS: usize = 200;

#[derive(Clone, Debug)]
pub(crate) enum DvmStatus {
    Idle,
    Refreshing,
    Error(String),
}

impl DvmStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            DvmStatus::Idle => "Idle",
            DvmStatus::Refreshing => "Refreshing",
            DvmStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            DvmStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum DvmEvent {
    Providers(Vec<DvmProvider>),
    Error(String),
}

#[derive(Debug)]
pub(crate) enum DvmCommand {
    Refresh { relay_url: String, job_kind: u16 },
}

pub(crate) struct DvmRuntime {
    cmd_tx: mpsc::Sender<DvmCommand>,
    pub(crate) event_rx: mpsc::Receiver<DvmEvent>,
    pub(crate) pubkey_hex: String,
}

impl DvmRuntime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<DvmCommand>(16);
        let (event_tx, event_rx) = mpsc::channel::<DvmEvent>(32);

        let secret_key = generate_secret_key();
        let pubkey_hex = get_public_key_hex(&secret_key).unwrap_or_default();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_dvm_loop(cmd_rx, event_tx, secret_key));
        });

        Self {
            cmd_tx,
            event_rx,
            pubkey_hex,
        }
    }

    pub(crate) fn refresh(&self, relay_url: &str, job_kind: u16) {
        let _ = self.cmd_tx.try_send(DvmCommand::Refresh {
            relay_url: relay_url.to_string(),
            job_kind,
        });
    }
}

impl Default for DvmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct DvmState {
    pub(crate) runtime: DvmRuntime,
    pub(crate) relay_url: String,
    pub(crate) job_kind: u16,
    pub(crate) status: DvmStatus,
    pub(crate) providers: Vec<DvmProvider>,
    pub(crate) last_refresh: Option<u64>,
    pub(crate) status_message: Option<String>,
}

impl DvmState {
    pub(crate) fn new() -> Self {
        Self {
            runtime: DvmRuntime::new(),
            relay_url: DEFAULT_RELAY_URL.to_string(),
            job_kind: DEFAULT_JOB_KIND,
            status: DvmStatus::Idle,
            providers: Vec::new(),
            last_refresh: None,
            status_message: None,
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.status = DvmStatus::Refreshing;
        self.status_message = Some("Discovering providers...".to_string());
        self.runtime.refresh(&self.relay_url, self.job_kind);
    }

    pub(crate) fn connect_to(&mut self, relay_url: String) {
        self.relay_url = relay_url;
        self.refresh();
    }

    pub(crate) fn set_job_kind(&mut self, kind: u16) {
        self.job_kind = kind;
        self.refresh();
    }

    pub(crate) fn set_providers(&mut self, mut providers: Vec<DvmProvider>) {
        providers.sort_by(|a, b| provider_sort_key(a).cmp(&provider_sort_key(b)));
        if providers.len() > MAX_PROVIDERS {
            providers.truncate(MAX_PROVIDERS);
        }
        self.providers = providers;
        self.last_refresh = Some(now());
        self.status = DvmStatus::Idle;
    }
}

impl Default for DvmState {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn job_kind_label(kind: u16) -> &'static str {
    match kind {
        KIND_JOB_TEXT_EXTRACTION => "Text extraction",
        KIND_JOB_SUMMARIZATION => "Summarization",
        KIND_JOB_TRANSLATION => "Translation",
        KIND_JOB_TEXT_GENERATION => "Text generation",
        KIND_JOB_IMAGE_GENERATION => "Image generation",
        KIND_JOB_SPEECH_TO_TEXT => "Speech to text",
        KIND_JOB_SANDBOX_RUN => "Sandbox run",
        KIND_JOB_REPO_INDEX => "Repo index",
        KIND_JOB_PATCH_GEN => "Patch gen",
        KIND_JOB_CODE_REVIEW => "Code review",
        KIND_JOB_RLM_SUBQUERY => "RLM subquery",
        _ => "Custom",
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn provider_sort_key(provider: &DvmProvider) -> String {
    provider
        .name
        .clone()
        .unwrap_or_else(|| provider.pubkey.clone())
}

async fn run_dvm_loop(
    mut cmd_rx: mpsc::Receiver<DvmCommand>,
    event_tx: mpsc::Sender<DvmEvent>,
    secret_key: [u8; 32],
) {
    let client = match DvmClient::new(secret_key) {
        Ok(client) => client,
        Err(err) => {
            let _ = event_tx
                .send(DvmEvent::Error(format!(
                    "Failed to initialize DVM client: {}",
                    err
                )))
                .await;
            return;
        }
    };

    loop {
        let Some(cmd) = cmd_rx.recv().await else {
            break;
        };
        match cmd {
            DvmCommand::Refresh {
                relay_url,
                job_kind,
            } => {
                let relays = vec![relay_url.as_str()];
                match client.discover_providers(job_kind, &relays).await {
                    Ok(providers) => {
                        let _ = event_tx.send(DvmEvent::Providers(providers)).await;
                    }
                    Err(err) => {
                        let _ = event_tx
                            .send(DvmEvent::Error(format!(
                                "Provider discovery failed: {}",
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
    fn provider_list_is_capped() {
        let mut state = DvmState::new();
        let providers = (0..210)
            .map(|idx| DvmProvider {
                pubkey: format!("pubkey-{}", idx),
                name: Some(format!("Provider {}", idx)),
                about: None,
                supported_kinds: vec![KIND_JOB_TEXT_GENERATION],
                relays: vec!["wss://relay.example".to_string()],
            })
            .collect::<Vec<_>>();
        state.set_providers(providers);
        assert_eq!(state.providers.len(), 200);
    }
}
