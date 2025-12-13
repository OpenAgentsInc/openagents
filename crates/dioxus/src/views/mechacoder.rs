use dioxus::prelude::*;
use futures::StreamExt;
use lumen_blocks::components::avatar::{Avatar, AvatarFallback};
use lumen_blocks::components::button::{Button, ButtonVariant};
use serde::{Deserialize, Serialize};

/// Message in the conversation
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: usize,
    pub role: String,
    pub content: String,
}

/// Tool use entry
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUse {
    pub tool_use_id: String,
    pub tool_name: String,
    pub input: String,
    pub output: Option<String>,
    pub status: String, // "pending", "running", "completed", "failed"
}

/// Thread entry - either a message or tool use
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum ThreadEntry {
    Message(Message),
    ToolUse(ToolUse),
}

/// Streaming entry for real-time updates
#[derive(Clone, Serialize, Deserialize)]
pub enum StreamEntry {
    /// Session initialized with ID
    SessionInit { session_id: String },
    /// New text content (incremental)
    TextDelta { text: String },
    /// Tool call started
    ToolStart { tool_use_id: String, tool_name: String },
    /// Tool input streaming (partial JSON)
    ToolInput { tool_use_id: String, partial_json: String },
    /// Tool execution progress
    ToolProgress { tool_use_id: String, elapsed_seconds: f64 },
    /// Tool completed with result
    ToolResult {
        tool_use_id: String,
        output: String,
        is_error: bool,
    },
    /// Stream complete
    Done { error: Option<String> },
}

/// Send a message to Claude via the SDK with streaming updates
#[server(output = dioxus::fullstack::payloads::stream::JsonStream)]
pub async fn send_chat_message(
    message: String,
    cwd: String,
) -> Result<dioxus::fullstack::payloads::stream::JsonStream<StreamEntry>, ServerFnError> {
    use claude_agent_sdk::{query, QueryOptions, SdkMessage};
    use dioxus::fullstack::payloads::stream::JsonStream;
    use futures::channel::mpsc;
    use std::path::PathBuf;

    let (tx, rx) = mpsc::unbounded();

    // Spawn background task to process SDK stream
    tokio::spawn(async move {
        // Create query options
        let options = QueryOptions::new()
            .cwd(PathBuf::from(&cwd))
            .dangerously_skip_permissions(true);

        // Create query stream
        let query_result = query(&message, options).await;
        let mut stream = match query_result {
            Ok(q) => q,
            Err(e) => {
                let _ = tx.unbounded_send(StreamEntry::Done {
                    error: Some(e.to_string()),
                });
                return;
            }
        };

        // Process stream and send entries
        while let Some(msg_result) = stream.next().await {
            let msg = match msg_result {
                Ok(m) => m,
                Err(e) => {
                    let _ = tx.unbounded_send(StreamEntry::Done {
                        error: Some(e.to_string()),
                    });
                    return;
                }
            };

            match msg {
                SdkMessage::System(sys) => {
                    if let claude_agent_sdk::SdkSystemMessage::Init(init) = sys {
                        let _ = tx.unbounded_send(StreamEntry::SessionInit {
                            session_id: init.session_id,
                        });
                    }
                }
                SdkMessage::StreamEvent(event) => {
                    let event_type = event.event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "content_block_start" => {
                            // Check if this is a tool use block
                            if let Some(content_block) = event.event.get("content_block") {
                                let block_type =
                                    content_block.get("type").and_then(|t| t.as_str());
                                if block_type == Some("tool_use") {
                                    let tool_use_id = content_block
                                        .get("id")
                                        .and_then(|id| id.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let tool_name = content_block
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let _ = tx.unbounded_send(StreamEntry::ToolStart {
                                        tool_use_id,
                                        tool_name,
                                    });
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event.event.get("delta") {
                                let delta_type = delta.get("type").and_then(|t| t.as_str());
                                match delta_type {
                                    Some("text_delta") => {
                                        if let Some(text) =
                                            delta.get("text").and_then(|t| t.as_str())
                                        {
                                            let _ = tx.unbounded_send(StreamEntry::TextDelta {
                                                text: text.to_string(),
                                            });
                                        }
                                    }
                                    Some("input_json_delta") => {
                                        if let Some(partial_json) =
                                            delta.get("partial_json").and_then(|j| j.as_str())
                                        {
                                            let _ = tx.unbounded_send(StreamEntry::ToolInput {
                                                tool_use_id: String::new(),
                                                partial_json: partial_json.to_string(),
                                            });
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                SdkMessage::ToolProgress(progress) => {
                    let _ = tx.unbounded_send(StreamEntry::ToolProgress {
                        tool_use_id: progress.tool_use_id,
                        elapsed_seconds: progress.elapsed_time_seconds,
                    });
                }
                SdkMessage::User(user) => {
                    // Check if this is a tool result
                    if let Some(tool_use_id) = &user.parent_tool_use_id {
                        if let Some(result) = &user.tool_use_result {
                            let stdout = result
                                .get("stdout")
                                .and_then(|s| s.as_str())
                                .unwrap_or("")
                                .to_string();
                            let stderr = result
                                .get("stderr")
                                .and_then(|s| s.as_str())
                                .unwrap_or("");
                            let is_error = !stderr.is_empty();
                            let output = if is_error {
                                format!("{}\n{}", stdout, stderr)
                            } else {
                                stdout
                            };
                            let _ = tx.unbounded_send(StreamEntry::ToolResult {
                                tool_use_id: tool_use_id.clone(),
                                output,
                                is_error,
                            });
                        }
                    }
                }
                SdkMessage::Result(result) => {
                    let error = match result {
                        claude_agent_sdk::SdkResultMessage::Success(_) => None,
                        claude_agent_sdk::SdkResultMessage::ErrorDuringExecution(err)
                        | claude_agent_sdk::SdkResultMessage::ErrorMaxTurns(err)
                        | claude_agent_sdk::SdkResultMessage::ErrorMaxBudget(err)
                        | claude_agent_sdk::SdkResultMessage::ErrorMaxStructuredOutputRetries(
                            err,
                        ) => Some(format!("{:?}", err.errors)),
                    };
                    let _ = tx.unbounded_send(StreamEntry::Done { error });
                    return;
                }
                _ => {}
            }
        }

        // If stream ended without result, send done
        let _ = tx.unbounded_send(StreamEntry::Done { error: None });
    });

    Ok(JsonStream::new(rx))
}

#[component]
pub fn MechaCoder() -> Element {
    let mut entries = use_signal(|| {
        vec![ThreadEntry::Message(Message {
            id: 0,
            role: "assistant".to_string(),
            content: "Hello! I'm MechaCoder, your AI coding assistant powered by Claude. How can I help you today?".to_string(),
        })]
    });
    let mut input_value = use_signal(|| String::new());
    let mut is_loading = use_signal(|| false);
    let mut next_id = use_signal(|| 1usize);
    // Track current assistant message for streaming text
    let mut current_assistant_content = use_signal(|| String::new());
    // Track current tool for input streaming
    let mut current_tool_id = use_signal(|| Option::<String>::None);

    let mut send_message = move |_| {
        let content = input_value();
        if content.trim().is_empty() || is_loading() {
            return;
        }

        // Add user message
        let user_id = next_id();
        entries.write().push(ThreadEntry::Message(Message {
            id: user_id,
            role: "user".to_string(),
            content: content.clone(),
        }));
        next_id += 1;

        // Clear input and set loading
        let message = content.clone();
        input_value.set(String::new());
        is_loading.set(true);
        current_assistant_content.set(String::new());
        current_tool_id.set(None);

        // Send to server with streaming
        spawn(async move {
            // Use current directory as cwd (could be configured)
            let cwd = ".".to_string();

            match send_chat_message(message, cwd).await {
                Ok(stream) => {
                    let mut stream = stream.into_inner();

                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(entry) => match entry {
                                StreamEntry::SessionInit { .. } => {
                                    // Session initialized, could store session_id
                                }
                                StreamEntry::TextDelta { text } => {
                                    // Append text to current assistant content
                                    let mut content = current_assistant_content();
                                    content.push_str(&text);
                                    current_assistant_content.set(content);
                                }
                                StreamEntry::ToolStart {
                                    tool_use_id,
                                    tool_name,
                                } => {
                                    // Add a new tool entry
                                    current_tool_id.set(Some(tool_use_id.clone()));
                                    entries.write().push(ThreadEntry::ToolUse(ToolUse {
                                        tool_use_id,
                                        tool_name,
                                        input: String::new(),
                                        output: None,
                                        status: "running".to_string(),
                                    }));
                                }
                                StreamEntry::ToolInput { partial_json, .. } => {
                                    // Update current tool's input
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
                                StreamEntry::ToolProgress { tool_use_id, .. } => {
                                    // Update tool status (could show elapsed time)
                                    let mut entries_mut = entries.write();
                                    for entry in entries_mut.iter_mut().rev() {
                                        if let ThreadEntry::ToolUse(tool) = entry {
                                            if tool.tool_use_id == tool_use_id {
                                                tool.status = "running".to_string();
                                                break;
                                            }
                                        }
                                    }
                                }
                                StreamEntry::ToolResult {
                                    tool_use_id,
                                    output,
                                    is_error,
                                } => {
                                    // Update tool with result
                                    let mut entries_mut = entries.write();
                                    for entry in entries_mut.iter_mut().rev() {
                                        if let ThreadEntry::ToolUse(tool) = entry {
                                            if tool.tool_use_id == tool_use_id {
                                                tool.output = Some(output);
                                                tool.status = if is_error {
                                                    "failed".to_string()
                                                } else {
                                                    "completed".to_string()
                                                };
                                                break;
                                            }
                                        }
                                    }
                                }
                                StreamEntry::Done { error } => {
                                    // Add final assistant message if we have content
                                    let content = current_assistant_content();
                                    if !content.is_empty() {
                                        entries.write().push(ThreadEntry::Message(Message {
                                            id: next_id(),
                                            role: "assistant".to_string(),
                                            content,
                                        }));
                                        next_id += 1;
                                    }

                                    // Show error if any
                                    if let Some(err) = error {
                                        entries.write().push(ThreadEntry::Message(Message {
                                            id: next_id(),
                                            role: "assistant".to_string(),
                                            content: format!("Error: {}", err),
                                        }));
                                        next_id += 1;
                                    }
                                    break;
                                }
                            },
                            Err(e) => {
                                entries.write().push(ThreadEntry::Message(Message {
                                    id: next_id(),
                                    role: "assistant".to_string(),
                                    content: format!("Stream error: {}", e),
                                }));
                                next_id += 1;
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    entries.write().push(ThreadEntry::Message(Message {
                        id: next_id(),
                        role: "assistant".to_string(),
                        content: format!("Error: {}", e),
                    }));
                    next_id += 1;
                }
            }
            is_loading.set(false);
        });
    };

    rsx! {
        div {
            class: "flex flex-col h-screen bg-background",

            // Header
            header {
                class: "flex items-center justify-between px-6 py-4 border-b border-border",
                div {
                    class: "flex items-center gap-3",
                    Avatar {
                        class: "h-8 w-8",
                        AvatarFallback { "MC" }
                    }
                    h1 {
                        class: "text-xl font-semibold text-foreground",
                        "MechaCoder"
                    }
                }
                span {
                    class: "text-sm text-muted-foreground",
                    "AI Coding Assistant"
                }
            }

            // Messages area
            main {
                class: "flex-1 overflow-y-auto p-6 space-y-4",
                for entry in entries() {
                    match entry {
                        ThreadEntry::Message(msg) => rsx! {
                            MessageBubble {
                                key: "{msg.id}",
                                role: msg.role.clone(),
                                content: msg.content.clone(),
                            }
                        },
                        ThreadEntry::ToolUse(tool) => rsx! {
                            ToolCallBubble {
                                key: "{tool.tool_use_id}",
                                tool_name: tool.tool_name.clone(),
                                status: tool.status.clone(),
                                input: tool.input.clone(),
                            }
                        },
                    }
                }

                // Show streaming text if any
                if !current_assistant_content().is_empty() && is_loading() {
                    MessageBubble {
                        role: "assistant".to_string(),
                        content: current_assistant_content(),
                    }
                }

                // Loading indicator
                if is_loading() && current_assistant_content().is_empty() {
                    div {
                        class: "flex justify-start",
                        div {
                            class: "flex items-center gap-2 text-muted-foreground",
                            span { class: "animate-pulse", "..." }
                            span { "Claude is thinking" }
                        }
                    }
                }
            }

            // Input area
            footer {
                class: "border-t border-border p-4",
                form {
                    class: "flex gap-3 max-w-4xl mx-auto",
                    onsubmit: move |e| {
                        e.prevent_default();
                        send_message(());
                    },
                    input {
                        class: "flex-1 rounded border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50",
                        placeholder: "Type your message...",
                        value: input_value(),
                        disabled: is_loading(),
                        oninput: move |e| input_value.set(e.value()),
                    }
                    Button {
                        button_type: "submit",
                        variant: ButtonVariant::Primary,
                        disabled: is_loading(),
                        if is_loading() { "..." } else { "Send" }
                    }
                }
            }
        }
    }
}

#[component]
fn MessageBubble(role: String, content: String) -> Element {
    let is_user = role == "user";

    rsx! {
        div {
            class: if is_user { "flex justify-end" } else { "flex justify-start" },
            div {
                class: "flex items-start gap-3 max-w-[80%]",
                class: if is_user { "flex-row-reverse" } else { "" },

                Avatar {
                    class: "h-8 w-8 shrink-0",
                    AvatarFallback {
                        if is_user { "You" } else { "MC" }
                    }
                }

                div {
                    class: "rounded-lg px-4 py-2",
                    class: if is_user {
                        "bg-primary text-primary-foreground"
                    } else {
                        "bg-muted text-foreground"
                    },
                    // Render content with basic markdown-like formatting
                    div {
                        class: "whitespace-pre-wrap",
                        "{content}"
                    }
                }
            }
        }
    }
}

#[component]
fn ToolCallBubble(tool_name: String, status: String, input: String) -> Element {
    let status_color = match status.as_str() {
        "completed" => "text-green-500",
        "failed" => "text-red-500",
        "running" => "text-yellow-500",
        _ => "text-muted-foreground",
    };

    let status_icon = match status.as_str() {
        "completed" => "✓",
        "failed" => "✗",
        "running" => "...",
        _ => "○",
    };

    rsx! {
        div {
            class: "flex justify-start",
            div {
                class: "flex flex-col gap-1 px-3 py-2 rounded bg-muted/50 text-sm max-w-[80%]",
                div {
                    class: "flex items-center gap-2",
                    span {
                        class: "font-mono text-xs font-medium",
                        "{tool_name}"
                    }
                    span {
                        class: status_color,
                        "{status_icon}"
                    }
                }
                if !input.is_empty() {
                    div {
                        class: "font-mono text-xs text-muted-foreground truncate max-w-full",
                        title: "{input}",
                        "{input}"
                    }
                }
            }
        }
    }
}
