//! ChatService - the main API for chat operations.
//!
//! Provides a simple stream-based API for the UI to consume.

use crate::bridge::Bridge;
use crate::update::ChatUpdate;
use coder_agent::{AgentDefinition, AgentPermission, AgentRegistry};
use coder_domain::PermissionId;
use coder_domain::ids::SessionId;
use coder_permission::{PermissionManager, Response as PermissionResponse};
use coder_session::{AgentConfig, ProcessorConfig, PromptBuilder, Session};
use coder_storage::Storage;
use futures::stream::{Stream, StreamExt};
use llm::{CompletionRequest, Message, ModelInfo, ProviderRegistry, Tool};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, mpsc};
use tool_registry::ToolRegistry;
use tracing::{info, warn};

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

/// Org-wide default agent/model/provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrgDefaults {
    pub agent_id: String,
    pub model_id: String,
    pub provider_id: String,
}

const ORG_DEFAULTS_KEY: [&str; 2] = ["org", "defaults"];

/// Agent capability details for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    pub agent_id: String,
    pub description: Option<String>,
    pub model_id: String,
    pub provider_id: String,
    pub temperature: Option<f32>,
    pub max_steps: Option<u32>,
    /// Tool overrides: name -> enabled (true) or disabled (false).
    pub tool_overrides: HashMap<String, bool>,
    /// Whether tools default to enabled when not listed in overrides.
    pub all_tools_enabled: bool,
    pub permissions: AgentPermission,
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
            default_model: "claude-sonnet-4-5-20250929".to_string(),
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
        let storage =
            Arc::new(Storage::open(&config.database_path).map_err(ServiceError::Storage)?);

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

    fn config_agent_config(&self) -> AgentConfig {
        AgentConfig {
            agent_id: self.inner.config.default_agent.clone(),
            model_id: self.inner.config.default_model.clone(),
            provider_id: self.inner.config.default_provider.clone(),
            max_tokens: Some(8192),
            temperature: None,
        }
    }

    fn org_defaults(&self) -> Option<OrgDefaults> {
        match self.inner.storage.get(&ORG_DEFAULTS_KEY) {
            Ok(value) => value,
            Err(e) => {
                warn!(error = %e, "Failed to load org defaults, falling back to config defaults");
                None
            }
        }
    }

    fn default_agent_config(&self) -> AgentConfig {
        let config_default = self.config_agent_config();

        if let Some(defaults) = self.org_defaults() {
            if self.inner.agent_registry.get(&defaults.agent_id).is_some() {
                return AgentConfig {
                    agent_id: defaults.agent_id,
                    model_id: defaults.model_id,
                    provider_id: defaults.provider_id,
                    max_tokens: config_default.max_tokens,
                    temperature: config_default.temperature,
                };
            }

            warn!(
                agent_id = %defaults.agent_id,
                "Org default agent not found; using configured defaults"
            );
        }

        config_default
    }

    fn capabilities_for(&self, agent: &AgentDefinition) -> AgentCapabilities {
        let config_default = self.config_agent_config();
        let (provider_id, model_id) = agent
            .model
            .as_ref()
            .map(|m| (m.provider_id.clone(), m.model_id.clone()))
            .unwrap_or_else(|| {
                (
                    config_default.provider_id.clone(),
                    config_default.model_id.clone(),
                )
            });

        AgentCapabilities {
            agent_id: agent.name.clone(),
            description: agent.description.clone(),
            model_id,
            provider_id,
            temperature: agent.temperature,
            max_steps: agent.max_steps,
            tool_overrides: agent.tools.clone().into_iter().collect(),
            all_tools_enabled: agent.tools.is_empty(),
            permissions: agent.permission.clone(),
        }
    }

    /// Get the saved org defaults, if present.
    pub fn get_org_defaults(&self) -> Result<Option<OrgDefaults>, ServiceError> {
        Ok(self.inner.storage.get(&ORG_DEFAULTS_KEY)?)
    }

    /// Set org-wide default agent/model/provider configuration.
    pub fn set_org_defaults(&self, defaults: OrgDefaults) -> Result<(), ServiceError> {
        if self.inner.agent_registry.get(&defaults.agent_id).is_none() {
            return Err(ServiceError::AgentNotFound(defaults.agent_id));
        }

        self.inner.storage.set(&ORG_DEFAULTS_KEY, &defaults)?;
        Ok(())
    }

    /// Create a new session.
    pub async fn create_session(
        &self,
        working_directory: Option<PathBuf>,
    ) -> Result<Session, ServiceError> {
        let agent_config = self.default_agent_config();
        self.create_session_with_agent(agent_config, working_directory)
            .await
    }

    /// Create a session with a specific agent/model configuration.
    pub async fn create_session_with_agent(
        &self,
        agent_config: AgentConfig,
        working_directory: Option<PathBuf>,
    ) -> Result<Session, ServiceError> {
        let working_dir =
            working_directory.unwrap_or_else(|| self.inner.config.working_directory.clone());

        let session = Session::new(working_dir).with_agent(agent_config);

        self.inner
            .sessions
            .write()
            .await
            .insert(session.id, session.clone());

        info!(session_id = %session.id, "Session created");

        Ok(session)
    }

    /// Switch the active agent (and optional model/provider) for an existing session.
    pub async fn switch_agent(
        &self,
        session_id: SessionId,
        agent_id: String,
        model_id: Option<String>,
        provider_id: Option<String>,
    ) -> Result<AgentConfig, ServiceError> {
        if self.inner.agent_registry.get(agent_id.as_str()).is_none() {
            return Err(ServiceError::AgentNotFound(agent_id));
        }

        let mut sessions = self.inner.sessions.write().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or(ServiceError::SessionNotFound(session_id))?;

        let updated_config = AgentConfig {
            agent_id: agent_id.clone(),
            model_id: model_id.unwrap_or_else(|| session.agent_config.model_id.clone()),
            provider_id: provider_id.unwrap_or_else(|| session.agent_config.provider_id.clone()),
            max_tokens: session.agent_config.max_tokens,
            temperature: session.agent_config.temperature,
        };

        session.agent_config = updated_config.clone();
        info!(
            session_id = %session_id,
            agent = %updated_config.agent_id,
            model = %updated_config.model_id,
            provider = %updated_config.provider_id,
            "Session agent switched"
        );

        Ok(updated_config)
    }

    /// List capabilities for primary agents.
    pub fn list_agent_capabilities(&self) -> Vec<AgentCapabilities> {
        self.inner
            .agent_registry
            .list_primary()
            .iter()
            .map(|agent| self.capabilities_for(agent))
            .collect()
    }

    /// Get capabilities for a specific agent.
    pub fn agent_capabilities(&self, agent_id: &str) -> Result<AgentCapabilities, ServiceError> {
        let agent = self
            .inner
            .agent_registry
            .get(agent_id)
            .ok_or_else(|| ServiceError::AgentNotFound(agent_id.to_string()))?;

        Ok(self.capabilities_for(&agent))
    }

    /// Send a message and get a stream of updates.
    ///
    /// This is the main entry point for chat operations.
    pub fn send_message(&self, session_id: SessionId, content: String) -> ChatStream {
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
            let (update_tx, _update_rx) = mpsc::unbounded_channel();

            // Create bridge
            let bridge = Bridge::new(session_id, session.thread_id, update_tx.clone());

            // Emit session started
            bridge.emit_session_started();
            yield ChatUpdate::SessionStarted {
                session_id,
                thread_id: session.thread_id,
            };

            // Resolve provider for this model
            let provider = match inner
                .provider_registry
                .get(&session.agent_config.provider_id)
                .await
            {
                Some(p) => Some(p),
                None => inner
                    .provider_registry
                    .provider_for_model(&session.agent_config.model_id)
                    .await,
            };

            let provider = match provider {
                Some(p) => p,
                None => {
                    yield ChatUpdate::Error {
                        session_id,
                        message: format!(
                            "Provider not found for model {}",
                            session.agent_config.model_id
                        ),
                        code: Some("PROVIDER_NOT_FOUND".into()),
                        recoverable: false,
                    };
                    return;
                }
            };

            let capabilities = provider.capabilities();
            let resolved_provider_id = provider.id().to_string();

            // Emit agent info
            bridge.emit_agent_info(
                &session.agent_config.agent_id,
                &session.agent_config.model_id,
                &resolved_provider_id,
            );
            yield ChatUpdate::AgentInfo {
                session_id,
                agent_id: session.agent_config.agent_id.clone(),
                model_id: session.agent_config.model_id.clone(),
                provider_id: resolved_provider_id.clone(),
            };

            // Build system prompt
            let prompt = PromptBuilder::new(&session.working_directory, session.agent_config.clone())
                .build();

            // Build completion request
            let mut request = CompletionRequest::new(&session.agent_config.model_id)
                .system(&prompt)
                .message(Message::user(&content));

            if let Some(max_tokens) = session.agent_config.max_tokens {
                request = request.max_tokens(max_tokens);
            }

            if let Some(temp) = session.agent_config.temperature {
                request = request.temperature(temp);
            }

            if capabilities.tool_calling {
                let tools = inner.tool_registry.to_anthropic_tools();
                let llm_tools: Vec<Tool> = tools
                    .into_iter()
                    .map(|t| {
                        Tool::new(
                            t["name"].as_str().unwrap_or(""),
                            t["description"].as_str().unwrap_or(""),
                            t["input_schema"].clone(),
                        )
                    })
                    .collect();

                request = request.tools(llm_tools);
            }

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

            let current_message_id = coder_domain::MessageId::new();

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

    /// List available models from registered providers.
    pub async fn list_models(&self) -> Vec<ModelInfo> {
        self.inner.provider_registry.list_models().await
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
    use tempfile::tempdir;

    #[test]
    fn test_service_config_default() {
        let config = ServiceConfig::default();
        assert_eq!(config.default_agent, "build");
        assert_eq!(config.default_model, "claude-sonnet-4-5-20250929");
        assert_eq!(config.default_provider, "anthropic");
    }

    #[tokio::test]
    async fn set_org_defaults_requires_known_agent() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let result = service.set_org_defaults(OrgDefaults {
            agent_id: "missing".to_string(),
            model_id: "gpt-4o".to_string(),
            provider_id: "openai".to_string(),
        });

        assert!(matches!(
            result,
            Err(ServiceError::AgentNotFound(agent)) if agent == "missing"
        ));
    }

    #[tokio::test]
    async fn uses_org_defaults_for_new_session() {
        let tmp = tempdir().unwrap();
        let defaults = OrgDefaults {
            agent_id: "build".to_string(),
            model_id: "gpt-4o".to_string(),
            provider_id: "openai".to_string(),
        };
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        service.set_org_defaults(defaults.clone()).unwrap();

        let session = service.create_session(None).await.unwrap();

        assert_eq!(session.agent_config.agent_id, defaults.agent_id);
        assert_eq!(session.agent_config.model_id, defaults.model_id);
        assert_eq!(session.agent_config.provider_id, defaults.provider_id);
        assert_eq!(service.get_org_defaults().unwrap(), Some(defaults));
    }

    #[tokio::test]
    async fn switch_agent_updates_session_config() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let session = service.create_session(None).await.unwrap();

        let updated = service
            .switch_agent(
                session.id,
                "plan".to_string(),
                Some("gpt-4o".to_string()),
                Some("openai".to_string()),
            )
            .await
            .unwrap();

        assert_eq!(updated.agent_id, "plan");
        assert_eq!(updated.model_id, "gpt-4o");
        assert_eq!(updated.provider_id, "openai");

        let stored = service.get_session(session.id).await.unwrap();
        assert_eq!(stored.agent_config.agent_id, "plan");
        assert_eq!(stored.agent_config.model_id, "gpt-4o");
        assert_eq!(stored.agent_config.provider_id, "openai");
        assert_eq!(
            stored.thread_id, session.thread_id,
            "thread context preserved"
        );
    }

    #[tokio::test]
    async fn switch_agent_validates_agent_exists() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let session = service.create_session(None).await.unwrap();

        let result = service
            .switch_agent(session.id, "missing".to_string(), None, None)
            .await;

        assert!(matches!(
            result,
            Err(ServiceError::AgentNotFound(agent)) if agent == "missing"
        ));

        let stored = service.get_session(session.id).await.unwrap();
        assert_eq!(stored.agent_config.agent_id, "build");
    }

    #[tokio::test]
    async fn switch_agent_errors_for_missing_session() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let missing_id = SessionId::new();

        let result = service
            .switch_agent(missing_id, "plan".to_string(), None, None)
            .await;

        assert!(matches!(result, Err(ServiceError::SessionNotFound(id)) if id == missing_id));
    }

    #[tokio::test]
    async fn agent_capabilities_available_for_primary_agents() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let caps = service.list_agent_capabilities();

        // Built-in primary agents include build and plan.
        let build = caps.iter().find(|c| c.agent_id == "build").unwrap();
        assert!(build.all_tools_enabled);
        assert_eq!(build.permissions.edit, coder_agent::Permission::Allow);

        let plan = caps.iter().find(|c| c.agent_id == "plan").unwrap();
        assert_eq!(plan.permissions.edit, coder_agent::Permission::Deny);
        assert!(
            plan.permissions
                .bash
                .get("*")
                .is_some_and(|perm| *perm == coder_agent::Permission::Ask)
        );
    }

    #[tokio::test]
    async fn get_agent_capabilities_includes_overrides() {
        let tmp = tempdir().unwrap();
        let config = ServiceConfig {
            working_directory: tmp.path().to_path_buf(),
            database_path: tmp.path().join("coder.db"),
            ..ServiceConfig::default()
        };

        let service = ChatService::new(config).await.unwrap();
        let general = service.agent_capabilities("general").unwrap();

        assert!(!general.tool_overrides.is_empty());
        assert!(
            general
                .tool_overrides
                .get("todoread")
                .is_some_and(|enabled| !enabled)
        );
        assert_eq!(general.model_id, service.config_agent_config().model_id);
    }
}
