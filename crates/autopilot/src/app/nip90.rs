use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nostr::nip90::{
    JobRequest, JobResult, KIND_JOB_CODE_REVIEW, KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION,
    KIND_JOB_PATCH_GEN, KIND_JOB_REPO_INDEX, KIND_JOB_RLM_SUBQUERY, KIND_JOB_SANDBOX_RUN,
    KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_EXTRACTION,
    KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION, get_request_kind, is_job_feedback_kind,
    is_job_request_kind, is_job_result_kind,
};
use nostr::{Event, EventTemplate, finalize_event, generate_secret_key, get_public_key_hex};
use nostr_client::{RelayConnection, RelayMessage};
use tokio::sync::mpsc;

const DEFAULT_RELAY_URL: &str = "wss://nexus.openagents.com/";
const MAX_JOB_EVENTS: usize = 200;

const DEFAULT_JOB_KINDS: &[u16] = &[
    KIND_JOB_TEXT_EXTRACTION,
    KIND_JOB_SUMMARIZATION,
    KIND_JOB_TRANSLATION,
    KIND_JOB_TEXT_GENERATION,
    KIND_JOB_IMAGE_GENERATION,
    KIND_JOB_SPEECH_TO_TEXT,
    KIND_JOB_SANDBOX_RUN,
    KIND_JOB_REPO_INDEX,
    KIND_JOB_PATCH_GEN,
    KIND_JOB_CODE_REVIEW,
    KIND_JOB_RLM_SUBQUERY,
];

#[derive(Clone, Debug)]
pub(crate) enum Nip90MessageKind {
    Request,
    Result,
    Feedback,
}

impl Nip90MessageKind {
    pub(crate) fn label(&self) -> &str {
        match self {
            Nip90MessageKind::Request => "REQ",
            Nip90MessageKind::Result => "RES",
            Nip90MessageKind::Feedback => "FB",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Nip90Message {
    pub(crate) kind: u16,
    pub(crate) message_kind: Nip90MessageKind,
    pub(crate) pubkey: String,
    pub(crate) created_at: u64,
    pub(crate) summary: String,
}

#[derive(Clone, Debug)]
pub(crate) enum Nip90ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Authenticated,
    Error(String),
}

impl Nip90ConnectionStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            Nip90ConnectionStatus::Disconnected => "Disconnected",
            Nip90ConnectionStatus::Connecting => "Connecting",
            Nip90ConnectionStatus::Connected => "Connected",
            Nip90ConnectionStatus::Authenticating => "Authenticating",
            Nip90ConnectionStatus::Authenticated => "Authenticated",
            Nip90ConnectionStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            Nip90ConnectionStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Nip90Event {
    Connected,
    ConnectionFailed(String),
    AuthChallenge(String),
    Authenticated,
    JobMessage(Nip90Message),
    Notice(String),
}

#[derive(Debug)]
pub(crate) enum Nip90Command {
    Connect { relay_url: String },
    Authenticate { challenge: String },
    Subscribe { kinds: Vec<u16> },
}

pub(crate) struct Nip90Runtime {
    cmd_tx: mpsc::Sender<Nip90Command>,
    pub(crate) event_rx: mpsc::Receiver<Nip90Event>,
    #[allow(dead_code)]
    pub(crate) pubkey_hex: String,
}

impl Nip90Runtime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Nip90Command>(32);
        let (event_tx, event_rx) = mpsc::channel::<Nip90Event>(256);

        let secret_key = generate_secret_key();
        let pubkey_hex = get_public_key_hex(&secret_key).unwrap_or_default();

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_nip90_loop(cmd_rx, event_tx, secret_key));
        });

        Self {
            cmd_tx,
            event_rx,
            pubkey_hex,
        }
    }

    pub(crate) fn connect(&self, relay_url: &str) {
        let _ = self.cmd_tx.try_send(Nip90Command::Connect {
            relay_url: relay_url.to_string(),
        });
    }

    pub(crate) fn authenticate(&self, challenge: &str) {
        let _ = self.cmd_tx.try_send(Nip90Command::Authenticate {
            challenge: challenge.to_string(),
        });
    }

    pub(crate) fn subscribe(&self, kinds: Vec<u16>) {
        let _ = self.cmd_tx.try_send(Nip90Command::Subscribe { kinds });
    }
}

impl Default for Nip90Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct Nip90State {
    pub(crate) runtime: Nip90Runtime,
    pub(crate) relay_url: String,
    pub(crate) status: Nip90ConnectionStatus,
    pub(crate) messages: Vec<Nip90Message>,
    pub(crate) status_message: Option<String>,
    subscribed: bool,
}

impl Nip90State {
    pub(crate) fn new() -> Self {
        Self {
            runtime: Nip90Runtime::new(),
            relay_url: DEFAULT_RELAY_URL.to_string(),
            status: Nip90ConnectionStatus::Disconnected,
            messages: Vec::new(),
            status_message: None,
            subscribed: false,
        }
    }

    pub(crate) fn connect(&mut self) {
        self.status = Nip90ConnectionStatus::Connecting;
        self.status_message = Some(format!("Connecting to {}", self.relay_url));
        self.subscribed = false;
        self.runtime.connect(&self.relay_url);
    }

    pub(crate) fn connect_to(&mut self, relay_url: String) {
        self.relay_url = relay_url;
        self.connect();
    }

    pub(crate) fn request_subscription(&mut self) {
        if self.subscribed {
            return;
        }
        self.runtime.subscribe(default_kinds());
        self.subscribed = true;
    }

    pub(crate) fn push_message(&mut self, message: Nip90Message) {
        self.messages.push(message);
        if self.messages.len() > MAX_JOB_EVENTS {
            let overflow = self.messages.len() - MAX_JOB_EVENTS;
            self.messages.drain(0..overflow);
        }
    }
}

impl Default for Nip90State {
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

fn default_kinds() -> Vec<u16> {
    let mut kinds = Vec::new();
    for kind in DEFAULT_JOB_KINDS {
        kinds.push(*kind);
        kinds.push(kind + 1000);
    }
    kinds.push(KIND_JOB_FEEDBACK);
    kinds.sort_unstable();
    kinds.dedup();
    kinds
}

fn job_kind_label(kind: u16) -> &'static str {
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

fn short_id(value: &str) -> String {
    if value.len() > 8 {
        format!("{}...", &value[..8])
    } else {
        value.to_string()
    }
}

fn summarize_request(event_id: &str, request: &JobRequest) -> String {
    let mut summary = format!("{} {}", job_kind_label(request.kind), short_id(event_id));
    if !request.inputs.is_empty() {
        summary.push_str(&format!(" inputs {}", request.inputs.len()));
    }
    if let Some(bid) = request.bid {
        summary.push_str(&format!(" bid {} msat", bid));
    }
    summary
}

fn summarize_result(result: &JobResult) -> String {
    let kind_label = get_request_kind(result.kind)
        .map(job_kind_label)
        .unwrap_or("Result");
    let mut summary = format!("{} req {}", kind_label, short_id(&result.request_id));
    if let Some(amount) = result.amount {
        summary.push_str(&format!(" amount {} msat", amount));
    }
    if !result.content.trim().is_empty() {
        summary.push_str(&format!(
            " {}",
            crate::app::truncate_preview(&result.content, 40)
        ));
    }
    summary
}

fn summarize_feedback(event: &Event) -> Option<(String, Option<String>)> {
    let mut status = None;
    let mut status_extra = None;
    let mut request_id = None;
    let mut amount = None;

    for tag in &event.tags {
        if tag.is_empty() {
            continue;
        }
        match tag[0].as_str() {
            "status" if tag.len() >= 2 => {
                status = Some(tag[1].clone());
                if tag.len() >= 3 {
                    status_extra = Some(tag[2].clone());
                }
            }
            "e" if tag.len() >= 2 => {
                request_id = Some(tag[1].clone());
            }
            "amount" if tag.len() >= 2 => {
                amount = tag[1].parse::<u64>().ok();
            }
            _ => {}
        }
    }

    let request_id = request_id?;
    let mut summary = format!(
        "{} req {}",
        status.clone().unwrap_or_else(|| "update".to_string()),
        short_id(&request_id)
    );
    if let Some(amount) = amount {
        summary.push_str(&format!(" amount {} msat", amount));
    }
    if let Some(extra) = &status_extra {
        if !extra.trim().is_empty() {
            summary.push_str(&format!(" {}", crate::app::truncate_preview(extra, 32)));
        }
    }

    Some((summary, status))
}

fn build_job_message(event: &Event) -> Option<Nip90Message> {
    if is_job_request_kind(event.kind) {
        let request = JobRequest::from_event(event).ok()?;
        Some(Nip90Message {
            kind: event.kind,
            message_kind: Nip90MessageKind::Request,
            pubkey: event.pubkey.clone(),
            created_at: event.created_at,
            summary: summarize_request(&event.id, &request),
        })
    } else if is_job_result_kind(event.kind) {
        let result = JobResult::from_event(event).ok()?;
        Some(Nip90Message {
            kind: event.kind,
            message_kind: Nip90MessageKind::Result,
            pubkey: event.pubkey.clone(),
            created_at: event.created_at,
            summary: summarize_result(&result),
        })
    } else if is_job_feedback_kind(event.kind) {
        let (summary, _status) = summarize_feedback(event)?;
        Some(Nip90Message {
            kind: event.kind,
            message_kind: Nip90MessageKind::Feedback,
            pubkey: event.pubkey.clone(),
            created_at: event.created_at,
            summary,
        })
    } else {
        None
    }
}

async fn run_nip90_loop(
    mut cmd_rx: mpsc::Receiver<Nip90Command>,
    event_tx: mpsc::Sender<Nip90Event>,
    secret_key: [u8; 32],
) {
    let mut relay: Option<RelayConnection> = None;
    let mut relay_url_str = String::new();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(Nip90Command::Connect { relay_url }) => {
                        relay_url_str = relay_url.clone();
                        handle_connect(&mut relay, &event_tx, &relay_url).await;
                    }
                    Some(Nip90Command::Authenticate { challenge }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_authenticate(relay_conn, &event_tx, &secret_key, &challenge, &relay_url_str).await;
                        }
                    }
                    Some(Nip90Command::Subscribe { kinds }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_subscribe(relay_conn, &kinds).await;
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(50)) => {
                if let Some(ref relay_conn) = relay {
                    if relay_conn.is_connected().await {
                        poll_relay_messages(relay_conn, &event_tx).await;
                    }
                }
            }
        }
    }
}

async fn handle_connect(
    relay: &mut Option<RelayConnection>,
    event_tx: &mpsc::Sender<Nip90Event>,
    relay_url: &str,
) {
    match RelayConnection::new(relay_url) {
        Ok(conn) => match conn.connect().await {
            Ok(()) => {
                *relay = Some(conn);
                let _ = event_tx.send(Nip90Event::Connected).await;
            }
            Err(err) => {
                let _ = event_tx
                    .send(Nip90Event::ConnectionFailed(err.to_string()))
                    .await;
            }
        },
        Err(err) => {
            let _ = event_tx
                .send(Nip90Event::ConnectionFailed(err.to_string()))
                .await;
        }
    }
}

async fn handle_authenticate(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<Nip90Event>,
    secret_key: &[u8; 32],
    challenge: &str,
    relay_url: &str,
) {
    let template = EventTemplate {
        kind: 22242,
        content: String::new(),
        tags: vec![
            vec!["relay".to_string(), relay_url.to_string()],
            vec!["challenge".to_string(), challenge.to_string()],
        ],
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let msg = serde_json::json!(["AUTH", event]);
            if relay.send_message(&msg).await.is_ok() {
                let _ = event_tx.send(Nip90Event::Authenticated).await;
            }
        }
        Err(err) => {
            let _ = event_tx
                .send(Nip90Event::ConnectionFailed(format!(
                    "Auth failed: {}",
                    err
                )))
                .await;
        }
    }
}

async fn handle_subscribe(relay: &RelayConnection, kinds: &[u16]) {
    let kinds: Vec<u16> = kinds.iter().copied().collect();
    let filter = serde_json::json!({
        "kinds": kinds,
        "limit": 50
    });
    let _ = relay.subscribe("nip90", &[filter]).await;
}

async fn poll_relay_messages(relay: &RelayConnection, event_tx: &mpsc::Sender<Nip90Event>) {
    while let Ok(Ok(Some(msg))) =
        tokio::time::timeout(Duration::from_millis(10), relay.recv()).await
    {
        match msg {
            RelayMessage::Auth(challenge) => {
                let _ = event_tx.send(Nip90Event::AuthChallenge(challenge)).await;
            }
            RelayMessage::Event(sub_id, event) => {
                if sub_id.as_str() == "nip90" {
                    if let Some(message) = build_job_message(&event) {
                        let _ = event_tx.send(Nip90Event::JobMessage(message)).await;
                    }
                }
            }
            RelayMessage::Notice(notice) => {
                let _ = event_tx.send(Nip90Event::Notice(notice)).await;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_kinds_include_results_and_feedback() {
        let kinds = default_kinds();
        assert!(kinds.contains(&KIND_JOB_FEEDBACK));
        assert!(kinds.contains(&KIND_JOB_TEXT_GENERATION));
        assert!(kinds.contains(&(KIND_JOB_TEXT_GENERATION + 1000)));
    }
}
