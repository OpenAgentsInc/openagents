//! ChatService - the main API for chat operations.
//!
//! Provides a simple stream-based API for the UI to consume.

use crate::bridge::Bridge;
use crate::update::ChatUpdate;
use coder_agent::AgentRegistry;
use coder_domain::ids::SessionId;
use coder_domain::PermissionId;
use coder_permission::{PermissionManager, Response as PermissionResponse};
use coder_session::{AgentConfig, ProcessorConfig, PromptBuilder, Session};
use coder_storage::Storage;
use futures::stream::{Stream, StreamExt};
use llm::{CompletionRequest, Message, ProviderRegistry, Tool};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, RwLock};
use tool_registry::ToolRegistry;
use tracing::{info};

/// Chat stream type alias.
pub type ChatStream = Pin<Box<dyn Stream<Item = ChatUpdate> + Send>>;

/// Errors that can occur in the ChatService.
#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Session not found: {0}")]
    SessionNotFound(SessionId),

    #[error("Session is busy")]
    SessionBusy,

    #[error("Storage error: {0}")]
    Storage(#[from] coder_storage::StorageError),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Permission error: {0}")]
    Permission(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Configuration for the ChatService.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    /// Default working directory.
    pub working_directory: PathBuf,
    /// Database path for storage.
    pub database_path: PathBuf,
    /// Default agent ID.
    pub default_agent: String,
    /// Default model ID.
    pub default_model: String,
    /// Default provider ID.
    pub default_provider: String,
    /// Maximum turns in a conversation loop.
    pub max_turns: usize,
    /// Processor configuration.
    pub processor_config: ProcessorConfig,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            database_path: PathBuf::from("coder.db"),
            default_agent: "build".to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            default_provider: "anthropic".to_string(),
            max_turns: 50,
            processor_config: ProcessorConfig::default(),
        }
    }
}

impl ServiceConfig {
    /// Create config from environment variables.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(dir) = std::env::var("CODER_WORKING_DIR") {
            config.working_directory = PathBuf::from(dir);
        }

        if let Ok(db) = std::env::var("CODER_DATABASE") {
            config.database_path = PathBuf::from(db);
        }

        if let Ok(agent) = std::env::var("CODER_DEFAULT_AGENT") {
            config.default_agent = agent;
        }

        if let Ok(model) = std::env::var("CODER_DEFAULT_MODEL") {
            config.default_model = model;
        }

        if let Ok(provider) = std::env::var("CODER_DEFAULT_PROVIDER") {
            config.default_provider = provider;
        }

        config
    }
}

/// The ChatService provides the main API for chat operations.
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

struct ChatServiceInner {
    config: ServiceConfig,
    provider_registry: ProviderRegistry,
    tool_registry: Arc<ToolRegistry>,
    permission_manager: Arc<PermissionManager>,
    storage: Arc<Storage>,
    agent_registry: AgentRegistry,
    /// Active sessions.
    sessions: RwLock<HashMap<SessionId, Session>>,
}

impl ChatService {
    /// Create a new ChatService.
    pub async fn new(config: ServiceConfig) -> Result<Self, ServiceError> {
        // Initialize provider registry
        let provider_registry = ProviderRegistry::new();
        provider_registry
            .init_defaults()
            .await
            .map_err(|e| ServiceError::Provider(e.to_string()))?;

        // Initialize tool registry
        let tool_registry = Arc::new(ToolRegistry::with_standard_tools());

        // Initialize permission manager
        let permission_manager = Arc::new(PermissionManager::new());

        // Initialize storage
        let storage = Arc::new(
            Storage::open(&config.database_path)
                .map_err(ServiceError::Storage)?,
        );

        // Initialize agent registry
        let agent_registry = AgentRegistry::with_builtin_agents();

        info!(
            working_dir = %config.working_directory.display(),
            "ChatService initialized"
        );

        Ok(Self {
            inner: Arc::new(ChatServiceInner {
                config,
                provider_registry,
                tool_registry,
                permission_manager,
                storage,
                agent_registry,
                sessions: RwLock::new(HashMap::new()),
            }),
        })
    }

    /// Create a new session.
    pub async fn create_session(
        &self,
        working_directory: Option<PathBuf>,
    ) -> Result<Session, ServiceError> {
        let working_dir = working_directory.unwrap_or_else(|| self.inner.config.working_directory.clone());

        let agent_config = AgentConfig {
            agent_id: self.inner.config.default_agent.clone(),
            model_id: self.inner.config.default_model.clone(),
            provider_id: self.inner.config.default_provider.clone(),
            max_tokens: Some(8192),
            temperature: None,
        };

        let session = Session::new(working_dir).with_agent(agent_config);

        // Store in active sessions
        self.inner
            .sessions
            .write()
            .await
            .insert(session.id, session.clone());

        info!(session_id = %session.id, "Session created");

        Ok(session)
    }

    /// Send a message and get a stream of updates.
    ///
    /// This is the main entry point for chat operations.
    pub fn send_message(
        &self,
        session_id: SessionId,
        content: String,
    ) -> ChatStream {
        let inner = self.inner.clone();

        Box::pin(async_stream::stream! {
            // Get session
            let session = {
                let sessions = inner.sessions.read().await;
                match sessions.get(&session_id) {
                    Some(s) => s.clone(),
                    None => {
                        yield ChatUpdate::Error {
                            session_id,
                            message: "Session not found".into(),
                            code: Some("SESSION_NOT_FOUND".into()),
                            recoverable: false,
                        };
                        return;
                    }
                }
            };

            // Create update channel
            let (update_tx, mut update_rx) = mpsc::unbounded_channel();

            // Create bridge
            let bridge = Bridge::new(session_id, session.thread_id, update_tx.clone());

            // Emit session started
            bridge.emit_session_started();
            yield ChatUpdate::SessionStarted {
                session_id,
                thread_id: session.thread_id,
            };

            // Emit agent info
            bridge.emit_agent_info(
                &session.agent_config.agent_id,
                &session.agent_config.model_id,
                &session.agent_config.provider_id,
            );
            yield ChatUpdate::AgentInfo {
                session_id,
                agent_id: session.agent_config.agent_id.clone(),
                model_id: session.agent_config.model_id.clone(),
                provider_id: session.agent_config.provider_id.clone(),
            };

            // Get provider
            let provider = match inner.provider_registry.get(&session.agent_config.provider_id).await {
                Some(p) => p,
                None => {
                    yield ChatUpdate::Error {
                        session_id,
                        message: format!("Provider not found: {}", session.agent_config.provider_id),
                        code: Some("PROVIDER_NOT_FOUND".into()),
                        recoverable: false,
                    };
                    return;
                }
            };

            // Build system prompt
            let prompt = PromptBuilder::new(&session.working_directory, session.agent_config.clone())
                .build();

            // Build completion request
            let tools = inner.tool_registry.to_anthropic_tools();
            let llm_tools: Vec<Tool> = tools
                .into_iter()
                .map(|t| Tool::new(
                    t["name"].as_str().unwrap_or(""),
                    t["description"].as_str().unwrap_or(""),
                    t["input_schema"].clone(),
                ))
                .collect();

            let request = CompletionRequest::new(&session.agent_config.model_id)
                .system(&prompt)
                .message(Message::user(&content))
                .max_tokens(session.agent_config.max_tokens.unwrap_or(8192))
                .tools(llm_tools);

            // Start streaming
            let stream = match provider.stream(request).await {
                Ok(s) => s,
                Err(e) => {
                    yield ChatUpdate::Error {
                        session_id,
                        message: format!("Failed to start stream: {}", e),
                        code: Some("STREAM_ERROR".into()),
                        recoverable: false,
                    };
                    return;
                }
            };

            // Process stream events
            futures::pin_mut!(stream);

            let mut current_message_id = coder_domain::MessageId::new();
            let mut has_tool_use = false;

            // Emit message started
            yield ChatUpdate::MessageStarted {
                session_id,
                message_id: current_message_id,
                role: crate::update::MessageRole::Assistant,
            };

            while let Some(result) = stream.next().await {
                match result {
                    Ok(event) => {
                        match event {
                            llm::StreamEvent::TextDelta { delta, .. } => {
                                yield ChatUpdate::TextDelta {
                                    session_id,
                                    message_id: current_message_id,
                                    delta,
                                };
                            }
                            llm::StreamEvent::ReasoningDelta { delta, .. } => {
                                yield ChatUpdate::ReasoningDelta {
                                    session_id,
                                    message_id: current_message_id,
                                    delta,
                                };
                            }
                            llm::StreamEvent::ToolInputStart { id, tool_name } => {
                                has_tool_use = true;
                                yield ChatUpdate::ToolStarted {
                                    session_id,
                                    message_id: current_message_id,
                                    tool_call_id: id,
                                    tool_name,
                                };
                            }
                            llm::StreamEvent::ToolInputDelta { id, delta } => {
                                yield ChatUpdate::ToolInputDelta {
                                    session_id,
                                    tool_call_id: id,
                                    delta,
                                };
                            }
                            llm::StreamEvent::ToolCall { tool_call_id, tool_name, input, .. } => {
                                has_tool_use = true;
                                yield ChatUpdate::ToolExecuting {
                                    session_id,
                                    tool_call_id: tool_call_id.clone(),
                                    input: input.clone(),
                                };

                                // Execute tool
                                let ctx = tool_registry::ToolContext::new(&session.working_directory);
                                match inner.tool_registry.execute(&tool_name, input, &ctx).await {
                                    Ok(output) => {
                                        yield ChatUpdate::ToolCompleted {
                                            session_id,
                                            tool_call_id,
                                            output: output.content.clone(),
                                            is_error: false,
                                            duration_ms: 0, // TODO: track duration
                                        };
                                    }
                                    Err(e) => {
                                        yield ChatUpdate::ToolCompleted {
                                            session_id,
                                            tool_call_id,
                                            output: e.to_string(),
                                            is_error: true,
                                            duration_ms: 0,
                                        };
                                    }
                                }
                            }
                            llm::StreamEvent::Finish { finish_reason, usage, .. } => {
                                yield ChatUpdate::MessageCompleted {
                                    session_id,
                                    message_id: current_message_id,
                                    finish_reason: format!("{:?}", finish_reason),
                                };

                                yield ChatUpdate::UsageUpdate {
                                    session_id,
                                    total_tokens: usage.total_tokens(),
                                    cost_usd: 0.0, // TODO: calculate cost
                                };
                            }
                            llm::StreamEvent::Error { error } => {
                                yield ChatUpdate::Error {
                                    session_id,
                                    message: error.message,
                                    code: Some(error.code),
                                    recoverable: false,
                                };
                            }
                            _ => {
                                // Ignore other events (Start, TextStart, TextEnd, etc.)
                            }
                        }
                    }
                    Err(e) => {
                        yield ChatUpdate::Error {
                            session_id,
                            message: e.to_string(),
                            code: Some("STREAM_ERROR".into()),
                            recoverable: false,
                        };
                        break;
                    }
                }
            }

            // Session ended
            yield ChatUpdate::SessionEnded {
                session_id,
                success: true,
                error: None,
            };
        })
    }

    /// Respond to a permission request.
    pub async fn respond_permission(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: PermissionResponse,
    ) -> Result<(), ServiceError> {
        self.inner
            .permission_manager
            .respond(session_id, permission_id, response)
            .await
            .map_err(|e| ServiceError::Permission(e.to_string()))
    }

    /// Cancel an active session.
    pub async fn cancel(&self, session_id: SessionId) -> Result<(), ServiceError> {
        let mut sessions = self.inner.sessions.write().await;
        if sessions.remove(&session_id).is_none() {
            return Err(ServiceError::SessionNotFound(session_id));
        }
        info!(session_id = %session_id, "Session cancelled");
        Ok(())
    }

    /// Get the agent registry.
    pub fn agents(&self) -> &AgentRegistry {
        &self.inner.agent_registry
    }

    /// Get the storage.
    pub fn storage(&self) -> Arc<Storage> {
        self.inner.storage.clone()
    }

    /// Get a session by ID.
    pub async fn get_session(&self, session_id: SessionId) -> Option<Session> {
        self.inner.sessions.read().await.get(&session_id).cloned()
    }

    /// List active sessions.
    pub async fn list_sessions(&self) -> Vec<Session> {
        self.inner.sessions.read().await.values().cloned().collect()
    }
}

impl Clone for ChatService {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.default_agent, "build");
        assert_eq!(config.default_model, "claude-sonnet-4-20250514");
        assert_eq!(config.default_provider, "anthropic");
    }
}
