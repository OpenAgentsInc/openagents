//! Bloomberg Terminal-style Nostr Chat Screen
//!
//! This module implements a 4-panel chat interface inspired by Bloomberg Terminal:
//! - Command bar (top): Always-active command input
//! - Channel list (left): NIP-28 channels + NIP-90 DVM jobs
//! - Message view (center): Messages with Bloomberg IB-style colors
//! - Info panel (right): Context-sensitive info
//! - Status bar (bottom): Connection status, identity

use gpui::prelude::FluentBuilder;
use gpui::*;
use nostr_chat::ChatState;
use theme::{bg, border, status, text, FONT_FAMILY};
use ui::TextInput;

/// Bloomberg-style color constants
mod colors {
    use gpui::Hsla;

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

/// Chat screen state
pub struct ChatScreen {
    focus_handle: FocusHandle,

    /// Command bar input
    command_input: Entity<TextInput>,

    /// Chat state (manages relays, channels, messages)
    chat_state: ChatState,

    /// Selected channel ID
    selected_channel_id: Option<String>,

    /// Mock channels for UI development
    mock_channels: Vec<MockChannel>,

    /// Mock messages for UI development
    mock_messages: Vec<MockMessage>,

    /// Mock DVM jobs
    mock_jobs: Vec<MockJob>,

    /// Info panel collapsed
    info_panel_collapsed: bool,

    /// Connection status
    connection_status: ConnectionStatus,
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
    id: String,
    kind: u16,
    status: &'static str,
}

/// Connection status
#[derive(Clone, Copy, PartialEq, Eq)]
enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected(u32), // Number of relays
}

impl ChatScreen {
    /// Create a new ChatScreen
    pub fn new(cx: &mut Context<Self>) -> Self {
        let command_input = cx.new(|cx| TextInput::new("> join #bitcoin", cx));

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
            chat_state: ChatState::new(),
            selected_channel_id: Some("ch1".to_string()),
            mock_channels,
            mock_messages,
            mock_jobs,
            info_panel_collapsed: false,
            connection_status: ConnectionStatus::Connected(3),
        }
    }

    /// Render the command bar (top)
    fn render_command_bar(&self) -> impl IntoElement {
        let (status_color, status_text) = match self.connection_status {
            ConnectionStatus::Disconnected => (status::ERROR, "DISCONNECTED"),
            ConnectionStatus::Connecting => (status::WARNING, "CONNECTING"),
            ConnectionStatus::Connected(n) => (status::SUCCESS, "CONNECTED"),
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
                            .child("npub1zut...mytsd"),
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

    /// Render a channel item (click handlers removed to avoid borrow issues)
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
                            .child("1,234 users"),
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
        } else {
            text::SECONDARY
        };

        let content_color = if msg.is_own {
            colors::OUTGOING
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
            // Recent users header
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
                            .child("RECENT USERS"),
                    ),
            )
            // Recent users list
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
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("satoshi"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("hal"),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::SECONDARY)
                            .child("adam"),
                    ),
            )
    }

    /// Render the status bar (bottom)
    fn render_status_bar(&self) -> impl IntoElement {
        let relay_info = match self.connection_status {
            ConnectionStatus::Disconnected => "No relays connected".to_string(),
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
            // Right: timestamp
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .child("15:36:02 UTC"),
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
                    .when(!self.info_panel_collapsed, |el| {
                        el.child(self.render_info_panel())
                    }),
            )
            // Status bar (bottom)
            .child(self.render_status_bar())
    }
}
