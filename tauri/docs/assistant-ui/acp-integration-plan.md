# ACP + assistant-ui Integration Plan

## Executive Summary

This document outlines the architecture for integrating Agent Client Protocol (ACP) with assistant-ui in our Tauri desktop application. We will create a Rust backend that manages ACP agent processes (Claude Code, Codex) and bridges them to the React frontend using assistant-ui components.

**Key Goals:**
- Replace basic Ollama chat with full ACP agent integration
- Support multiple agent types (Claude Code, Codex, future orchestrator)
- Enable rich tool calls, file operations, terminal access
- Provide multi-thread conversation management
- Maintain type safety across the stack (Rust ↔ TypeScript)

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    React Frontend (Tauri)                    │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐  │
│  │              assistant-ui Components                  │  │
│  │  - Thread (chat interface)                            │  │
│  │  - ThreadList (conversation sidebar)                  │  │
│  │  - Tool UI components                                 │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           │ Custom Runtime                   │
│                           ▼                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │        ACPRuntimeAdapter (TypeScript)                 │  │
│  │  - Implements ChatModelAdapter                        │  │
│  │  - Implements RemoteThreadListAdapter                 │  │
│  │  - Converts ACP types ↔ assistant-ui types            │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           │ Tauri Commands                   │
│                           ▼                                  │
└───────────────────────────────────────────────────────────┘
                            │
                 ┌──────────┴──────────┐
                 │   Tauri IPC Bridge   │
                 └──────────┬──────────┘
                            │
┌───────────────────────────▼─────────────────────────────────┐
│                    Rust Backend (Tauri)                      │
│                                                               │
│  ┌──────────────────────────────────────────────────────┐  │
│  │            ACP Agent Manager (Rust)                   │  │
│  │  - Spawns agent processes (stdio)                     │  │
│  │  - JSON-RPC 2.0 client implementation                 │  │
│  │  - Session lifecycle management                       │  │
│  │  - Tool call execution (fs/terminal services)         │  │
│  │  - Permission request handling                        │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           │ stdio pipes                      │
│                           ▼                                  │
│  ┌──────────────────────────────────────────────────────┐  │
│  │          Agent Processes (External)                   │  │
│  │  - claude-code (Claude Sonnet via Anthropic API)      │  │
│  │  - codex (Local/Remote LLM)                           │  │
│  │  - Future: orchestrator, custom agents                │  │
│  └──────────────────────────────────────────────────────┘  │
│                                                               │
└───────────────────────────────────────────────────────────┘
```

## Component Breakdown

### 1. Rust ACP Backend (Tauri)

#### 1.1 Crate Structure

```
src-tauri/
├── Cargo.toml
├── src/
│   ├── main.rs                      # Tauri app entry
│   ├── acp/
│   │   ├── mod.rs                   # Module exports
│   │   ├── client.rs                # ACP client implementation
│   │   ├── transport.rs             # stdio transport
│   │   ├── session_manager.rs       # Session state
│   │   ├── types.rs                 # Type conversions
│   │   └── services/
│   │       ├── fs.rs                # File system service
│   │       ├── terminal.rs          # Terminal service
│   │       └── permission.rs        # Permission dialogs
│   ├── commands.rs                  # Tauri commands
│   └── state.rs                     # Global app state
└── crates/
    └── agent-client-protocol/       # Copy from rust repo
        └── ...
```

#### 1.2 Dependencies

```toml
[dependencies]
tauri = { version = "2.0", features = ["dialog", "process", "shell"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
tokio-util = { version = "0.7", features = ["codec"] }
anyhow = "1"
thiserror = "1"
futures = "0.3"
uuid = { version = "1", features = ["v4", "serde"] }

# ACP Schema (local path or git)
agent-client-protocol-schema = { path = "../crates/agent-client-protocol" }

# Optional: SQLite for persistence
rusqlite = { version = "0.31", features = ["bundled"], optional = true }
```

#### 1.3 Core Types

```rust
use agent_client_protocol_schema::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub agent_type: AgentType,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentType {
    ClaudeCode,
    Codex,
    Orchestrator,
}

/// Session state
pub struct Session {
    pub id: SessionId,
    pub agent_type: AgentType,
    pub mode_id: SessionModeId,
    pub cwd: String,
    pub messages: Vec<SessionMessage>,
    pub active_tool_calls: HashMap<String, ToolCallState>,
}

/// Internal message representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub role: MessageRole,
    pub content: Vec<ContentBlock>,
    pub created_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Tool call tracking
pub struct ToolCallState {
    pub call: ToolCall,
    pub status: ToolCallStatus,
    pub permission_pending: bool,
}
```

#### 1.4 ACP Client Implementation

```rust
use tokio::process::{Command, Child, ChildStdin, ChildStdout};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, RwLock};
use agent_client_protocol_schema::*;

pub struct ACPClient {
    child: Child,
    stdin: ChildStdin,
    stdout_task: JoinHandle<()>,
    request_tx: mpsc::Sender<PendingRequest>,
    update_rx: mpsc::Receiver<SessionNotification>,
    next_id: AtomicU64,
}

struct PendingRequest {
    id: RequestId,
    responder: oneshot::Sender<Result<serde_json::Value>>,
}

impl ACPClient {
    /// Spawn agent process and initialize connection
    pub async fn spawn(config: &AgentConfig) -> Result<Self> {
        let mut child = Command::new(&config.command)
            .args(&config.args)
            .envs(config.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let (request_tx, mut request_rx) = mpsc::channel(32);
        let (update_tx, update_rx) = mpsc::channel(128);
        let (response_tx, mut response_rx) = mpsc::channel(32);

        // Background task: read stdout and route messages
        let stdout_task = tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if let Ok(msg) = serde_json::from_str::<JsonRpcMessage>(&line) {
                    match msg {
                        JsonRpcMessage::Response(resp) => {
                            response_tx.send(resp).await.ok();
                        }
                        JsonRpcMessage::Notification(notif) => {
                            if notif.method == "session/update" {
                                if let Ok(update) = serde_json::from_value(notif.params) {
                                    update_tx.send(update).await.ok();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        });

        let mut client = Self {
            child,
            stdin,
            stdout_task,
            request_tx,
            update_rx,
            next_id: AtomicU64::new(1),
        };

        // Send initialize request
        client.initialize().await?;

        Ok(client)
    }

    /// Initialize handshake
    async fn initialize(&mut self) -> Result<InitializeResponse> {
        let request = InitializeRequest {
            protocol_version: "0.7.0".to_string(),
            client_capabilities: ClientCapabilities {
                fs: Some(FileSystemCapability {
                    read_text_file: true,
                    write_text_file: true,
                    _meta: None,
                }),
                terminal: Some(true),
                _meta: None,
            },
            client_info: Some(Implementation {
                name: "openagents".to_string(),
                title: Some("OpenAgents".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
            _meta: None,
        };

        self.send_request("initialize", request).await
    }

    /// Create new session
    pub async fn new_session(
        &mut self,
        cwd: Option<String>,
        mode_id: Option<SessionModeId>,
    ) -> Result<SessionId> {
        let request = NewSessionRequest {
            cwd,
            mcp_servers: None,
            mode_id,
            _meta: None,
        };

        let response: NewSessionResponse =
            self.send_request("session/new", request).await?;

        Ok(response.session_id)
    }

    /// Send prompt to session
    pub async fn prompt(
        &mut self,
        session_id: SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<()> {
        let request = PromptRequest {
            session_id,
            prompt: content,
            _meta: None,
        };

        self.send_notification("session/prompt", request).await
    }

    /// Cancel running operation
    pub async fn cancel(&mut self, session_id: SessionId) -> Result<()> {
        let notification = CancelNotification {
            session_id,
            _meta: None,
        };

        self.send_notification("session/cancel", notification).await
    }

    /// Receive next session update
    pub async fn recv_update(&mut self) -> Option<SessionNotification> {
        self.update_rx.recv().await
    }

    /// Generic request sender
    async fn send_request<P, R>(&mut self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst) as i64);

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: method.to_string(),
            params: serde_json::to_value(params)?,
        };

        let json = serde_json::to_string(&request)?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        // Wait for response (with timeout)
        let (tx, rx) = oneshot::channel();
        self.request_tx.send(PendingRequest { id, responder: tx }).await?;

        let response = timeout(Duration::from_secs(30), rx).await??;
        let result: R = serde_json::from_value(response?)?;

        Ok(result)
    }

    /// Generic notification sender
    async fn send_notification<P>(&mut self, method: &str, params: P) -> Result<()>
    where
        P: Serialize,
    {
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: serde_json::to_value(params)?,
        };

        let json = serde_json::to_string(&notification)?;
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;

        Ok(())
    }
}

impl Drop for ACPClient {
    fn drop(&mut self) {
        self.child.kill().ok();
    }
}
```

#### 1.5 Session Manager

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<SessionId, Session>>>,
    clients: Arc<RwLock<HashMap<SessionId, ACPClient>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create new session
    pub async fn create_session(
        &self,
        agent_config: AgentConfig,
        cwd: Option<String>,
    ) -> Result<SessionId> {
        // Spawn agent
        let mut client = ACPClient::spawn(&agent_config).await?;

        // Create session
        let session_id = client.new_session(cwd.clone(), None).await?;

        // Store session state
        let session = Session {
            id: session_id.clone(),
            agent_type: agent_config.agent_type,
            mode_id: SessionModeId::DefaultMode,
            cwd: cwd.unwrap_or_else(|| std::env::current_dir()
                .unwrap()
                .to_string_lossy()
                .into_owned()),
            messages: Vec::new(),
            active_tool_calls: HashMap::new(),
        };

        self.sessions.write().await.insert(session_id.clone(), session);
        self.clients.write().await.insert(session_id.clone(), client);

        Ok(session_id)
    }

    /// Send prompt to session
    pub async fn prompt(
        &self,
        session_id: &SessionId,
        content: Vec<ContentBlock>,
    ) -> Result<()> {
        // Add user message to session
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions.get_mut(session_id)
                .ok_or_else(|| anyhow!("Session not found"))?;

            session.messages.push(SessionMessage {
                id: Uuid::new_v4().to_string(),
                role: MessageRole::User,
                content: content.clone(),
                created_at: Utc::now(),
                metadata: None,
            });
        }

        // Send to agent
        let mut clients = self.clients.write().await;
        let client = clients.get_mut(session_id)
            .ok_or_else(|| anyhow!("Client not found"))?;

        client.prompt(session_id.clone(), content).await
    }

    /// Handle session update
    pub async fn handle_update(
        &self,
        session_id: &SessionId,
        update: SessionUpdate,
    ) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(session_id)
            .ok_or_else(|| anyhow!("Session not found"))?;

        match update {
            SessionUpdate::AgentMessageChunk(chunk) => {
                // Accumulate text in current assistant message
                if let Some(last) = session.messages.last_mut() {
                    if last.role == MessageRole::Assistant {
                        last.content.push(chunk.content);
                    } else {
                        session.messages.push(SessionMessage {
                            id: Uuid::new_v4().to_string(),
                            role: MessageRole::Assistant,
                            content: vec![chunk.content],
                            created_at: Utc::now(),
                            metadata: None,
                        });
                    }
                } else {
                    session.messages.push(SessionMessage {
                        id: Uuid::new_v4().to_string(),
                        role: MessageRole::Assistant,
                        content: vec![chunk.content],
                        created_at: Utc::now(),
                        metadata: None,
                    });
                }
            }

            SessionUpdate::ToolCall(tool_call) => {
                session.active_tool_calls.insert(
                    tool_call.call_id.clone(),
                    ToolCallState {
                        call: ToolCall::from_wire(tool_call),
                        status: ToolCallStatus::Pending,
                        permission_pending: false,
                    },
                );
            }

            SessionUpdate::ToolCallUpdate(update) => {
                if let Some(state) = session.active_tool_calls.get_mut(&update.call_id) {
                    state.status = update.status;
                    // Update tool call in messages
                }
            }

            SessionUpdate::Plan(plan) => {
                // Store plan in session metadata
                // Could render in UI as thinking/planning display
            }

            _ => {
                // Handle other update types
            }
        }

        Ok(())
    }

    /// Get session state (for frontend)
    pub async fn get_session(&self, session_id: &SessionId) -> Option<Session> {
        self.sessions.read().await.get(session_id).cloned()
    }

    /// List all sessions
    pub async fn list_sessions(&self) -> Vec<SessionId> {
        self.sessions.read().await.keys().cloned().collect()
    }
}
```

#### 1.6 Tauri Commands

```rust
use tauri::State;

#[tauri::command]
async fn create_session(
    state: State<'_, Arc<SessionManager>>,
    agent_type: String,
    cwd: Option<String>,
) -> Result<String, String> {
    let agent_config = match agent_type.as_str() {
        "claude-code" => AgentConfig {
            agent_type: AgentType::ClaudeCode,
            command: "claude-code".to_string(),
            args: vec!["--json".to_string()],
            env: vec![],
            cwd: cwd.clone(),
        },
        "codex" => AgentConfig {
            agent_type: AgentType::Codex,
            command: "codex".to_string(),
            args: vec!["agent".to_string()],
            env: vec![],
            cwd: cwd.clone(),
        },
        _ => return Err("Unknown agent type".to_string()),
    };

    let session_id = state.create_session(agent_config, cwd).await
        .map_err(|e| e.to_string())?;

    Ok(session_id.to_string())
}

#[tauri::command]
async fn send_prompt(
    state: State<'_, Arc<SessionManager>>,
    session_id: String,
    text: String,
) -> Result<(), String> {
    let sid = SessionId::from(session_id);

    let content = vec![ContentBlock::Text(TextContent {
        type_: "text".to_string(),
        annotations: None,
        text,
        _meta: None,
    })];

    state.prompt(&sid, content).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cancel_session(
    state: State<'_, Arc<SessionManager>>,
    session_id: String,
) -> Result<(), String> {
    let sid = SessionId::from(session_id);

    let clients = state.clients.read().await;
    let client = clients.get(&sid)
        .ok_or_else(|| "Session not found".to_string())?;

    client.cancel(sid).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_session(
    state: State<'_, Arc<SessionManager>>,
    session_id: String,
) -> Result<Session, String> {
    let sid = SessionId::from(session_id);

    state.get_session(&sid).await
        .ok_or_else(|| "Session not found".to_string())
}

#[tauri::command]
async fn list_sessions(
    state: State<'_, Arc<SessionManager>>,
) -> Result<Vec<String>, String> {
    Ok(state.list_sessions().await
        .into_iter()
        .map(|id| id.to_string())
        .collect())
}
```

#### 1.7 Event Streaming to Frontend

```rust
use tauri::{AppHandle, Manager};

/// Background task to stream updates to frontend
pub async fn stream_updates(
    app: AppHandle,
    session_manager: Arc<SessionManager>,
) {
    loop {
        // Poll all active sessions for updates
        let clients = session_manager.clients.read().await;

        for (session_id, client) in clients.iter() {
            if let Some(update) = client.recv_update().await {
                // Handle update in session manager
                session_manager.handle_update(session_id, update.update.clone()).await.ok();

                // Emit to frontend
                app.emit_to(
                    &format!("session:{}", session_id),
                    "session-update",
                    update,
                ).ok();
            }
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}
```

### 2. TypeScript Frontend Integration

#### 2.1 Type Definitions

```typescript
// src/types/acp.ts

export interface ACPSession {
  id: string;
  agentType: "claude-code" | "codex" | "orchestrator";
  modeId: string;
  cwd: string;
  messages: ACPMessage[];
}

export interface ACPMessage {
  id: string;
  role: "user" | "assistant" | "system";
  content: ContentBlock[];
  createdAt: string;
  metadata?: any;
}

export type ContentBlock =
  | TextContent
  | ImageContent
  | AudioContent
  | ResourceLink
  | ResourceContent;

export interface TextContent {
  type: "text";
  text: string;
  annotations?: Annotations;
}

export interface ImageContent {
  type: "image";
  data: string; // base64
  mimeType: string;
  uri?: string;
  annotations?: Annotations;
}

export interface AudioContent {
  type: "audio";
  data: string; // base64
  mimeType: string;
  annotations?: Annotations;
}

export interface ResourceLink {
  type: "resource_link";
  name: string;
  uri: string;
  description?: string;
  mimeType?: string;
  size?: number;
  title?: string;
  annotations?: Annotations;
}

export interface ResourceContent {
  type: "resource";
  resource: {
    uri: string;
    text?: string;
    blob?: string; // base64
    mimeType?: string;
  };
  annotations?: Annotations;
}

export interface Annotations {
  audience?: ("assistant" | "user")[];
  priority?: number;
  lastModified?: string;
}

export interface ToolCall {
  callId: string;
  name: string;
  arguments?: Record<string, any>;
  status: "pending" | "in_progress" | "completed" | "failed";
  output?: any;
  error?: string;
}

export interface SessionUpdate {
  sessionId: string;
  update:
    | { type: "user_message_chunk"; content: ContentBlock }
    | { type: "agent_message_chunk"; content: ContentBlock }
    | { type: "agent_thought_chunk"; content: ContentBlock }
    | { type: "tool_call"; toolCall: ToolCall }
    | { type: "tool_call_update"; toolCall: ToolCall }
    | { type: "plan"; plan: Plan };
}

export interface Plan {
  entries: PlanEntry[];
}

export interface PlanEntry {
  content: string;
  priority: "high" | "medium" | "low";
  status: "pending" | "in_progress" | "completed";
}
```

#### 2.2 Tauri API Client

```typescript
// src/lib/tauri-acp.ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ACPSession, SessionUpdate } from "@/types/acp";

export class TauriACPClient {
  async createSession(
    agentType: "claude-code" | "codex",
    cwd?: string
  ): Promise<string> {
    return invoke("create_session", { agentType, cwd });
  }

  async sendPrompt(sessionId: string, text: string): Promise<void> {
    return invoke("send_prompt", { sessionId, text });
  }

  async cancelSession(sessionId: string): Promise<void> {
    return invoke("cancel_session", { sessionId });
  }

  async getSession(sessionId: string): Promise<ACPSession> {
    return invoke("get_session", { sessionId });
  }

  async listSessions(): Promise<string[]> {
    return invoke("list_sessions");
  }

  subscribeToUpdates(
    sessionId: string,
    callback: (update: SessionUpdate) => void
  ): () => void {
    const unlisten = listen<SessionUpdate>(
      `session:${sessionId}`,
      (event) => {
        callback(event.payload);
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }
}

export const tauriACP = new TauriACPClient();
```

#### 2.3 ACP to assistant-ui Type Conversion

```typescript
// src/lib/acp-converter.ts
import type {
  ThreadMessage,
  ThreadMessageLike,
  ThreadAssistantMessagePart,
  ThreadUserMessagePart,
} from "@assistant-ui/react";
import type { ACPMessage, ContentBlock, ToolCall } from "@/types/acp";

export function convertACPToThreadMessage(
  acpMessage: ACPMessage
): ThreadMessageLike {
  const baseMessage = {
    id: acpMessage.id,
    createdAt: new Date(acpMessage.createdAt),
  };

  if (acpMessage.role === "user") {
    return {
      ...baseMessage,
      role: "user" as const,
      content: acpMessage.content.map(convertContentToUserPart),
    };
  }

  if (acpMessage.role === "assistant") {
    return {
      ...baseMessage,
      role: "assistant" as const,
      content: acpMessage.content.map(convertContentToAssistantPart),
    };
  }

  // System messages
  return {
    ...baseMessage,
    role: "system" as const,
    content: acpMessage.content
      .filter((c) => c.type === "text")
      .map((c) => ({ type: "text" as const, text: (c as any).text })),
  };
}

function convertContentToUserPart(
  content: ContentBlock
): ThreadUserMessagePart {
  switch (content.type) {
    case "text":
      return { type: "text", text: content.text };

    case "image":
      return {
        type: "image",
        image: content.data.startsWith("data:")
          ? content.data
          : `data:${content.mimeType};base64,${content.data}`,
      };

    case "audio":
      // Map to custom audio part or text representation
      return {
        type: "text",
        text: `[Audio: ${content.mimeType}]`,
      };

    case "resource_link":
      return {
        type: "file",
        name: content.name,
        contentType: content.mimeType || "application/octet-stream",
        // assistant-ui expects file part for attachments
      } as any;

    default:
      return { type: "text", text: "[Unsupported content]" };
  }
}

function convertContentToAssistantPart(
  content: ContentBlock
): ThreadAssistantMessagePart {
  switch (content.type) {
    case "text":
      return { type: "text", text: content.text };

    case "image":
      return {
        type: "image",
        image: content.data.startsWith("data:")
          ? content.data
          : `data:${content.mimeType};base64,${content.data}`,
      };

    // Tool calls handled separately via ToolCall updates
    default:
      return { type: "text", text: "[Unsupported content]" };
  }
}

export function convertToolCallToPart(
  toolCall: ToolCall
): ThreadAssistantMessagePart {
  return {
    type: "tool-call",
    toolCallId: toolCall.callId,
    toolName: toolCall.name,
    args: toolCall.arguments || {},
    argsText: JSON.stringify(toolCall.arguments || {}),
    result: toolCall.output,
    isError: toolCall.status === "failed",
  };
}
```

#### 2.4 Custom Runtime Implementation

```typescript
// src/lib/acp-runtime.ts
import { useLocalRuntime } from "@assistant-ui/react";
import type { ChatModelAdapter } from "@assistant-ui/react";
import { tauriACP } from "./tauri-acp";
import { convertACPToThreadMessage, convertToolCallToPart } from "./acp-converter";
import { create } from "zustand";

interface ACPRuntimeState {
  sessionId: string | null;
  isRunning: boolean;
  currentToolCalls: Map<string, ToolCall>;
  accumulatedText: string;
  setSessionId: (id: string | null) => void;
  setRunning: (running: boolean) => void;
  addToolCall: (toolCall: ToolCall) => void;
  updateToolCall: (toolCall: ToolCall) => void;
  appendText: (text: string) => void;
  clearText: () => void;
}

const useACPStore = create<ACPRuntimeState>((set) => ({
  sessionId: null,
  isRunning: false,
  currentToolCalls: new Map(),
  accumulatedText: "",
  setSessionId: (id) => set({ sessionId: id }),
  setRunning: (running) => set({ isRunning: running }),
  addToolCall: (toolCall) =>
    set((state) => ({
      currentToolCalls: new Map(state.currentToolCalls).set(
        toolCall.callId,
        toolCall
      ),
    })),
  updateToolCall: (toolCall) =>
    set((state) => {
      const calls = new Map(state.currentToolCalls);
      calls.set(toolCall.callId, toolCall);
      return { currentToolCalls: calls };
    }),
  appendText: (text) =>
    set((state) => ({ accumulatedText: state.accumulatedText + text })),
  clearText: () => set({ accumulatedText: "" }),
}));

export function useACPRuntime(agentType: "claude-code" | "codex" = "claude-code") {
  const store = useACPStore();

  const adapter: ChatModelAdapter = {
    async *run({ messages, abortSignal }) {
      try {
        store.setRunning(true);
        store.clearText();
        store.currentToolCalls.clear();

        // Create session if needed
        let sessionId = store.sessionId;
        if (!sessionId) {
          sessionId = await tauriACP.createSession(agentType);
          store.setSessionId(sessionId);
        }

        // Extract last user message
        const lastMessage = messages[messages.length - 1];
        if (lastMessage.role !== "user") {
          throw new Error("Last message must be from user");
        }

        const userText = lastMessage.content
          .filter((p) => p.type === "text")
          .map((p) => (p as any).text)
          .join("\n");

        // Subscribe to updates
        const updates: SessionUpdate[] = [];
        const unsubscribe = tauriACP.subscribeToUpdates(sessionId, (update) => {
          updates.push(update);
        });

        try {
          // Send prompt
          await tauriACP.sendPrompt(sessionId, userText);

          // Stream updates
          while (!abortSignal.aborted) {
            if (updates.length > 0) {
              const update = updates.shift()!;

              switch (update.update.type) {
                case "agent_message_chunk": {
                  const content = update.update.content;
                  if (content.type === "text") {
                    store.appendText(content.text);
                    yield {
                      content: [
                        { type: "text", text: store.accumulatedText },
                      ],
                    };
                  }
                  break;
                }

                case "agent_thought_chunk": {
                  // Could render as reasoning/thinking
                  const content = update.update.content;
                  if (content.type === "text") {
                    // Option 1: Show as separate reasoning part
                    yield {
                      content: [
                        { type: "text", text: store.accumulatedText },
                        // Custom reasoning part (if supported)
                      ],
                    };
                  }
                  break;
                }

                case "tool_call": {
                  const toolCall = update.update.toolCall;
                  store.addToolCall(toolCall);

                  yield {
                    content: [
                      ...(store.accumulatedText
                        ? [{ type: "text" as const, text: store.accumulatedText }]
                        : []),
                      convertToolCallToPart(toolCall),
                    ],
                    status:
                      toolCall.status === "pending"
                        ? { type: "requires-action" as const, reason: "tool-calls" as const }
                        : undefined,
                  };
                  break;
                }

                case "tool_call_update": {
                  const toolCall = update.update.toolCall;
                  store.updateToolCall(toolCall);

                  // Rebuild content with all tool calls
                  const toolCallParts = Array.from(
                    store.currentToolCalls.values()
                  ).map(convertToolCallToPart);

                  yield {
                    content: [
                      ...(store.accumulatedText
                        ? [{ type: "text" as const, text: store.accumulatedText }]
                        : []),
                      ...toolCallParts,
                    ],
                  };
                  break;
                }

                case "plan": {
                  // Store plan in metadata for potential rendering
                  const plan = update.update.plan;
                  yield {
                    content: [{ type: "text", text: store.accumulatedText }],
                    metadata: {
                      custom: { plan },
                    },
                  };
                  break;
                }
              }
            } else {
              // Wait for next update
              await new Promise((resolve) => setTimeout(resolve, 50));
            }

            // Check if we're done (no more updates and stream ended)
            // This would need better completion detection from ACP
            if (updates.length === 0 && store.accumulatedText) {
              // Check if all tool calls are completed
              const allToolsComplete = Array.from(
                store.currentToolCalls.values()
              ).every(
                (tc) =>
                  tc.status === "completed" ||
                  tc.status === "failed"
              );

              if (allToolsComplete) {
                yield {
                  content: [
                    { type: "text", text: store.accumulatedText },
                    ...Array.from(store.currentToolCalls.values()).map(
                      convertToolCallToPart
                    ),
                  ],
                  status: { type: "complete", reason: "stop" },
                };
                break;
              }
            }
          }
        } finally {
          unsubscribe();
        }
      } catch (error) {
        if (!abortSignal.aborted) {
          console.error("ACP runtime error:", error);
          throw error;
        }
      } finally {
        store.setRunning(false);
      }
    },
  };

  return useLocalRuntime(adapter, {
    maxSteps: 10, // Allow up to 10 sequential tool calls
  });
}
```

#### 2.5 Thread List Integration

```typescript
// src/lib/acp-thread-list.ts
import { unstable_useRemoteThreadListRuntime } from "@assistant-ui/react";
import type { RemoteThreadListAdapter } from "@assistant-ui/react";
import { tauriACP } from "./tauri-acp";
import { useACPRuntime } from "./acp-runtime";

export function useACPThreadListRuntime(
  agentType: "claude-code" | "codex" = "claude-code"
) {
  const adapter: RemoteThreadListAdapter = {
    async list() {
      const sessionIds = await tauriACP.listSessions();

      return sessionIds.map((id) => ({
        id,
        status: "regular" as const,
        remoteId: id,
        metadata: {},
      }));
    },

    async initialize(localId: string) {
      // Create new session
      const sessionId = await tauriACP.createSession(agentType);
      return { remoteId: sessionId };
    },

    async rename(remoteId: string, title: string) {
      // Store title in session metadata
      // Would need to add this to Rust backend
      await invoke("update_session_title", { sessionId: remoteId, title });
    },

    async archive(remoteId: string) {
      // Mark session as archived
      await invoke("archive_session", { sessionId: remoteId });
    },

    async unarchive(remoteId: string) {
      await invoke("unarchive_session", { sessionId: remoteId });
    },

    async delete(remoteId: string) {
      await invoke("delete_session", { sessionId: remoteId });
    },

    async *generateTitle(remoteId: string, messages) {
      // Could use agent to generate title
      // For now, use first user message
      const firstUserMessage = messages.find((m) => m.role === "user");
      if (firstUserMessage) {
        const text = firstUserMessage.content
          .filter((p) => p.type === "text")
          .map((p) => (p as any).text)
          .join(" ")
          .slice(0, 50);

        yield text;
      } else {
        yield "New Chat";
      }
    },
  };

  return unstable_useRemoteThreadListRuntime({
    runtime: useACPRuntime(agentType),
    adapter,
  });
}
```

#### 2.6 Updated App Component

```typescript
// src/App.tsx
import "./App.css";
import { AssistantSidebar } from "@/components/assistant-ui/assistant-sidebar";
import { AssistantRuntimeProvider } from "@assistant-ui/react";
import { useACPThreadListRuntime } from "@/lib/acp-thread-list";

function App() {
  const runtime = useACPThreadListRuntime("claude-code");

  return (
    <AssistantRuntimeProvider runtime={runtime}>
      <div className="dark fixed inset-0 h-screen w-screen bg-zinc-900 text-white">
        <AssistantSidebar />
      </div>
    </AssistantRuntimeProvider>
  );
}

export default App;
```

### 3. Tool Call Handling

#### 3.1 Rust Service Implementations

```rust
// src/acp/services/fs.rs

use agent_client_protocol_schema::*;
use anyhow::Result;
use std::fs;
use std::path::Path;

pub struct FileSystemService;

impl FileSystemService {
    pub fn read_text_file(
        &self,
        request: ReadTextFileRequest,
    ) -> Result<ReadTextFileResponse> {
        let path = Path::new(&request.path);

        // Security: check if path is within allowed directories
        // TODO: implement path validation

        let content = fs::read_to_string(path)?;

        let lines: Vec<&str> = content.lines().collect();

        let start = request.start_line.unwrap_or(1).max(1) as usize;
        let end = request.end_line
            .map(|l| (l as usize).min(lines.len()))
            .unwrap_or(lines.len());

        let selected_lines = &lines[start - 1..end];
        let text = selected_lines.join("\n");

        Ok(ReadTextFileResponse {
            text,
            _meta: None,
        })
    }

    pub fn write_text_file(
        &self,
        request: WriteTextFileRequest,
    ) -> Result<WriteTextFileResponse> {
        let path = Path::new(&request.path);

        // Security: check permissions
        // TODO: implement permission checking

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(path, &request.text)?;

        Ok(WriteTextFileResponse {
            _meta: None,
        })
    }
}
```

```rust
// src/acp/services/terminal.rs

use agent_client_protocol_schema::*;
use anyhow::Result;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command as AsyncCommand;

pub struct TerminalService {
    terminals: Arc<Mutex<HashMap<TerminalId, TerminalHandle>>>,
}

struct TerminalHandle {
    child: Child,
    output: Arc<Mutex<String>>,
    exit_code: Option<i32>,
}

impl TerminalService {
    pub fn new() -> Self {
        Self {
            terminals: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn create(
        &self,
        request: CreateTerminalRequest,
    ) -> Result<CreateTerminalResponse> {
        let terminal_id = TerminalId::from(uuid::Uuid::new_v4().to_string());

        let mut cmd = AsyncCommand::new(&request.command);

        if let Some(args) = request.args {
            cmd.args(args);
        }

        if let Some(cwd) = request.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(env) = request.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        cmd.stdout(Stdio::piped())
           .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let output = Arc::new(Mutex::new(String::new()));
        let output_clone = output.clone();

        // Spawn task to read output
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut buffer = vec![0u8; 4096];

                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(text) = String::from_utf8(buffer[..n].to_vec()) {
                                output_clone.lock().unwrap().push_str(&text);
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        let handle = TerminalHandle {
            child,
            output,
            exit_code: None,
        };

        self.terminals.lock().unwrap().insert(terminal_id.clone(), handle);

        Ok(CreateTerminalResponse {
            terminal_id,
            _meta: None,
        })
    }

    pub fn output(
        &self,
        request: OutputTerminalRequest,
    ) -> Result<OutputTerminalResponse> {
        let terminals = self.terminals.lock().unwrap();
        let handle = terminals.get(&request.terminal_id)
            .ok_or_else(|| anyhow!("Terminal not found"))?;

        let output = handle.output.lock().unwrap().clone();

        Ok(OutputTerminalResponse {
            output,
            exit_code: handle.exit_code,
            _meta: None,
        })
    }

    pub fn kill(&self, request: KillTerminalRequest) -> Result<KillTerminalResponse> {
        let mut terminals = self.terminals.lock().unwrap();

        if let Some(mut handle) = terminals.get_mut(&request.terminal_id) {
            handle.child.kill()?;
        }

        Ok(KillTerminalResponse {
            _meta: None,
        })
    }

    pub fn release(&self, request: ReleaseTerminalRequest) -> Result<ReleaseTerminalResponse> {
        let mut terminals = self.terminals.lock().unwrap();
        terminals.remove(&request.terminal_id);

        Ok(ReleaseTerminalResponse {
            _meta: None,
        })
    }
}
```

#### 3.2 Permission Handling

```rust
// src/acp/services/permission.rs

use agent_client_protocol_schema::*;
use anyhow::Result;
use tauri::{AppHandle, Manager};

pub struct PermissionService {
    app: AppHandle,
}

impl PermissionService {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    pub async fn request_permission(
        &self,
        request: RequestPermissionRequest,
    ) -> Result<RequestPermissionResponse> {
        // Emit event to frontend to show permission dialog
        let (tx, rx) = oneshot::channel();

        self.app.emit_to(
            "permission-request",
            "permission-request",
            serde_json::to_value(&request)?,
        )?;

        // Store responder for frontend to call back
        // (This requires state management in Tauri)

        // Wait for user response
        let outcome = rx.await?;

        Ok(RequestPermissionResponse {
            outcome,
            _meta: None,
        })
    }
}
```

### 4. Implementation Phases

#### Phase 1: Basic ACP Integration (Week 1)
- [ ] Copy Rust ACP schema crate into `crates/agent-client-protocol/`
- [ ] Implement basic ACPClient with stdio transport
- [ ] Create SessionManager with simple state tracking
- [ ] Implement Tauri commands for create_session, send_prompt, get_session
- [ ] Build basic TypeScript adapter that converts ACP messages to assistant-ui
- [ ] Test with simple text prompts and responses

#### Phase 2: Streaming & Updates (Week 2)
- [ ] Implement background update polling task
- [ ] Add Tauri event emission for session updates
- [ ] Build streaming ChatModelAdapter with proper yielding
- [ ] Handle message chunk accumulation correctly
- [ ] Test with long responses and verify smooth streaming

#### Phase 3: Tool Calls (Week 3)
- [ ] Implement file system service (read/write)
- [ ] Implement terminal service (create/output/kill/release)
- [ ] Add tool call tracking in SessionManager
- [ ] Convert ACP tool calls to assistant-ui tool-call parts
- [ ] Build custom tool UI components for file operations
- [ ] Test with agents that use tools

#### Phase 4: Permissions & Human-in-Loop (Week 4)
- [ ] Implement permission request service
- [ ] Build permission dialog components in React
- [ ] Wire up permission responses back to agents
- [ ] Test with operations requiring approval
- [ ] Add allow-once/allow-always preference storage

#### Phase 5: Multi-Thread Support (Week 5)
- [ ] Implement RemoteThreadListAdapter
- [ ] Add thread persistence (SQLite or file-based)
- [ ] Build thread switching UI
- [ ] Implement thread archiving and deletion
- [ ] Add auto-title generation

#### Phase 6: Advanced Features (Week 6+)
- [ ] Add plan/thinking visualization
- [ ] Support for image/audio content blocks
- [ ] Implement resource links and embedded resources
- [ ] Add MCP server configuration UI
- [ ] Session mode switching (ask/architect/code)
- [ ] Multiple agent type support
- [ ] Agent process lifecycle management improvements

### 5. Testing Strategy

#### Unit Tests (Rust)
- ACPClient message serialization/deserialization
- SessionManager state updates
- Service implementations (fs, terminal, permission)
- Type conversions between ACP schema and internal types

#### Integration Tests (Rust)
- Full ACP handshake with mock agent
- Session create → prompt → response flow
- Tool call execution end-to-end
- Permission request/response cycle

#### E2E Tests (TypeScript)
- Create session and send prompt
- Receive and render streaming updates
- Execute tool calls from UI
- Switch between threads
- Archive and restore conversations

#### Manual Testing Scenarios
1. Start agent, create session, send simple prompt
2. Ask agent to read a file
3. Ask agent to execute a terminal command
4. Request operation requiring permission
5. Create multiple threads and switch between them
6. Test cancellation mid-stream
7. Test error handling (agent crash, network issues)

### 6. Open Questions & Decisions

#### Q1: Where to store session persistence?
**Options:**
- A) SQLite database in Tauri app data directory
- B) JSON files in user home directory
- C) No persistence (in-memory only for MVP)

**Recommendation:** Start with C for MVP, add A in Phase 5.

#### Q2: How to handle agent process lifecycle?
**Options:**
- A) One long-running agent per session
- B) Spawn agent per prompt, shut down after
- C) Agent pool with session routing

**Recommendation:** Start with A, most similar to current ACP model.

#### Q3: Should tools execute in Rust or be forwarded to frontend?
**Options:**
- A) All tools in Rust (fs, terminal, etc.)
- B) Some tools in Rust, some in frontend (e.g., UI interactions)
- C) All tools in frontend, Rust just proxies

**Recommendation:** A for fs/terminal, B for future UI tools.

#### Q4: How to handle multi-agent scenarios (orchestrator)?
**Recommendation:** Defer to Phase 6, focus on single agent first.

#### Q5: Content block rendering - which types to prioritize?
**Priority order:**
1. Text (must have)
2. Images (important for screenshots, diagrams)
3. Resource links (file references)
4. Audio (nice to have)
5. Embedded resources (advanced)

### 7. Migration Path

#### From Current Ollama Implementation
1. Keep Ollama adapter as fallback
2. Add agent selection UI (Ollama vs Claude Code vs Codex)
3. Gradually deprecate Ollama once ACP agents stable
4. Maintain compatibility with existing chat UI

#### Data Migration
- No migration needed (starting fresh)
- Could add import from Ollama chat history if desired

### 8. Performance Considerations

#### Rust Backend
- Use `tokio` async runtime throughout
- Avoid blocking operations in hot paths
- Buffer stdout reads efficiently
- Limit concurrent sessions (e.g., max 5)

#### Frontend
- Memoize message conversions
- Use virtual scrolling for long conversations
- Debounce rapid update events
- Lazy load thread list

#### IPC Overhead
- Batch multiple updates into single events when possible
- Use binary serialization (MessagePack) for large payloads if needed
- Profile Tauri command latency

### 9. Security Considerations

#### File System Access
- Validate all paths are within allowed directories
- Never allow write access to system directories
- Prompt user for permission on sensitive operations

#### Terminal Execution
- Limit to known safe commands or require approval
- Prevent shell injection via proper argument escaping
- Set resource limits (CPU, memory, execution time)

#### Agent Process Isolation
- Run agents with limited privileges
- Sandbox file system access if possible
- Monitor resource usage and kill runaway agents

#### Permission System
- Store "allow always" decisions securely
- Allow user to review and revoke permissions
- Log all sensitive operations for audit

### 10. Future Enhancements

#### Agent Marketplace
- Discover and install new agent types
- Share custom agent configurations
- Rate and review agents

#### Collaboration Features
- Share sessions with teammates
- Real-time co-piloting
- Session replay/debugging

#### Analytics & Telemetry
- Track tool call success rates
- Measure response latency
- User engagement metrics (opt-in)

#### Advanced UI
- Diff viewer for file changes
- Interactive plan visualization
- Graph view of tool call dependencies
- Timeline scrubbing for session replay

## Conclusion

This architecture provides a solid foundation for integrating ACP agents with assistant-ui in our Tauri application. The phased approach allows us to build incrementally, testing each component thoroughly before moving to the next.

**Key Success Metrics:**
- Successful handshake with claude-code and codex agents
- Smooth streaming of responses with no lag
- Reliable tool execution with proper error handling
- Multi-thread support with persistence
- Sub-100ms latency for user interactions

**Next Steps:**
1. Review this plan with team
2. Set up Rust ACP crate structure
3. Begin Phase 1 implementation
4. Create tracking issues for each phase
5. Schedule weekly sync to review progress
