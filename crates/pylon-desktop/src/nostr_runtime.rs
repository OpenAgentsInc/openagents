//! Async Nostr runtime for pylon-desktop
//!
//! Bridges async nostr-client with synchronous winit event loop using channels.

use nostr::{
    generate_secret_key, get_public_key_hex, finalize_event, EventTemplate,
    nip90::{KIND_JOB_TEXT_GENERATION, JobRequest, JobInput, JobResult},
};
use nostr_client::{RelayConnection, RelayMessage};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

/// Events sent from Nostr runtime to UI
#[derive(Debug, Clone)]
pub enum NostrEvent {
    /// Connected to relay
    Connected,
    /// Connection failed
    ConnectionFailed(String),
    /// NIP-42 auth challenge received
    AuthChallenge(String),
    /// Authenticated successfully
    Authenticated,
    /// Received a job request (kind 5050)
    JobRequest {
        id: String,
        pubkey: String,
        prompt: String,
        created_at: u64,
    },
    /// Received a job result (kind 6050)
    JobResult {
        id: String,
        request_id: String,
        pubkey: String,
        content: String,
    },
    /// Received a chat message (kind 42)
    ChatMessage {
        id: String,
        pubkey: String,
        content: String,
        created_at: u64,
    },
    /// Event published successfully
    Published { event_id: String },
    /// Event publish failed
    PublishFailed { error: String },
    /// Channel found or created (kind 40)
    ChannelFound { channel_id: String, name: String },
}

/// Commands sent from UI to Nostr runtime
#[derive(Debug)]
pub enum NostrCommand {
    /// Connect to relay
    Connect { relay_url: String },
    /// Respond to NIP-42 auth challenge
    Authenticate { challenge: String },
    /// Subscribe to job requests (kind 5050)
    SubscribeJobs,
    /// Subscribe to chat channel (kind 42)
    SubscribeChat { channel_id: String },
    /// Publish a job request (kind 5050)
    PublishJobRequest { prompt: String },
    /// Publish a job result (kind 6050)
    PublishJobResult { request_id: String, request_pubkey: String, content: String },
    /// Publish a chat message (kind 42)
    PublishChatMessage { channel_id: String, content: String },
    /// Create or find a channel (kind 40)
    CreateOrFindChannel { name: String },
}

/// Nostr runtime handle
pub struct NostrRuntime {
    cmd_tx: mpsc::Sender<NostrCommand>,
    pub event_rx: mpsc::Receiver<NostrEvent>,
    pub pubkey: String,
}

impl NostrRuntime {
    /// Create new Nostr runtime with background thread
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<NostrCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<NostrEvent>(256);

        // Generate keypair for this session
        let secret_key = generate_secret_key();
        let pubkey = get_public_key_hex(&secret_key).expect("Failed to derive public key");
        let pubkey_clone = pubkey.clone();

        // Spawn background thread with tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(run_nostr_loop(cmd_rx, event_tx, secret_key));
        });

        Self { cmd_tx, event_rx, pubkey: pubkey_clone }
    }

    /// Get our public key
    pub fn pubkey(&self) -> &str {
        &self.pubkey
    }

    /// Connect to relay
    pub fn connect(&self, relay_url: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::Connect {
            relay_url: relay_url.to_string(),
        });
    }

    /// Subscribe to job requests
    pub fn subscribe_jobs(&self) {
        let _ = self.cmd_tx.try_send(NostrCommand::SubscribeJobs);
    }

    /// Subscribe to chat channel
    pub fn subscribe_chat(&self, channel_id: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::SubscribeChat {
            channel_id: channel_id.to_string(),
        });
    }

    /// Publish job request
    pub fn publish_job_request(&self, prompt: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::PublishJobRequest {
            prompt: prompt.to_string(),
        });
    }

    /// Publish job result
    pub fn publish_job_result(&self, request_id: &str, request_pubkey: &str, content: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::PublishJobResult {
            request_id: request_id.to_string(),
            request_pubkey: request_pubkey.to_string(),
            content: content.to_string(),
        });
    }

    /// Publish chat message
    pub fn publish_chat_message(&self, channel_id: &str, content: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::PublishChatMessage {
            channel_id: channel_id.to_string(),
            content: content.to_string(),
        });
    }

    /// Create or find a channel
    pub fn create_or_find_channel(&self, name: &str) {
        let _ = self.cmd_tx.try_send(NostrCommand::CreateOrFindChannel {
            name: name.to_string(),
        });
    }
}

impl Default for NostrRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current unix timestamp
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Main async loop running in background thread
async fn run_nostr_loop(
    mut cmd_rx: mpsc::Receiver<NostrCommand>,
    event_tx: mpsc::Sender<NostrEvent>,
    secret_key: [u8; 32],
) {
    let mut relay: Option<RelayConnection> = None;
    let mut relay_url_str = String::new();

    // Process commands
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            NostrCommand::Connect { relay_url } => {
                relay_url_str = relay_url.clone();
                handle_connect(&mut relay, &event_tx, &relay_url).await;
            }
            NostrCommand::Authenticate { challenge } => {
                if let Some(ref relay_conn) = relay {
                    handle_authenticate(relay_conn, &event_tx, &secret_key, &challenge, &relay_url_str).await;
                }
            }
            NostrCommand::SubscribeJobs => {
                if let Some(ref relay_conn) = relay {
                    handle_subscribe_jobs(relay_conn).await;
                }
            }
            NostrCommand::SubscribeChat { channel_id } => {
                if let Some(ref relay_conn) = relay {
                    handle_subscribe_chat(relay_conn, &channel_id).await;
                }
            }
            NostrCommand::PublishJobRequest { prompt } => {
                if let Some(ref relay_conn) = relay {
                    handle_publish_job_request(relay_conn, &event_tx, &secret_key, &prompt).await;
                }
            }
            NostrCommand::PublishJobResult { request_id, request_pubkey, content } => {
                if let Some(ref relay_conn) = relay {
                    handle_publish_job_result(relay_conn, &event_tx, &secret_key, &request_id, &request_pubkey, &content).await;
                }
            }
            NostrCommand::PublishChatMessage { channel_id, content } => {
                if let Some(ref relay_conn) = relay {
                    handle_publish_chat_message(relay_conn, &event_tx, &secret_key, &channel_id, &content).await;
                }
            }
            NostrCommand::CreateOrFindChannel { name } => {
                if let Some(ref relay_conn) = relay {
                    handle_create_or_find_channel(relay_conn, &event_tx, &secret_key, &name).await;
                }
            }
        }

        // Poll for incoming messages if connected
        if let Some(ref relay_conn) = relay {
            if relay_conn.is_connected().await {
                poll_relay_messages(relay_conn, &event_tx).await;
            }
        }
    }
}

/// Handle connect command
async fn handle_connect(
    relay: &mut Option<RelayConnection>,
    event_tx: &mpsc::Sender<NostrEvent>,
    relay_url: &str,
) {
    match RelayConnection::new(relay_url) {
        Ok(conn) => {
            match conn.connect().await {
                Ok(()) => {
                    *relay = Some(conn);
                    let _ = event_tx.send(NostrEvent::Connected).await;
                }
                Err(e) => {
                    let _ = event_tx.send(NostrEvent::ConnectionFailed(e.to_string())).await;
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(NostrEvent::ConnectionFailed(e.to_string())).await;
        }
    }
}

/// Handle NIP-42 authentication
async fn handle_authenticate(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
    secret_key: &[u8; 32],
    challenge: &str,
    relay_url: &str,
) {
    // Create NIP-42 AUTH event
    let template = EventTemplate {
        kind: 22242, // NIP-42 auth kind
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
                let _ = event_tx.send(NostrEvent::Authenticated).await;
            }
        }
        Err(e) => {
            let _ = event_tx.send(NostrEvent::ConnectionFailed(format!("Auth failed: {}", e))).await;
        }
    }
}

/// Subscribe to job requests (kind 5050)
async fn handle_subscribe_jobs(relay: &RelayConnection) {
    let filter = serde_json::json!({
        "kinds": [KIND_JOB_TEXT_GENERATION],
        "limit": 100
    });
    let _ = relay.subscribe("jobs", &[filter]).await;
}

/// Subscribe to chat channel (kind 42)
async fn handle_subscribe_chat(relay: &RelayConnection, channel_id: &str) {
    let filter = serde_json::json!({
        "kinds": [42],
        "#e": [channel_id],
        "limit": 100
    });
    let _ = relay.subscribe("chat", &[filter]).await;
}

/// Publish job request (kind 5050)
async fn handle_publish_job_request(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
    secret_key: &[u8; 32],
    prompt: &str,
) {
    // Build job request using NIP-90 helpers
    let job_request = match JobRequest::new(KIND_JOB_TEXT_GENERATION) {
        Ok(req) => req
            .add_input(JobInput::text(prompt))
            .add_param("model", "apple-fm")
            .add_param("max_tokens", "1024"),
        Err(e) => {
            let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
            return;
        }
    };

    let template = EventTemplate {
        kind: KIND_JOB_TEXT_GENERATION,
        content: String::new(),
        tags: job_request.to_tags(),
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let event_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    if confirmation.accepted {
                        let _ = event_tx.send(NostrEvent::Published { event_id }).await;
                    } else {
                        let _ = event_tx.send(NostrEvent::PublishFailed { error: confirmation.message }).await;
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
        }
    }
}

/// Publish job result (kind 6050)
async fn handle_publish_job_result(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
    secret_key: &[u8; 32],
    request_id: &str,
    request_pubkey: &str,
    content: &str,
) {
    // Build job result using NIP-90 helpers
    let job_result = match JobResult::new(KIND_JOB_TEXT_GENERATION, request_id, request_pubkey, content) {
        Ok(res) => res,
        Err(e) => {
            let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
            return;
        }
    };

    let result_kind = KIND_JOB_TEXT_GENERATION + 1000; // 6050

    let template = EventTemplate {
        kind: result_kind,
        content: content.to_string(),
        tags: job_result.to_tags(),
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let event_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    if confirmation.accepted {
                        let _ = event_tx.send(NostrEvent::Published { event_id }).await;
                    } else {
                        let _ = event_tx.send(NostrEvent::PublishFailed { error: confirmation.message }).await;
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
        }
    }
}

/// Publish chat message (kind 42)
async fn handle_publish_chat_message(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
    secret_key: &[u8; 32],
    channel_id: &str,
    content: &str,
) {
    let template = EventTemplate {
        kind: 42, // Channel message
        content: content.to_string(),
        tags: vec![
            vec!["e".to_string(), channel_id.to_string(), "wss://relay.openagents.com/".to_string(), "root".to_string()],
        ],
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let event_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    if confirmation.accepted {
                        let _ = event_tx.send(NostrEvent::Published { event_id }).await;
                    } else {
                        let _ = event_tx.send(NostrEvent::PublishFailed { error: confirmation.message }).await;
                    }
                }
                Err(e) => {
                    let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
                }
            }
        }
        Err(e) => {
            let _ = event_tx.send(NostrEvent::PublishFailed { error: e.to_string() }).await;
        }
    }
}

/// Create or find a channel (kind 40)
async fn handle_create_or_find_channel(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
    secret_key: &[u8; 32],
    name: &str,
) {
    // For now, just create a new channel
    // In the future, we could query for existing channels with this name
    let metadata = serde_json::json!({
        "name": name,
        "about": "Provider chat for the OpenAgents inference network",
        "relays": ["wss://relay.openagents.com/"]
    });

    let template = EventTemplate {
        kind: 40, // Channel creation
        content: metadata.to_string(),
        tags: vec![],
        created_at: now(),
    };

    match finalize_event(&template, secret_key) {
        Ok(event) => {
            let channel_id = event.id.clone();
            match relay.publish_event(&event, Duration::from_secs(5)).await {
                Ok(confirmation) => {
                    if confirmation.accepted {
                        let _ = event_tx.send(NostrEvent::ChannelFound {
                            channel_id,
                            name: name.to_string(),
                        }).await;
                    } else {
                        // Channel might already exist, use the name as a fallback ID
                        let _ = event_tx.send(NostrEvent::ChannelFound {
                            channel_id: name.to_string(),
                            name: name.to_string(),
                        }).await;
                    }
                }
                Err(e) => {
                    eprintln!("Failed to create channel: {}", e);
                    // Use name as fallback channel ID
                    let _ = event_tx.send(NostrEvent::ChannelFound {
                        channel_id: name.to_string(),
                        name: name.to_string(),
                    }).await;
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to finalize channel event: {}", e);
            let _ = event_tx.send(NostrEvent::ChannelFound {
                channel_id: name.to_string(),
                name: name.to_string(),
            }).await;
        }
    }
}

/// Poll for incoming relay messages
async fn poll_relay_messages(
    relay: &RelayConnection,
    event_tx: &mpsc::Sender<NostrEvent>,
) {
    // Non-blocking poll for messages
    while let Ok(Some(msg)) = relay.recv().await {
        match msg {
            RelayMessage::Auth(challenge) => {
                let _ = event_tx.send(NostrEvent::AuthChallenge(challenge)).await;
            }
            RelayMessage::Event(sub_id, event) => {
                match sub_id.as_str() {
                    "jobs" => {
                        // Parse job request
                        if event.kind == KIND_JOB_TEXT_GENERATION {
                            // Extract prompt from i tag
                            let prompt = event.tags.iter()
                                .find(|t: &&Vec<String>| t.first().map(|s| s.as_str()) == Some("i"))
                                .and_then(|t: &Vec<String>| t.get(1))
                                .cloned()
                                .unwrap_or_default();

                            let _ = event_tx.send(NostrEvent::JobRequest {
                                id: event.id,
                                pubkey: event.pubkey,
                                prompt,
                                created_at: event.created_at,
                            }).await;
                        }
                    }
                    "chat" => {
                        // Parse chat message
                        if event.kind == 42 {
                            let _ = event_tx.send(NostrEvent::ChatMessage {
                                id: event.id,
                                pubkey: event.pubkey,
                                content: event.content,
                                created_at: event.created_at,
                            }).await;
                        }
                    }
                    _ => {
                        // Check for job results (kind 6050)
                        if event.kind == KIND_JOB_TEXT_GENERATION + 1000 {
                            let request_id = event.tags.iter()
                                .find(|t: &&Vec<String>| t.first().map(|s| s.as_str()) == Some("e"))
                                .and_then(|t: &Vec<String>| t.get(1))
                                .cloned()
                                .unwrap_or_default();

                            let _ = event_tx.send(NostrEvent::JobResult {
                                id: event.id,
                                request_id,
                                pubkey: event.pubkey,
                                content: event.content,
                            }).await;
                        }
                    }
                }
            }
            RelayMessage::Notice(notice) => {
                // Log notice but don't send to UI
                eprintln!("Relay notice: {}", notice);
            }
            _ => {}
        }
    }
}
