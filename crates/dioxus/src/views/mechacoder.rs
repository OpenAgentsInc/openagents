use dioxus::document::eval;
use dioxus::fullstack::payloads::websocket::use_websocket;
use dioxus::prelude::*;
use mechacoder::{ClientMessage, Message, ServerMessage, ThreadEntry, ToolStatus, ToolUse};
use pulldown_cmark::{Options, Parser, html};

use super::ConversationGraph;

/// Convert markdown to HTML
fn markdown_to_html(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(md, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

// Bloomberg-inspired theme colors (from theme_oa)
#[allow(dead_code)]
mod theme {
    pub const BG_APP: &str = "#000000"; // Pure black
    pub const BG_SURFACE: &str = "#0A0A0A"; // Near black
    pub const BG_CODE: &str = "#101010"; // Code blocks
    pub const TEXT_PRIMARY: &str = "#E6E6E6"; // Main text
    pub const TEXT_SECONDARY: &str = "#B0B0B0"; // Less emphasis
    pub const TEXT_MUTED: &str = "#9E9E9E"; // Labels, hints
    pub const TEXT_HIGHLIGHT: &str = "#FFB400"; // Bloomberg yellow
    pub const BORDER: &str = "#1A1A1A"; // Default border
    pub const STATUS_SUCCESS: &str = "#00C853"; // Green
    pub const STATUS_ERROR: &str = "#D32F2F"; // Red
    pub const STATUS_RUNNING: &str = "#FFB400"; // Yellow
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
    let mut socket =
        use_websocket(|| chat_ws(dioxus::fullstack::payloads::websocket::WebSocketOptions::new()));

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
                Ok(msg) => match msg {
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
                },
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

/// CSS for markdown content
const MARKDOWN_STYLES: &str = r#"
    .markdown-content { color: #E6E6E6; }
    .markdown-content p { margin: 0.5em 0; }
    .markdown-content h1, .markdown-content h2, .markdown-content h3 {
        color: #FFB400;
        margin: 1em 0 0.5em 0;
        font-weight: 600;
    }
    .markdown-content h1 { font-size: 1.4em; }
    .markdown-content h2 { font-size: 1.2em; }
    .markdown-content h3 { font-size: 1.1em; }
    .markdown-content code {
        background: #1A1A1A;
        padding: 0.2em 0.4em;
        border-radius: 3px;
        font-family: 'Berkeley Mono', 'JetBrains Mono', monospace;
        font-size: 0.9em;
    }
    .markdown-content pre {
        background: #101010;
        padding: 12px;
        border-radius: 4px;
        overflow-x: auto;
        margin: 0.5em 0;
        border: 1px solid #1A1A1A;
    }
    .markdown-content pre code {
        background: none;
        padding: 0;
    }
    .markdown-content ul, .markdown-content ol {
        margin: 0.5em 0;
        padding-left: 1.5em;
    }
    .markdown-content li { margin: 0.25em 0; }
    .markdown-content blockquote {
        border-left: 3px solid #FFB400;
        margin: 0.5em 0;
        padding-left: 1em;
        color: #B0B0B0;
    }
    .markdown-content a { color: #4A9EFF; text-decoration: none; }
    .markdown-content a:hover { text-decoration: underline; }
    .markdown-content table { border-collapse: collapse; margin: 0.5em 0; }
    .markdown-content th, .markdown-content td {
        border: 1px solid #1A1A1A;
        padding: 0.5em;
    }
    .markdown-content th { background: #0A0A0A; }
    .markdown-content hr { border: none; border-top: 1px solid #1A1A1A; margin: 1em 0; }
"#;

#[component]
fn MessageLine(role: String, content: String) -> Element {
    let is_user = role == "user";

    // User messages get ">" prefix in yellow, assistant messages render as markdown
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
        let html_content = markdown_to_html(&content);
        rsx! {
            style { {MARKDOWN_STYLES} }
            div {
                class: "markdown-content",
                style: "padding: 8px 0;",
                dangerous_inner_html: "{html_content}",
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
