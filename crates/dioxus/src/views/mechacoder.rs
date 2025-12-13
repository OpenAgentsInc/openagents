use dioxus::prelude::*;
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

/// Response from the chat server function
#[derive(Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub entries: Vec<ThreadEntry>,
    pub session_id: Option<String>,
    pub error: Option<String>,
}

/// Send a message to Claude via the SDK
#[cfg(feature = "server")]
#[post("/api/chat")]
pub async fn send_chat_message(message: String, cwd: String) -> Result<ChatResponse, ServerFnError> {
    use claude_agent_sdk::{query, QueryOptions, SdkMessage, SdkResultMessage};
    use futures::StreamExt;
    use std::path::PathBuf;

    let mut entries = Vec::new();
    let mut session_id = None;
    let mut assistant_content = String::new();

    // Create query options
    let options = QueryOptions::new()
        .cwd(PathBuf::from(&cwd))
        .dangerously_skip_permissions(true);

    // Create query stream
    let query_result = query(&message, options).await;
    let mut stream = match query_result {
        Ok(q) => q,
        Err(e) => {
            return Ok(ChatResponse {
                entries: vec![],
                session_id: None,
                error: Some(e.to_string()),
            });
        }
    };

    // Process stream
    while let Some(msg_result) = stream.next().await {
        let msg = match msg_result {
            Ok(m) => m,
            Err(e) => {
                return Ok(ChatResponse {
                    entries,
                    session_id,
                    error: Some(e.to_string()),
                });
            }
        };

        match msg {
            SdkMessage::System(sys) => {
                if let claude_agent_sdk::SdkSystemMessage::Init(init) = sys {
                    session_id = Some(init.session_id);
                }
            }
            SdkMessage::Assistant(assistant) => {
                // Extract text from assistant message
                if let Some(content) = assistant.message.get("content") {
                    if let Some(blocks) = content.as_array() {
                        for block in blocks {
                            if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                                    assistant_content.push_str(text);
                                }
                            }
                        }
                    }
                }
            }
            SdkMessage::StreamEvent(event) => {
                let event_type = event.event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if event_type == "content_block_delta" {
                    if let Some(delta) = event.event.get("delta") {
                        if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                assistant_content.push_str(text);
                            }
                        }
                    }
                }
            }
            SdkMessage::Result(result) => {
                // Add final assistant message
                if !assistant_content.is_empty() {
                    entries.push(ThreadEntry::Message(Message {
                        id: entries.len(),
                        role: "assistant".to_string(),
                        content: assistant_content.clone(),
                    }));
                }

                if let SdkResultMessage::Error(err) = result {
                    return Ok(ChatResponse {
                        entries,
                        session_id,
                        error: Some(format!("{:?}", err)),
                    });
                }
            }
            _ => {}
        }
    }

    // If stream ended without result, add what we have
    if !assistant_content.is_empty() && entries.is_empty() {
        entries.push(ThreadEntry::Message(Message {
            id: 0,
            role: "assistant".to_string(),
            content: assistant_content,
        }));
    }

    Ok(ChatResponse {
        entries,
        session_id,
        error: None,
    })
}

#[cfg(not(feature = "server"))]
#[post("/api/chat")]
pub async fn send_chat_message(_message: String, _cwd: String) -> Result<ChatResponse, ServerFnError> {
    // Client-side stub - actual call goes to server
    Err(ServerFnError::new("This should not be called on the client"))
}

#[component]
pub fn MechaCoder() -> Element {
    let mut entries = use_signal(|| vec![
        ThreadEntry::Message(Message {
            id: 0,
            role: "assistant".to_string(),
            content: "Hello! I'm MechaCoder, your AI coding assistant powered by Claude. How can I help you today?".to_string(),
        }),
    ]);
    let mut input_value = use_signal(|| String::new());
    let mut is_loading = use_signal(|| false);
    let mut next_id = use_signal(|| 1usize);

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

        // Send to server
        spawn(async move {
            // Use current directory as cwd (could be configured)
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string());

            match send_chat_message(message, cwd).await {
                Ok(response) => {
                    if let Some(error) = response.error {
                        // Add error message
                        entries.write().push(ThreadEntry::Message(Message {
                            id: next_id(),
                            role: "assistant".to_string(),
                            content: format!("Error: {}", error),
                        }));
                        next_id += 1;
                    } else {
                        // Add response entries
                        for entry in response.entries {
                            match entry {
                                ThreadEntry::Message(mut msg) => {
                                    msg.id = next_id();
                                    entries.write().push(ThreadEntry::Message(msg));
                                    next_id += 1;
                                }
                                ThreadEntry::ToolUse(tool) => {
                                    entries.write().push(ThreadEntry::ToolUse(tool));
                                }
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
                            }
                        },
                    }
                }

                // Loading indicator
                if is_loading() {
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
fn ToolCallBubble(tool_name: String, status: String) -> Element {
    let status_color = match status.as_str() {
        "completed" => "text-green-500",
        "failed" => "text-red-500",
        "running" => "text-yellow-500",
        _ => "text-muted-foreground",
    };

    rsx! {
        div {
            class: "flex justify-start",
            div {
                class: "flex items-center gap-2 px-3 py-1 rounded bg-muted/50 text-sm",
                span {
                    class: "font-mono text-xs",
                    "{tool_name}"
                }
                span {
                    class: status_color,
                    match status.as_str() {
                        "completed" => "✓",
                        "failed" => "✗",
                        "running" => "...",
                        _ => "○",
                    }
                }
            }
        }
    }
}
