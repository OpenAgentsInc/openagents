use dioxus::prelude::*;
use dioxus::document::eval;
use dioxus::fullstack::payloads::websocket::use_websocket;
use mechacoder::{
    ClientMessage, Message, ServerMessage, ThreadEntry, ToolStatus, ToolUse,
};

use super::ConversationGraph;

// Bloomberg-inspired theme colors (from theme_oa)
#[allow(dead_code)]
mod theme {
    pub const BG_APP: &str = "#000000";           // Pure black
    pub const BG_SURFACE: &str = "#0A0A0A";       // Near black
    pub const BG_CODE: &str = "#101010";          // Code blocks
    pub const TEXT_PRIMARY: &str = "#E6E6E6";     // Main text
    pub const TEXT_SECONDARY: &str = "#B0B0B0";   // Less emphasis
    pub const TEXT_MUTED: &str = "#9E9E9E";       // Labels, hints
    pub const TEXT_HIGHLIGHT: &str = "#FFB400";   // Bloomberg yellow
    pub const BORDER: &str = "#1A1A1A";           // Default border
    pub const STATUS_SUCCESS: &str = "#00C853";   // Green
    pub const STATUS_ERROR: &str = "#D32F2F";     // Red
    pub const STATUS_RUNNING: &str = "#FFB400";   // Yellow
}

/// WebSocket endpoint for chat
#[get("/api/chat/ws")]
pub async fn chat_ws(
    options: dioxus::fullstack::payloads::websocket::WebSocketOptions,
) -> Result<
    dioxus::fullstack::payloads::websocket::Websocket<ClientMessage, ServerMessage>,
    ServerFnError,
> {
    use tokio::sync::mpsc;

    Ok(options.on_upgrade(move |mut socket| async move {
        eprintln!("[ws] WebSocket connection established");
        loop {
            eprintln!("[ws] Waiting for client message...");
            let msg = match socket.recv().await {
                Ok(m) => {
                    eprintln!("[ws] Received client message");
                    m
                }
                Err(e) => {
                    eprintln!("[ws] Error receiving: {:?}", e);
                    break;
                }
            };

            match msg {
                ClientMessage::SendMessage { content, cwd } => {
                    eprintln!("[ws] Got SendMessage: {}", content);
                    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();

                    tokio::spawn(async move {
                        mechacoder::session::run_claude_session(content, cwd, tx).await;
                    });

                    eprintln!("[ws] Spawned session, waiting for messages...");
                    while let Some(server_msg) = rx.recv().await {
                        eprintln!("[ws] Got message from channel, sending to client...");
                        let is_done = matches!(server_msg, ServerMessage::Done { .. });
                        if socket.send(server_msg).await.is_err() {
                            eprintln!("[ws] Failed to send to client!");
                            break;
                        }
                        eprintln!("[ws] Sent to client successfully");
                        if is_done {
                            eprintln!("[ws] Done message sent, breaking");
                            break;
                        }
                    }
                    eprintln!("[ws] Finished sending messages for this request");
                }
                ClientMessage::Cancel => {
                    eprintln!("[ws] Got Cancel");
                    let _ = socket
                        .send(ServerMessage::Done {
                            error: Some("Cancelled".to_string()),
                        })
                        .await;
                }
            }
        }
        eprintln!("[ws] WebSocket loop ended");
    }))
}

#[component]
pub fn MechaCoder() -> Element {
    let mut socket = use_websocket(|| chat_ws(dioxus::fullstack::payloads::websocket::WebSocketOptions::new()));

    let mut entries = use_signal(Vec::<ThreadEntry>::new);
    let mut input_value = use_signal(String::new);
    let mut is_loading = use_signal(|| false);
    let mut next_id = use_signal(|| 0usize);
    let mut current_assistant_content = use_signal(String::new);
    let mut current_tool_id = use_signal(|| None::<String>);

    // Receive messages from WebSocket
    use_future(move || async move {
        loop {
            match socket.recv().await {
                Ok(msg) => {
                    match msg {
                        ServerMessage::SessionInit { .. } => {}
                        ServerMessage::TextDelta { text } => {
                            let mut content = current_assistant_content();
                            content.push_str(&text);
                            current_assistant_content.set(content);
                        }
                        ServerMessage::ToolStart {
                            tool_use_id,
                            tool_name,
                        } => {
                            current_tool_id.set(Some(tool_use_id.clone()));
                            entries.write().push(ThreadEntry::ToolUse(ToolUse {
                                tool_use_id,
                                tool_name,
                                input: String::new(),
                                output: None,
                                status: ToolStatus::Running,
                            }));
                        }
                        ServerMessage::ToolInput { partial_json, .. } => {
                            if let Some(tool_id) = current_tool_id() {
                                let mut entries_mut = entries.write();
                                for entry in entries_mut.iter_mut().rev() {
                                    if let ThreadEntry::ToolUse(tool) = entry {
                                        if tool.tool_use_id == tool_id {
                                            tool.input.push_str(&partial_json);
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        ServerMessage::ToolProgress { tool_use_id, .. } => {
                            let mut entries_mut = entries.write();
                            for entry in entries_mut.iter_mut().rev() {
                                if let ThreadEntry::ToolUse(tool) = entry {
                                    if tool.tool_use_id == tool_use_id {
                                        tool.status = ToolStatus::Running;
                                        break;
                                    }
                                }
                            }
                        }
                        ServerMessage::ToolResult {
                            tool_use_id,
                            output,
                            is_error,
                        } => {
                            let mut entries_mut = entries.write();
                            for entry in entries_mut.iter_mut().rev() {
                                if let ThreadEntry::ToolUse(tool) = entry {
                                    if tool.tool_use_id == tool_use_id {
                                        tool.output = Some(output);
                                        tool.status = if is_error {
                                            ToolStatus::Error
                                        } else {
                                            ToolStatus::Completed
                                        };
                                        break;
                                    }
                                }
                            }
                        }
                        ServerMessage::Done { error } => {
                            let content = current_assistant_content();
                            if !content.is_empty() {
                                entries.write().push(ThreadEntry::Message(Message {
                                    id: next_id(),
                                    role: "assistant".to_string(),
                                    content,
                                }));
                                next_id += 1;
                                current_assistant_content.set(String::new());
                            }

                            if let Some(err) = error {
                                entries.write().push(ThreadEntry::Message(Message {
                                    id: next_id(),
                                    role: "assistant".to_string(),
                                    content: format!("Error: {}", err),
                                }));
                                next_id += 1;
                            }

                            is_loading.set(false);
                        }
                    }
                }
                Err(_) => {
                    is_loading.set(false);
                    break;
                }
            }
        }
    });

    let mut send_message = move |_| {
        let content = input_value();
        if content.trim().is_empty() || is_loading() {
            return;
        }

        let user_id = next_id();
        entries.write().push(ThreadEntry::Message(Message {
            id: user_id,
            role: "user".to_string(),
            content: content.clone(),
        }));
        next_id += 1;

        input_value.set(String::new());
        is_loading.set(true);
        current_assistant_content.set(String::new());
        current_tool_id.set(None);

        let message = content;
        spawn(async move {
            let _ = socket
                .send(ClientMessage::SendMessage {
                    content: message,
                    cwd: ".".to_string(),
                })
                .await;
        });
    };

    // Scroll to entry by ID
    let scroll_to_entry = move |id: String| {
        spawn(async move {
            let js = format!(
                r#"document.querySelector('[data-entry-id="{}"]')?.scrollIntoView({{ behavior: 'smooth', block: 'center' }})"#,
                id
            );
            let _ = eval(&js).await;
        });
    };

    // Layout: sidebar graph + main chat
    rsx! {
        div {
            style: "display: flex; height: 100vh; background: {theme::BG_APP}; color: {theme::TEXT_PRIMARY}; font-family: 'Berkeley Mono', 'JetBrains Mono', 'Fira Code', monospace; font-size: 13px; line-height: 1.3;",

            // Conversation graph sidebar
            ConversationGraph {
                entries: entries,
                on_node_click: move |id| scroll_to_entry(id),
            }

            // Main chat area
            div {
                style: "flex: 1; display: flex; flex-direction: column; min-width: 0;",

                // Messages area - takes all available space
                div {
                    style: "flex: 1; overflow-y: auto; padding: 12px 16px; display: flex; justify-content: center;",
                    div {
                        style: "width: 100%; max-width: 768px;",

                    for entry in entries() {
                        match entry {
                            ThreadEntry::Message(msg) => rsx! {
                                div {
                                    "data-entry-id": "{msg.id}",
                                    MessageLine {
                                        key: "{msg.id}",
                                        role: msg.role.clone(),
                                        content: msg.content.clone(),
                                    }
                                }
                            },
                            ThreadEntry::ToolUse(tool) => rsx! {
                                div {
                                    "data-entry-id": "{tool.tool_use_id}",
                                    ToolLine {
                                        key: "{tool.tool_use_id}",
                                        tool_name: tool.tool_name.clone(),
                                        status: tool.status.clone(),
                                        input: tool.input.clone(),
                                    }
                                }
                            },
                        }
                    }

                    // Streaming text
                    if !current_assistant_content().is_empty() && is_loading() {
                        MessageLine {
                            role: "assistant".to_string(),
                            content: current_assistant_content(),
                        }
                    }

                    // Thinking indicator
                    if is_loading() && current_assistant_content().is_empty() {
                        div {
                            style: "color: {theme::TEXT_MUTED}; padding: 8px 0;",
                            "..."
                        }
                    }
                    }
                }

                // Input area - simple, at bottom
                div {
                    style: "border-top: 1px solid {theme::BORDER}; padding: 8px 16px; background: {theme::BG_SURFACE}; display: flex; justify-content: center;",
                    form {
                        style: "display: flex; gap: 8px; width: 100%; max-width: 768px;",
                        onsubmit: move |e| {
                            e.prevent_default();
                            send_message(());
                        },
                        input {
                            style: "flex: 1; background: {theme::BG_APP}; border: 1px solid {theme::BORDER}; color: {theme::TEXT_PRIMARY}; padding: 8px 12px; font-family: inherit; font-size: inherit; outline: none;",
                            placeholder: "Message...",
                            value: input_value(),
                            autofocus: true,
                            oninput: move |e| input_value.set(e.value()),
                        }
                        button {
                            style: "background: {theme::BG_SURFACE}; border: 1px solid {theme::BORDER}; color: {theme::TEXT_PRIMARY}; padding: 8px 16px; font-family: inherit; font-size: inherit; cursor: pointer;",
                            r#type: "submit",
                            disabled: is_loading(),
                            if is_loading() { "..." } else { "Send" }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn MessageLine(role: String, content: String) -> Element {
    let is_user = role == "user";

    // User messages get ">" prefix in yellow, assistant messages are plain
    if is_user {
        rsx! {
            div {
                style: "padding: 8px 0; display: flex; gap: 8px;",
                span {
                    style: "color: {theme::TEXT_HIGHLIGHT}; flex-shrink: 0;",
                    ">"
                }
                div {
                    style: "white-space: pre-wrap; color: {theme::TEXT_PRIMARY};",
                    "{content}"
                }
            }
        }
    } else {
        rsx! {
            div {
                style: "padding: 8px 0; white-space: pre-wrap; color: {theme::TEXT_PRIMARY};",
                "{content}"
            }
        }
    }
}

#[component]
fn ToolLine(tool_name: String, status: ToolStatus, input: String) -> Element {
    let (status_color, status_icon) = match status {
        ToolStatus::Completed => (theme::STATUS_SUCCESS, "✓"),
        ToolStatus::Error => (theme::STATUS_ERROR, "✗"),
        ToolStatus::Running => (theme::STATUS_RUNNING, "..."),
    };

    rsx! {
        div {
            style: "padding: 4px 0; font-size: 12px;",
            div {
                style: "display: flex; align-items: center; gap: 8px;",
                span {
                    style: "color: {theme::TEXT_MUTED};",
                    "{tool_name}"
                }
                span {
                    style: "color: {status_color};",
                    "{status_icon}"
                }
            }
            if !input.is_empty() {
                div {
                    style: "color: {theme::TEXT_MUTED}; font-size: 11px; margin-top: 2px; max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                    title: "{input}",
                    "{input}"
                }
            }
        }
    }
}
