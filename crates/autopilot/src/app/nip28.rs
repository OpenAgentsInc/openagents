use std::time::{Duration, SystemTime, UNIX_EPOCH};

use nostr::public_key_to_npub;
use nostr::{
    EventTemplate, finalize_event, generate_secret_key, get_public_key, get_public_key_hex,
};
use nostr_client::{RelayConnection, RelayMessage};
use tokio::sync::mpsc;

const DEFAULT_RELAY_URL: &str = "wss://nexus.openagents.com/";
const DEFAULT_CHANNEL_NAME: &str = "openagents-providers";
const MAX_CHAT_MESSAGES: usize = 200;

#[derive(Clone, Debug)]
pub(crate) struct Nip28Message {
    pub(crate) _id: String,
    pub(crate) pubkey: String,
    pub(crate) content: String,
    pub(crate) created_at: u64,
}

#[derive(Clone, Debug)]
pub(crate) enum Nip28ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Authenticating,
    Authenticated,
    Error(String),
}

impl Nip28ConnectionStatus {
    pub(crate) fn label(&self) -> &str {
        match self {
            Nip28ConnectionStatus::Disconnected => "Disconnected",
            Nip28ConnectionStatus::Connecting => "Connecting",
            Nip28ConnectionStatus::Connected => "Connected",
            Nip28ConnectionStatus::Authenticating => "Authenticating",
            Nip28ConnectionStatus::Authenticated => "Authenticated",
            Nip28ConnectionStatus::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            Nip28ConnectionStatus::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Nip28Event {
    Connected,
    ConnectionFailed(String),
    AuthChallenge(String),
    Authenticated,
    ChatMessage {
        id: String,
        pubkey: String,
        content: String,
        created_at: u64,
    },
    Published {
        _event_id: String,
    },
    PublishFailed {
        error: String,
    },
    ChannelFound {
        channel_id: String,
        _name: String,
    },
}

#[derive(Debug)]
pub(crate) enum Nip28Command {
    Connect { relay_url: String },
    Authenticate { challenge: String },
    SubscribeChat { channel_id: String },
    PublishChatMessage { channel_id: String, content: String },
    CreateOrFindChannel { name: String },
}

pub(crate) struct Nip28Runtime {
    cmd_tx: mpsc::Sender<Nip28Command>,
    pub(crate) event_rx: mpsc::Receiver<Nip28Event>,
    pub(crate) pubkey_hex: String,
    pub(crate) npub: Option<String>,
}

impl Nip28Runtime {
    pub(crate) fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Nip28Command>(32);
        let (event_tx, event_rx) = mpsc::channel::<Nip28Event>(256);

        let secret_key = generate_secret_key();
        let pubkey_hex = get_public_key_hex(&secret_key).unwrap_or_default();
        let npub = get_public_key(&secret_key)
            .ok()
            .and_then(|pubkey| public_key_to_npub(&pubkey).ok());

        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            runtime.block_on(run_nip28_loop(cmd_rx, event_tx, secret_key));
        });

        Self {
            cmd_tx,
            event_rx,
            pubkey_hex,
            npub,
        }
    }

    pub(crate) fn connect(&self, relay_url: &str) {
        let _ = self.cmd_tx.try_send(Nip28Command::Connect {
            relay_url: relay_url.to_string(),
        });
    }

    pub(crate) fn authenticate(&self, challenge: &str) {
        let _ = self.cmd_tx.try_send(Nip28Command::Authenticate {
            challenge: challenge.to_string(),
        });
    }

    pub(crate) fn subscribe_chat(&self, channel_id: &str) {
        let _ = self.cmd_tx.try_send(Nip28Command::SubscribeChat {
            channel_id: channel_id.to_string(),
        });
    }

    pub(crate) fn publish_chat_message(&self, channel_id: &str, content: &str) {
        let _ = self.cmd_tx.try_send(Nip28Command::PublishChatMessage {
            channel_id: channel_id.to_string(),
            content: content.to_string(),
        });
    }

    pub(crate) fn create_or_find_channel(&self, name: &str) {
        let _ = self.cmd_tx.try_send(Nip28Command::CreateOrFindChannel {
            name: name.to_string(),
        });
    }
}

impl Default for Nip28Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) struct Nip28State {
    pub(crate) runtime: Nip28Runtime,
    pub(crate) relay_url: String,
    pub(crate) channel_id: Option<String>,
    pub(crate) status: Nip28ConnectionStatus,
    pub(crate) messages: Vec<Nip28Message>,
    pub(crate) input: String,
    pub(crate) cursor: usize,
    pub(crate) status_message: Option<String>,
    pending_channel_create: bool,
}

impl Nip28State {
    pub(crate) fn new() -> Self {
        let channel_id = DEFAULT_CHANNEL_NAME.to_string();
        let pending_channel_create = !looks_like_event_id(&channel_id);
        Self {
            runtime: Nip28Runtime::new(),
            relay_url: DEFAULT_RELAY_URL.to_string(),
            channel_id: Some(channel_id),
            status: Nip28ConnectionStatus::Disconnected,
            messages: Vec::new(),
            input: String::new(),
            cursor: 0,
            status_message: None,
            pending_channel_create,
        }
    }

    pub(crate) fn connect(&mut self) {
        self.status = Nip28ConnectionStatus::Connecting;
        self.status_message = Some(format!("Connecting to {}", self.relay_url));
        self.runtime.connect(&self.relay_url);
    }

    pub(crate) fn connect_to(&mut self, relay_url: String) {
        self.relay_url = relay_url;
        self.connect();
    }

    pub(crate) fn set_channel(&mut self, channel: String) {
        let pending = !looks_like_event_id(&channel);
        self.channel_id = Some(channel);
        self.pending_channel_create = pending;
        if matches!(
            self.status,
            Nip28ConnectionStatus::Connected | Nip28ConnectionStatus::Authenticated
        ) {
            self.request_channel_setup();
        }
    }

    pub(crate) fn request_channel_setup(&mut self) {
        if let Some(channel) = &self.channel_id {
            if self.pending_channel_create {
                self.runtime.create_or_find_channel(channel);
            }
            self.runtime.subscribe_chat(channel);
        }
    }

    pub(crate) fn mark_channel_ready(&mut self, channel_id: String) {
        self.channel_id = Some(channel_id);
        self.pending_channel_create = false;
    }

    pub(crate) fn authenticate(&mut self, challenge: &str) {
        self.status = Nip28ConnectionStatus::Authenticating;
        self.runtime.authenticate(challenge);
    }

    pub(crate) fn send_message(&mut self) -> Result<(), String> {
        let channel = self
            .channel_id
            .clone()
            .ok_or_else(|| "No channel selected".to_string())?;
        let message = self.input.trim().to_string();
        if message.is_empty() {
            return Err("Message is empty".to_string());
        }
        self.runtime.publish_chat_message(&channel, &message);
        self.input.clear();
        self.cursor = 0;
        self.status_message = Some("Message sent".to_string());
        Ok(())
    }

    pub(crate) fn push_message(&mut self, message: Nip28Message) {
        self.messages.push(message);
        if self.messages.len() > MAX_CHAT_MESSAGES {
            let overflow = self.messages.len() - MAX_CHAT_MESSAGES;
            self.messages.drain(0..overflow);
        }
    }

    pub(crate) fn insert_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let cursor = self.cursor.min(self.input.len());
        self.input.insert_str(cursor, text);
        self.cursor = cursor + text.len();
    }

    pub(crate) fn backspace(&mut self) {
        if self.cursor == 0 || self.input.is_empty() {
            return;
        }
        let cursor = self.cursor.min(self.input.len());
        let remove_at = cursor - 1;
        self.input.remove(remove_at);
        self.cursor = remove_at;
    }

    pub(crate) fn delete(&mut self) {
        let cursor = self.cursor.min(self.input.len());
        if cursor >= self.input.len() {
            return;
        }
        self.input.remove(cursor);
    }

    pub(crate) fn move_cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub(crate) fn move_cursor_right(&mut self) {
        if self.cursor < self.input.len() {
            self.cursor += 1;
        }
    }

    pub(crate) fn move_cursor_home(&mut self) {
        self.cursor = 0;
    }

    pub(crate) fn move_cursor_end(&mut self) {
        self.cursor = self.input.len();
    }
}

impl Default for Nip28State {
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

fn looks_like_event_id(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}

async fn run_nip28_loop(
    mut cmd_rx: mpsc::Receiver<Nip28Command>,
    event_tx: mpsc::Sender<Nip28Event>,
    secret_key: [u8; 32],
) {
    let mut relay: Option<RelayConnection> = None;
    let mut relay_url_str = String::new();

    loop {
        tokio::select! {
            cmd = cmd_rx.recv() => {
                match cmd {
                    Some(Nip28Command::Connect { relay_url }) => {
                        relay_url_str = relay_url.clone();
                        handle_connect(&mut relay, &event_tx, &relay_url).await;
                    }
                    Some(Nip28Command::Authenticate { challenge }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_authenticate(relay_conn, &event_tx, &secret_key, &challenge, &relay_url_str).await;
                        }
                    }
                    Some(Nip28Command::SubscribeChat { channel_id }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_subscribe_chat(relay_conn, &channel_id).await;
                        }
                    }
                    Some(Nip28Command::PublishChatMessage { channel_id, content }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_publish_chat_message(relay_conn, &event_tx, &secret_key, &channel_id, &content).await;
                        }
                    }
                    Some(Nip28Command::CreateOrFindChannel { name }) => {
                        if let Some(ref relay_conn) = relay {
                            handle_create_or_find_channel(relay_conn, &event_tx, &secret_key, &name).await;
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
    event_tx: &mpsc::Sender<Nip28Event>,
    relay_url: &str,
) {
    match RelayConnection::new(relay_url) {
        Ok(conn) => match conn.connect().await {
            Ok(()) => {
                *relay = Some(conn);
                let _ = event_tx.send(Nip28Event::Connected).await;
            }
            Err(err) => {
                let _ = event_tx
                    .send(Nip28Event::ConnectionFailed(err.to_string()))
                    .await;
            }
        },
        Err(err) => {
            let _ = event_tx
                .send(Nip28Event::ConnectionFailed(err.to_string()))
                .await;
        }
    }
}

async fn handle_authenticate(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<Nip28Event>,
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
                let _ = event_tx.send(Nip28Event::Authenticated).await;
            }
        }
        Err(err) => {
            let _ = event_tx
                .send(Nip28Event::ConnectionFailed(format!(
                    "Auth failed: {}",
                    err
                )))
                .await;
        }
    }
}

async fn handle_subscribe_chat(relay: &RelayConnection, channel_id: &str) {
    let filter = serde_json::json!({
        "kinds": [42],
        "#e": [channel_id],
        "limit": 100
    });
    let _ = relay.subscribe("chat", &[filter]).await;
}

async fn handle_publish_chat_message(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<Nip28Event>,
    secret_key: &[u8; 32],
    channel_id: &str,
    content: &str,
) {
    let template = EventTemplate {
        kind: 42,
        content: content.to_string(),
        tags: vec![vec![
            "e".to_string(),
            channel_id.to_string(),
            relay.url().to_string(),
            "root".to_string(),
        ]],
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let event_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    if confirmation.accepted {
                        let _ = event_tx
                            .send(Nip28Event::Published {
                                _event_id: event_id,
                            })
                            .await;
                    } else {
                        let _ = event_tx
                            .send(Nip28Event::PublishFailed {
                                error: confirmation.message,
                            })
                            .await;
                    }
                }
                Err(err) => {
                    let _ = event_tx
                        .send(Nip28Event::PublishFailed {
                            error: err.to_string(),
                        })
                        .await;
                }
            }
        }
        Err(err) => {
            let _ = event_tx
                .send(Nip28Event::PublishFailed {
                    error: err.to_string(),
                })
                .await;
        }
    }
}

async fn handle_create_or_find_channel(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<Nip28Event>,
    secret_key: &[u8; 32],
    name: &str,
) {
    let metadata = serde_json::json!({
        "name": name,
        "about": "OpenAgents NIP-28 chat channel",
        "relays": [relay.url()]
    });

    let template = EventTemplate {
        kind: 40,
        content: metadata.to_string(),
        tags: vec![],
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let channel_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    let resolved_id = if confirmation.accepted {
                        channel_id
                    } else {
                        name.to_string()
                    };
                    let _ = event_tx
                        .send(Nip28Event::ChannelFound {
                            channel_id: resolved_id,
                            _name: name.to_string(),
                        })
                        .await;
                }
                Err(err) => {
                    let _ = event_tx
                        .send(Nip28Event::ChannelFound {
                            channel_id: name.to_string(),
                            _name: format!("{} ({})", name, err),
                        })
                        .await;
                }
            }
        }
        Err(err) => {
            let _ = event_tx
                .send(Nip28Event::ChannelFound {
                    channel_id: name.to_string(),
                    _name: format!("{} ({})", name, err),
                })
                .await;
        }
    }
}

async fn poll_relay_messages(relay: &RelayConnection, event_tx: &mpsc::Sender<Nip28Event>) {
    while let Ok(Ok(Some(msg))) =
        tokio::time::timeout(Duration::from_millis(10), relay.recv()).await
    {
        match msg {
            RelayMessage::Auth(challenge) => {
                let _ = event_tx.send(Nip28Event::AuthChallenge(challenge)).await;
            }
            RelayMessage::Event(sub_id, event) => {
                if sub_id.as_str() == "chat" && event.kind == 42 {
                    let _ = event_tx
                        .send(Nip28Event::ChatMessage {
                            id: event.id,
                            pubkey: event.pubkey,
                            content: event.content,
                            created_at: event.created_at,
                        })
                        .await;
                }
            }
            RelayMessage::Notice(notice) => {
                let _ = event_tx
                    .send(Nip28Event::PublishFailed { error: notice })
                    .await;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_event_id_matches_hex() {
        let hex = "a".repeat(64);
        assert!(looks_like_event_id(&hex));
        assert!(!looks_like_event_id("openagents-providers"));
        assert!(!looks_like_event_id("deadbeef"));
    }

    #[test]
    fn nip28_channel_create_flag_tracks_names() {
        let mut state = Nip28State::new();
        let hex = "b".repeat(64);
        state.set_channel(hex);
        assert!(!state.pending_channel_create);
        state.set_channel("openagents".to_string());
        assert!(state.pending_channel_create);
    }
}
