//! Bloomberg Terminal-style Nostr Chat Screen
//!
//! This module implements a 4-panel chat interface inspired by Bloomberg Terminal:
//! - Command bar (top): Always-active command input
//! - Channel list (left): NIP-28 channels + NIP-90 DVM jobs
//! - Message view (center): Messages with Bloomberg IB-style colors
//! - Info panel (right): Context-sensitive info
//! - Status bar (bottom): Connection status, identity

use gpui_oa::prelude::FluentBuilder;
use gpui_oa::*;
use nostr_chat::ChatState;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use theme_oa::{bg, border, status, text, FONT_FAMILY};
use tokio::sync::mpsc;
use ui_oa::{SubmitEvent, TextInput};

/// Parsed command result for testing
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedCommand {
    Help,
    Connect,
    Join(String),
    Job(u16, String),
    Clear,
    Message(String),
    Invalid(String),
    Empty,
}

/// Parse a command string into a ParsedCommand
pub fn parse_command(input: &str) -> ParsedCommand {
    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return ParsedCommand::Empty;
    }

    let cmd = parts[0].to_lowercase();
    let cmd = cmd.trim_start_matches('/');

    match cmd.as_ref() {
        "help" => ParsedCommand::Help,
        "connect" => ParsedCommand::Connect,
        "join" => {
            if parts.len() > 1 {
                let channel = parts[1].trim_start_matches('#');
                ParsedCommand::Join(channel.to_string())
            } else {
                ParsedCommand::Invalid("Usage: join #channel".to_string())
            }
        }
        "job" => {
            if parts.len() > 2 {
                let kind_str = parts[1];
                let input = parts[2..].join(" ");
                if let Ok(kind) = kind_str.parse::<u16>() {
                    ParsedCommand::Job(kind, input)
                } else {
                    ParsedCommand::Invalid(format!(
                        "Invalid job kind '{}'. Use a number like 5050.",
                        kind_str
                    ))
                }
            } else {
                ParsedCommand::Invalid(
                    "Usage: job <kind> <input> (e.g., job 5050 summarize this)".to_string(),
                )
            }
        }
        "clear" => ParsedCommand::Clear,
        _ => ParsedCommand::Message(input.to_string()),
    }
}


/// Bloomberg-style color constants
mod colors {
    use gpui_oa::Hsla;

    /// Yellow for outgoing messages (Bloomberg style)
    pub const OUTGOING: Hsla = Hsla {
        h: 0.14,
        s: 1.0,
        l: 0.5,
        a: 1.0,
    };

    /// Green for DVM results
    pub const DVM_RESULT: Hsla = Hsla {
        h: 0.33,
        s: 0.8,
        l: 0.45,
        a: 1.0,
    };

    /// Orange for DVM in-progress
    pub const DVM_PENDING: Hsla = Hsla {
        h: 0.08,
        s: 0.9,
        l: 0.55,
        a: 1.0,
    };
}

/// Pending messages from command handler
type PendingMessages = Arc<Mutex<Vec<MockMessage>>>;

/// Commands sent to the background tokio task
#[derive(Debug)]
enum ChatCommand {
    Connect,
    JoinChannel(String),
    SendMessage(String, String), // channel_id, content
    SubmitJob(u16, String),      // kind, input
}

/// Chat screen state
pub struct ChatScreen {
    focus_handle: FocusHandle,

    /// Command bar input
    command_input: Entity<TextInput>,

    /// Chat state (manages relays, channels, messages)
    #[allow(dead_code)]
    chat_state: Arc<Mutex<Option<ChatState>>>,

    /// Pending messages from commands
    pending_messages: PendingMessages,

    /// Command sender to background tokio task
    command_tx: mpsc::UnboundedSender<ChatCommand>,

    /// Selected channel ID
    selected_channel_id: Option<String>,

    /// Mock channels for UI development
    mock_channels: Vec<MockChannel>,

    /// Mock messages for UI development
    mock_messages: Vec<MockMessage>,

    /// Mock DVM jobs
    mock_jobs: Vec<MockJob>,

    /// Info panel collapsed
    #[allow(dead_code)]
    info_panel_collapsed: bool,

    /// Connection status
    connection_status: ConnectionStatus,

    /// Connected relay count
    connected_relay_count: Arc<AtomicUsize>,

    /// User's npub (for display)
    npub_display: String,
}

/// Mock channel for UI development
#[derive(Clone)]
struct MockChannel {
    id: String,
    name: String,
    unread_count: u32,
}

/// Mock message for UI development
#[derive(Clone)]
struct MockMessage {
    #[allow(dead_code)]
    id: String,
    author: String,
    content: String,
    timestamp: String,
    is_own: bool,
    is_dvm: bool,
    dvm_kind: Option<u16>,
}

/// Mock DVM job for UI development
#[derive(Clone)]
struct MockJob {
    #[allow(dead_code)]
    id: String,
    kind: u16,
    status: &'static str,
}

/// Connection status
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionStatus {
    Disconnected,
    #[allow(dead_code)]
    Connecting,
    Connected(u32), // Number of relays
}

fn current_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let secs = now % 86400;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}

impl ChatScreen {
    /// Create a new ChatScreen
    pub fn new(cx: &mut Context<Self>) -> Self {
        let command_input = cx.new(|cx| TextInput::new("", cx));
        let pending_messages: PendingMessages = Arc::new(Mutex::new(Vec::new()));
        let connected_relay_count = Arc::new(AtomicUsize::new(0));

        // Create chat state and set identity
        let mut chat_state = ChatState::new();
        let _ = chat_state.set_identity_from_mnemonic(
            "leader monkey parrot ring guide accident before fence cannon height naive bean",
        );
        let npub_display = chat_state
            .npub()
            .map(|n| format!("{}...{}", &n[..10], &n[n.len() - 5..]))
            .unwrap_or_else(|| "No identity".to_string());

        // Create command channel for background tokio task
        let (command_tx, mut command_rx) = mpsc::unbounded_channel::<ChatCommand>();
        let chat_state = Arc::new(Mutex::new(Some(chat_state)));

        // Spawn background thread with tokio runtime for relay communication
        let pending_for_tokio = pending_messages.clone();
        let relay_count_for_tokio = connected_relay_count.clone();
        let chat_state_for_tokio = chat_state.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                while let Some(cmd) = command_rx.recv().await {
                    match cmd {
                        ChatCommand::Connect => {
                            let chat = chat_state_for_tokio.lock().unwrap().take();
                            if let Some(chat) = chat {
                                match chat.connect().await {
                                    Ok(count) => {
                                        relay_count_for_tokio.store(count, Ordering::SeqCst);
                                        let msg = MockMessage {
                                            id: format!("sys-{}", std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis()),
                                            author: "SYSTEM".to_string(),
                                            content: format!("Connected to {} relays", count),
                                            timestamp: current_timestamp(),
                                            is_own: false,
                                            is_dvm: false,
                                            dvm_kind: None,
                                        };
                                        pending_for_tokio.lock().unwrap().push(msg);
                                    }
                                    Err(e) => {
                                        let msg = MockMessage {
                                            id: format!("sys-{}", std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis()),
                                            author: "SYSTEM".to_string(),
                                            content: format!("Connection error: {}", e),
                                            timestamp: current_timestamp(),
                                            is_own: false,
                                            is_dvm: false,
                                            dvm_kind: None,
                                        };
                                        pending_for_tokio.lock().unwrap().push(msg);
                                    }
                                }
                                // Put it back
                                *chat_state_for_tokio.lock().unwrap() = Some(chat);
                            }
                        }
                        ChatCommand::JoinChannel(channel_id) => {
                            let chat = chat_state_for_tokio.lock().unwrap().take();
                            if let Some(chat) = chat {
                                match chat.join_channel(&channel_id).await {
                                    Ok(()) => {
                                        let msg = MockMessage {
                                            id: format!("sys-{}", std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis()),
                                            author: "SYSTEM".to_string(),
                                            content: format!("Joined channel: {}", channel_id),
                                            timestamp: current_timestamp(),
                                            is_own: false,
                                            is_dvm: false,
                                            dvm_kind: None,
                                        };
                                        pending_for_tokio.lock().unwrap().push(msg);
                                    }
                                    Err(e) => {
                                        let msg = MockMessage {
                                            id: format!("sys-{}", std::time::SystemTime::now()
                                                .duration_since(std::time::UNIX_EPOCH)
                                                .unwrap()
                                                .as_millis()),
                                            author: "SYSTEM".to_string(),
                                            content: format!("Failed to join channel: {}", e),
                                            timestamp: current_timestamp(),
                                            is_own: false,
                                            is_dvm: false,
                                            dvm_kind: None,
                                        };
                                        pending_for_tokio.lock().unwrap().push(msg);
                                    }
                                }
                                *chat_state_for_tokio.lock().unwrap() = Some(chat);
                            }
                        }
                        ChatCommand::SendMessage(_channel_id, _content) => {
                            // TODO: Implement message sending
                            let msg = MockMessage {
                                id: format!("sys-{}", std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()),
                                author: "SYSTEM".to_string(),
                                content: "Message sending not yet implemented".to_string(),
                                timestamp: current_timestamp(),
                                is_own: false,
                                is_dvm: false,
                                dvm_kind: None,
                            };
                            pending_for_tokio.lock().unwrap().push(msg);
                        }
                        ChatCommand::SubmitJob(kind, input) => {
                            // TODO: Implement DVM job submission
                            let msg = MockMessage {
                                id: format!("sys-{}", std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis()),
                                author: "SYSTEM".to_string(),
                                content: format!("DVM job submission not yet implemented (kind={}, input={})", kind, input),
                                timestamp: current_timestamp(),
                                is_own: false,
                                is_dvm: false,
                                dvm_kind: None,
                            };
                            pending_for_tokio.lock().unwrap().push(msg);
                        }
                    }
                }
            });
        });

        // Subscribe to command input submit events
        let pending_for_submit = pending_messages.clone();
        let cmd_tx_for_submit = command_tx.clone();
        cx.subscribe(&command_input, move |_this, _input, event: &SubmitEvent, _cx| {
            let command = event.0.trim().to_string();
            if !command.is_empty() {
                Self::handle_command(&command, &pending_for_submit, &cmd_tx_for_submit);
            }
        })
        .detach();

        // Start background polling for pending messages
        let pending_poll = pending_messages.clone();
        let relay_count_poll = connected_relay_count.clone();
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(100))
                    .await;

                let messages: Vec<MockMessage> = {
                    let mut pending = pending_poll.lock().unwrap();
                    std::mem::take(&mut *pending)
                };

                // Also update connection status based on relay count
                let relay_count = relay_count_poll.load(Ordering::SeqCst);

                if !messages.is_empty() || relay_count > 0 {
                    let _ = view.update(cx, |view, cx| {
                        view.mock_messages.extend(messages);
                        if relay_count > 0 {
                            view.connection_status = ConnectionStatus::Connected(relay_count as u32);
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        // Create mock data for UI development
        let mock_channels = vec![
            MockChannel {
                id: "ch1".to_string(),
                name: "#bitcoin-dev".to_string(),
                unread_count: 12,
            },
            MockChannel {
                id: "ch2".to_string(),
                name: "#nostr-dev".to_string(),
                unread_count: 3,
            },
            MockChannel {
                id: "ch3".to_string(),
                name: "#agents".to_string(),
                unread_count: 0,
            },
            MockChannel {
                id: "ch4".to_string(),
                name: "#mechacoder".to_string(),
                unread_count: 0,
            },
        ];

        let mock_messages = vec![
            MockMessage {
                id: "m1".to_string(),
                author: "satoshi".to_string(),
                content: "Looking for review on PR #42".to_string(),
                timestamp: "15:32:01".to_string(),
                is_own: false,
                is_dvm: false,
                dvm_kind: None,
            },
            MockMessage {
                id: "m2".to_string(),
                author: "hal".to_string(),
                content: "@satoshi I'll take a look".to_string(),
                timestamp: "15:32:15".to_string(),
                is_own: false,
                is_dvm: false,
                dvm_kind: None,
            },
            MockMessage {
                id: "m3".to_string(),
                author: "DVM:5050".to_string(),
                content: "Job submitted: text-gen \"summarize the discussion\"".to_string(),
                timestamp: "15:33:02".to_string(),
                is_own: true,
                is_dvm: true,
                dvm_kind: Some(5050),
            },
            MockMessage {
                id: "m4".to_string(),
                author: "DVM:5050".to_string(),
                content: "Result: The discussion covers PR #42 review request and initial response from hal."
                    .to_string(),
                timestamp: "15:33:45".to_string(),
                is_own: false,
                is_dvm: true,
                dvm_kind: Some(6050),
            },
            MockMessage {
                id: "m5".to_string(),
                author: "adam".to_string(),
                content: "Can someone explain NIP-90?".to_string(),
                timestamp: "15:34:00".to_string(),
                is_own: false,
                is_dvm: false,
                dvm_kind: None,
            },
            MockMessage {
                id: "m6".to_string(),
                author: "you".to_string(),
                content: "NIP-90 defines Data Vending Machines - services that process jobs for sats"
                    .to_string(),
                timestamp: "15:35:12".to_string(),
                is_own: true,
                is_dvm: false,
                dvm_kind: None,
            },
        ];

        let mock_jobs = vec![
            MockJob {
                id: "job1".to_string(),
                kind: 5050,
                status: "OK",
            },
            MockJob {
                id: "job2".to_string(),
                kind: 5100,
                status: "..",
            },
        ];

        Self {
            focus_handle: cx.focus_handle(),
            command_input,
            chat_state,
            pending_messages,
            command_tx,
            selected_channel_id: Some("ch1".to_string()),
            mock_channels,
            mock_messages,
            mock_jobs,
            info_panel_collapsed: false,
            connection_status: ConnectionStatus::Disconnected,
            connected_relay_count,
            npub_display,
        }
    }

    /// Handle a command from the command bar
    fn handle_command(
        command: &str,
        pending: &PendingMessages,
        cmd_tx: &mpsc::UnboundedSender<ChatCommand>,
    ) {
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return;
        }

        let timestamp = current_timestamp();
        let cmd = parts[0].to_lowercase();
        let cmd = cmd.trim_start_matches('/');

        let response = match cmd {
            "help" => {
                "Commands: connect, join #channel, job <kind> <input>, clear, help".to_string()
            }
            "connect" => {
                // Send connect command to background tokio task
                let _ = cmd_tx.send(ChatCommand::Connect);
                "Connecting to relays... (wss://relay.damus.io, wss://nos.lol, wss://relay.nostr.band, wss://nostr.wine)".to_string()
            }
            "join" => {
                if parts.len() > 1 {
                    let channel = parts[1].trim_start_matches('#');
                    // Send join command to background tokio task
                    let _ = cmd_tx.send(ChatCommand::JoinChannel(channel.to_string()));
                    format!("Joining channel: #{}", channel)
                } else {
                    "Usage: join #channel".to_string()
                }
            }
            "job" => {
                if parts.len() > 2 {
                    let kind_str = parts[1];
                    let input = parts[2..].join(" ");
                    if let Ok(kind) = kind_str.parse::<u16>() {
                        // Send job command to background tokio task
                        let _ = cmd_tx.send(ChatCommand::SubmitJob(kind, input.clone()));
                        format!("Submitting DVM job: kind={}, input=\"{}\"", kind, input)
                    } else {
                        format!("Invalid job kind '{}'. Use a number like 5050.", kind_str)
                    }
                } else {
                    "Usage: job <kind> <input> (e.g., job 5050 summarize this)".to_string()
                }
            }
            "clear" => {
                "Use Cmd+K to clear messages (not implemented yet)".to_string()
            }
            _ => {
                // Treat as a message to send
                format!("Sending: {}", command)
            }
        };

        let msg = MockMessage {
            id: format!("sys-{}", std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()),
            author: "SYSTEM".to_string(),
            content: response,
            timestamp,
            is_own: false,
            is_dvm: false,
            dvm_kind: None,
        };

        pending.lock().unwrap().push(msg);
    }

    /// Render the command bar (top)
    fn render_command_bar(&self) -> impl IntoElement {
        let (status_color, status_text) = match self.connection_status {
            ConnectionStatus::Disconnected => (status::ERROR, "DISCONNECTED"),
            ConnectionStatus::Connecting => (status::WARNING, "CONNECTING"),
            ConnectionStatus::Connected(_n) => (status::SUCCESS, "CONNECTED"),
        };

        div()
            .id("command-bar")
            .w_full()
            .h(px(40.0))
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .bg(hsla(0.0, 0.0, 0.03, 1.0))
            .border_b_1()
            .border_color(border::DEFAULT)
            // Command input
            .child(
                div()
                    .flex_1()
                    .mr(px(16.0))
                    .child(self.command_input.clone()),
            )
            // Right side: identity + status
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    // npub display
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(self.npub_display.clone()),
                    )
                    // Connection status
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .bg(hsla(0.0, 0.0, 0.1, 1.0))
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .child(
                                div()
                                    .w(px(6.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(status_color),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(status_color)
                                    .child(status_text),
                            ),
                    ),
            )
    }

    /// Render the channel list (left panel)
    fn render_channel_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Collect channel items using for loop to avoid closure borrow issues
        let mut channel_items = Vec::new();
        for ch in &self.mock_channels {
            channel_items.push(self.render_channel_item(ch, cx));
        }

        let mut job_items = Vec::new();
        for job in &self.mock_jobs {
            job_items.push(self.render_job_item(job));
        }

        div()
            .id("channel-panel")
            .w(px(200.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::SIDEBAR)
            .border_r_1()
            .border_color(border::DEFAULT)
            // Channels header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("CHANNELS"),
                    ),
            )
            // Channel items
            .child(
                div()
                    .id("channel-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(channel_items),
            )
            // DVM Jobs header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("DVM JOBS"),
                    ),
            )
            // Job items
            .child(
                div()
                    .id("job-list")
                    .h(px(100.0))
                    .overflow_y_scroll()
                    .children(job_items),
            )
    }

    /// Render a channel item
    fn render_channel_item(&self, channel: &MockChannel, _cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let is_selected = self.selected_channel_id.as_ref() == Some(&channel.id);
        let (item_bg, item_text) = if is_selected {
            (bg::SELECTED, colors::OUTGOING)
        } else {
            (Hsla::transparent_black(), text::SECONDARY)
        };

        div()
            .id(SharedString::from(format!("ch-{}", channel.id)))
            .px(px(12.0))
            .py(px(6.0))
            .flex()
            .items_center()
            .justify_between()
            .bg(item_bg)
            .cursor_pointer()
            .hover(|s| s.bg(bg::HOVER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_color(text::MUTED)
                            .text_size(px(12.0))
                            .when(is_selected, |el| el.child(">"))
                            .when(!is_selected, |el| el.child(" ")),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .font_family(FONT_FAMILY)
                            .text_color(item_text)
                            .child(channel.name.clone()),
                    ),
            )
            .when(channel.unread_count > 0, |el| {
                el.child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(colors::OUTGOING)
                        .child(format!("{}", channel.unread_count)),
                )
            })
    }

    /// Render a DVM job item
    fn render_job_item(&self, job: &MockJob) -> impl IntoElement {
        let status_color = if job.status == "OK" {
            colors::DVM_RESULT
        } else {
            colors::DVM_PENDING
        };

        let kind_label = match job.kind {
            5050 => "text",
            5100 => "img",
            5250 => "stt",
            _ => "job",
        };

        div()
            .px(px(12.0))
            .py(px(4.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{}", job.kind)),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child(kind_label),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(status_color)
                    .child(job.status),
            )
    }

    /// Render the message view (center panel)
    fn render_message_view(&self) -> impl IntoElement {
        let channel_name = self
            .selected_channel_id
            .as_ref()
            .and_then(|id| self.mock_channels.iter().find(|c| &c.id == id))
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "No channel selected".to_string());

        let msg_count = self.mock_messages.len();

        div()
            .id("message-view")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::APP)
            // Header
            .child(
                div()
                    .h(px(36.0))
                    .px(px(16.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .child(channel_name),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} messages", msg_count)),
                    ),
            )
            // Messages
            .child(
                div()
                    .id("messages-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .px(px(16.0))
                    .py(px(8.0))
                    .children(self.mock_messages.iter().map(|msg| self.render_message(msg))),
            )
    }

    /// Render a single message (Bloomberg IB style)
    fn render_message(&self, msg: &MockMessage) -> impl IntoElement {
        // Bloomberg IB color coding:
        // Yellow: outgoing, White: incoming, Green: DVM results, Orange: DVM pending
        let author_color = if msg.is_own {
            colors::OUTGOING
        } else if msg.is_dvm {
            if msg.dvm_kind.map(|k| k >= 6000).unwrap_or(false) {
                colors::DVM_RESULT
            } else {
                colors::DVM_PENDING
            }
        } else if msg.author == "SYSTEM" {
            text::MUTED
        } else {
            text::SECONDARY
        };

        let content_color = if msg.is_own {
            colors::OUTGOING
        } else if msg.author == "SYSTEM" {
            text::MUTED
        } else {
            text::PRIMARY
        };

        div()
            .py(px(4.0))
            .flex()
            .items_start()
            .gap(px(8.0))
            // Timestamp
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .w(px(60.0))
                    .flex_shrink_0()
                    .child(msg.timestamp.clone()),
            )
            // Author
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(author_color)
                    .w(px(80.0))
                    .flex_shrink_0()
                    .truncate()
                    .child(msg.author.clone()),
            )
            // Content
            .child(
                div()
                    .text_size(px(12.0))
                    .font_family(FONT_FAMILY)
                    .text_color(content_color)
                    .flex_1()
                    .child(msg.content.clone()),
            )
    }

    /// Render the info panel (right panel)
    fn render_info_panel(&self) -> impl IntoElement {
        let channel = self
            .selected_channel_id
            .as_ref()
            .and_then(|id| self.mock_channels.iter().find(|c| &c.id == id));

        div()
            .id("info-panel")
            .w(px(200.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::SIDEBAR)
            .border_l_1()
            .border_color(border::DEFAULT)
            // Channel info header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("CHANNEL INFO"),
                    ),
            )
            // Channel details
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .when_some(channel, |el, ch| {
                        el.child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(ch.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Created: 2024-01-15"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Users: 1,234"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Messages: 45,678"),
                        )
                    }),
            )
            // Commands help header
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .mt(px(8.0))
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child("COMMANDS"),
                    ),
            )
            // Commands list
            .child(
                div()
                    .flex_1()
                    .px(px(12.0))
                    .py(px(8.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("connect"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("join #channel"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("job <kind> <input>"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("help"),
                    ),
            )
    }

    /// Render the status bar (bottom)
    fn render_status_bar(&self) -> impl IntoElement {
        let relay_info = match self.connection_status {
            ConnectionStatus::Disconnected => "Type 'connect' to join relays".to_string(),
            ConnectionStatus::Connecting => "Connecting...".to_string(),
            ConnectionStatus::Connected(n) => format!("{} relays connected", n),
        };

        div()
            .id("status-bar")
            .w_full()
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_between()
            .px(px(16.0))
            .bg(hsla(0.0, 0.0, 0.03, 1.0))
            .border_t_1()
            .border_color(border::DEFAULT)
            // Left: relay info
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(relay_info),
            )
            // Right: channels + messages count
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child(format!(
                        "{} channels | {} messages",
                        self.mock_channels.len(),
                        self.mock_messages.len()
                    )),
            )
    }
}

impl Focusable for ChatScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ChatScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("chat-screen")
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Command bar (top)
            .child(self.render_command_bar())
            // Main content area (4-panel layout)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .overflow_hidden()
                    // Channel list (left)
                    .child(self.render_channel_list(cx))
                    // Message view (center)
                    .child(self.render_message_view())
                    // Info panel (right)
                    .child(self.render_info_panel()),
            )
            // Status bar (bottom)
            .child(self.render_status_bar())
    }
}
