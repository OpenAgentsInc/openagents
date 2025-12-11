//! ACP connection management over stdio.
//!
//! This module provides the core connection handling for communicating with
//! Claude Code via the Agent Client Protocol over stdio.
//!
//! Adapted from Zed's crates/agent_servers/src/acp.rs

use agent_client_protocol::{self as acp, Agent};
use anyhow::{Context as _, Result};
use collections::HashMap;
use futures::io::BufReader;
use futures::AsyncBufReadExt as _;
use gpui::{App, AppContext, AsyncApp, Entity, SharedString, Task, WeakEntity};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::error::AcpError;
use crate::session::AcpThread;
use crate::types::{
    AgentConnection, AgentModelInfo, AgentModelList, AgentModelSelector, AgentSessionModes,
    Project, UserMessageId,
};

/// Minimum supported ACP protocol version.
const MINIMUM_SUPPORTED_VERSION: acp::ProtocolVersion = acp::ProtocolVersion::V1;

/// Session state for an ACP connection.
pub struct AcpSession {
    /// Weak reference to the thread entity.
    pub thread: WeakEntity<AcpThread>,
    /// Whether to suppress abort errors.
    pub suppress_abort_err: bool,
    /// Session modes state.
    pub session_modes: Option<Rc<RefCell<acp::SessionModeState>>>,
    /// Session models state.
    pub models: Option<Rc<RefCell<acp::SessionModelState>>>,
}

/// Connection to Claude Code via ACP protocol.
pub struct AcpConnection {
    /// Server name for display.
    server_name: SharedString,
    /// Telemetry identifier.
    telemetry_id: &'static str,
    /// The underlying ACP protocol connection.
    connection: Rc<acp::ClientSideConnection>,
    /// Active sessions.
    sessions: Rc<RefCell<HashMap<acp::SessionId, AcpSession>>>,
    /// Available authentication methods.
    auth_methods: Vec<acp::AuthMethod>,
    /// Agent capabilities received during initialization.
    agent_capabilities: acp::AgentCapabilities,
    /// Default mode to use.
    default_mode: Option<acp::SessionModeId>,
    /// Default model to use.
    default_model: Option<acp::ModelId>,
    /// Root directory for the connection.
    root_dir: PathBuf,
    /// The child process.
    child: smol::process::Child,
    /// Background task for I/O.
    _io_task: Task<Result<(), acp::Error>>,
    /// Background task for waiting on process exit.
    _wait_task: Task<Result<()>>,
    /// Background task for stderr logging.
    _stderr_task: Task<Result<()>>,
}

/// Command configuration for spawning Claude Code.
#[derive(Clone, Debug)]
pub struct AgentServerCommand {
    /// Path to the executable.
    pub path: PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Environment variables.
    pub env: Option<HashMap<String, String>>,
}

impl AcpConnection {
    /// Create a new ACP connection over stdio.
    ///
    /// This spawns the Claude Code process and establishes the ACP connection.
    pub async fn stdio(
        server_name: SharedString,
        telemetry_id: &'static str,
        command: AgentServerCommand,
        root_dir: &Path,
        default_mode: Option<acp::SessionModeId>,
        default_model: Option<acp::ModelId>,
        cx: &mut AsyncApp,
    ) -> Result<Self> {
        // Build the command
        let mut child_cmd = smol::process::Command::new(&command.path);
        child_cmd
            .args(command.args.iter().map(|arg| arg.as_str()))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(root_dir);

        if let Some(env) = &command.env {
            child_cmd.envs(env.iter());
        }

        let mut child = child_cmd.spawn().map_err(|e| {
            AcpError::SpawnFailed(format!(
                "Failed to spawn {:?}: {}",
                command.path, e
            ))
        })?;

        let stdout = child.stdout.take().context("Failed to take stdout")?;
        let stdin = child.stdin.take().context("Failed to take stdin")?;
        let stderr = child.stderr.take().context("Failed to take stderr")?;

        log::debug!(
            "Spawning external agent server: {:?}, {:?}",
            command.path,
            command.args
        );
        log::trace!("Spawned (pid: {})", child.id());

        let sessions = Rc::new(RefCell::new(HashMap::default()));

        // Create client delegate
        let client = ClientDelegate {
            sessions: sessions.clone(),
            cx: cx.clone(),
        };

        // Create the ACP connection
        let (connection, io_task) = acp::ClientSideConnection::new(client, stdin, stdout, {
            let foreground_executor = cx.foreground_executor().clone();
            move |fut| {
                foreground_executor.spawn(fut).detach();
            }
        });

        let io_task = cx.background_spawn(io_task);

        // Handle stderr logging
        let stderr_task = cx.background_spawn(async move {
            let mut stderr = BufReader::new(stderr);
            let mut line = String::new();
            while let Ok(n) = stderr.read_line(&mut line).await {
                if n == 0 {
                    break;
                }
                log::warn!("agent stderr: {}", line.trim());
                line.clear();
            }
            Ok(())
        });

        // Handle process exit
        let wait_task = cx.spawn({
            let sessions = sessions.clone();
            let status_fut = child.status();
            async move |_cx| {
                let status = status_fut.await?;
                log::info!("Agent process exited with status: {:?}", status);
                // Notify all sessions about the exit
                for session in sessions.borrow().values() {
                    if let Some(thread) = session.thread.upgrade() {
                        // TODO: Emit error to thread
                        log::warn!("Session thread still alive after agent exit");
                    }
                }
                anyhow::Ok(())
            }
        });

        let connection = Rc::new(connection);

        // Send initialize request
        let response = connection
            .initialize(
                acp::InitializeRequest::new(acp::ProtocolVersion::V1)
                    .client_capabilities(
                        acp::ClientCapabilities::new()
                            .fs(
                                acp::FileSystemCapability::new()
                                    .read_text_file(true)
                                    .write_text_file(true),
                            )
                            .terminal(true)
                            .meta(acp::Meta::from_iter([
                                ("terminal_output".into(), true.into()),
                                ("terminal-auth".into(), true.into()),
                            ])),
                    )
                    .client_info(acp::Implementation::new("mechacoder", "0.1.0")),
            )
            .await
            .map_err(|e| AcpError::Protocol(format!("Initialize failed: {}", e)))?;

        // Verify protocol version
        if response.protocol_version < MINIMUM_SUPPORTED_VERSION {
            return Err(AcpError::UnsupportedVersion.into());
        }

        Ok(Self {
            auth_methods: response.auth_methods,
            root_dir: root_dir.to_owned(),
            connection,
            server_name,
            telemetry_id,
            sessions,
            agent_capabilities: response.agent_capabilities,
            default_mode,
            default_model,
            _io_task: io_task,
            _wait_task: wait_task,
            _stderr_task: stderr_task,
            child,
        })
    }

    /// Get the prompt capabilities.
    pub fn prompt_capabilities(&self) -> &acp::PromptCapabilities {
        &self.agent_capabilities.prompt_capabilities
    }

    /// Get the root directory.
    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    /// Get the server name.
    pub fn server_name(&self) -> &SharedString {
        &self.server_name
    }
}

impl Drop for AcpConnection {
    fn drop(&mut self) {
        // Kill the child process on drop
        if let Err(e) = self.child.kill() {
            log::error!("Failed to kill agent process: {}", e);
        }
    }
}

impl AgentConnection for AcpConnection {
    fn telemetry_id(&self) -> &'static str {
        self.telemetry_id
    }

    fn auth_methods(&self) -> &[acp::AuthMethod] {
        &self.auth_methods
    }

    fn authenticate(&self, method_id: acp::AuthMethodId, cx: &mut App) -> Task<Result<()>> {
        let conn = self.connection.clone();
        cx.foreground_executor().spawn(async move {
            conn.authenticate(acp::AuthenticateRequest::new(method_id))
                .await?;
            Ok(())
        })
    }

    fn new_thread(
        self: Rc<Self>,
        project: Project,
        cwd: &Path,
        cx: &mut App,
    ) -> Task<Result<Entity<AcpThread>>> {
        let name = self.server_name.clone();
        let conn = self.connection.clone();
        let sessions = self.sessions.clone();
        let default_mode = self.default_mode.clone();
        let default_model = self.default_model.clone();
        let cwd = cwd.to_path_buf();
        let prompt_capabilities = self.agent_capabilities.prompt_capabilities.clone();

        cx.spawn(async move |cx| {
            // Create new session
            let response = conn
                .new_session(acp::NewSessionRequest::new(cwd.clone()))
                .await
                .map_err(|err| {
                    if err.code == acp::ErrorCode::AuthRequired {
                        AcpError::auth_required(err.message)
                    } else {
                        AcpError::Protocol(err.message)
                    }
                })?;

            let modes = response.modes.map(|modes| Rc::new(RefCell::new(modes)));
            let models = response.models.map(|models| Rc::new(RefCell::new(models)));

            // Set default mode if available
            if let Some(default_mode) = default_mode {
                if let Some(modes) = modes.as_ref() {
                    let has_mode = modes
                        .borrow()
                        .available_modes
                        .iter()
                        .any(|mode| mode.id == default_mode);
                    if has_mode {
                        let initial_mode_id = modes.borrow().current_mode_id.clone();
                        let session_id = response.session_id.clone();
                        let conn = conn.clone();
                        let modes_for_task = modes.clone();

                        cx.spawn({
                            let default_mode = default_mode.clone();
                            async move |_| {
                                let result = conn
                                    .set_session_mode(acp::SetSessionModeRequest::new(
                                        session_id,
                                        default_mode,
                                    ))
                                    .await;

                                if result.is_err() {
                                    modes_for_task.borrow_mut().current_mode_id = initial_mode_id;
                                }
                            }
                        })
                        .detach();

                        modes.borrow_mut().current_mode_id = default_mode;
                    }
                }
            }

            // Set default model if available
            if let Some(default_model) = default_model {
                if let Some(models) = models.as_ref() {
                    let has_model = models
                        .borrow()
                        .available_models
                        .iter()
                        .any(|model| model.model_id == default_model);
                    if has_model {
                        let initial_model_id = models.borrow().current_model_id.clone();
                        let session_id = response.session_id.clone();
                        let conn = conn.clone();
                        let models_for_task = models.clone();

                        cx.spawn({
                            let default_model = default_model.clone();
                            async move |_| {
                                let result = conn
                                    .set_session_model(acp::SetSessionModelRequest::new(
                                        session_id,
                                        default_model,
                                    ))
                                    .await;

                                if result.is_err() {
                                    models_for_task.borrow_mut().current_model_id = initial_model_id;
                                }
                            }
                        })
                        .detach();

                        models.borrow_mut().current_model_id = default_model;
                    }
                }
            }

            let session_id = response.session_id.clone();

            // Create the thread entity
            let thread = cx.new(|cx| {
                AcpThread::new(
                    name.clone(),
                    self.clone(),
                    project,
                    session_id.clone(),
                    prompt_capabilities,
                    cx,
                )
            })?;

            // Register the session
            let session = AcpSession {
                thread: thread.downgrade(),
                suppress_abort_err: false,
                session_modes: modes,
                models,
            };
            sessions.borrow_mut().insert(session_id, session);

            Ok(thread)
        })
    }

    fn prompt(
        &self,
        _id: Option<UserMessageId>,
        params: acp::PromptRequest,
        cx: &mut App,
    ) -> Task<Result<acp::PromptResponse>> {
        let conn = self.connection.clone();
        let sessions = self.sessions.clone();
        let session_id = params.session_id.clone();

        cx.foreground_executor().spawn(async move {
            let result = conn.prompt(params).await;

            let mut suppress_abort_err = false;
            if let Some(session) = sessions.borrow_mut().get_mut(&session_id) {
                suppress_abort_err = session.suppress_abort_err;
                session.suppress_abort_err = false;
            }

            match result {
                Ok(response) => Ok(response),
                Err(err) => {
                    if err.code == acp::ErrorCode::AuthRequired {
                        return Err(AcpError::auth_required(err.message).into());
                    }

                    if suppress_abort_err
                        && (err.message.contains("This operation was aborted")
                            || err.message.contains("The user aborted a request"))
                    {
                        Ok(acp::PromptResponse::new(acp::StopReason::Cancelled))
                    } else {
                        Err(AcpError::Protocol(err.message).into())
                    }
                }
            }
        })
    }

    fn cancel(&self, session_id: &acp::SessionId, cx: &mut App) {
        if let Some(session) = self.sessions.borrow_mut().get_mut(session_id) {
            session.suppress_abort_err = true;
        }
        let conn = self.connection.clone();
        let params = acp::CancelNotification::new(session_id.clone());
        cx.foreground_executor()
            .spawn(async move { conn.cancel(params).await })
            .detach();
    }

    fn session_modes(
        &self,
        session_id: &acp::SessionId,
        _cx: &App,
    ) -> Option<Rc<dyn AgentSessionModes>> {
        let sessions = self.sessions.borrow();
        let session = sessions.get(session_id)?;

        session.session_modes.as_ref().map(|modes| {
            Rc::new(AcpSessionModes {
                connection: self.connection.clone(),
                session_id: session_id.clone(),
                state: modes.clone(),
            }) as Rc<dyn AgentSessionModes>
        })
    }

    fn model_selector(&self, session_id: &acp::SessionId) -> Option<Rc<dyn AgentModelSelector>> {
        let sessions = self.sessions.borrow();
        let session = sessions.get(session_id)?;

        session.models.as_ref().map(|models| {
            Rc::new(AcpModelSelector {
                session_id: session_id.clone(),
                connection: self.connection.clone(),
                state: models.clone(),
            }) as Rc<dyn AgentModelSelector>
        })
    }

    fn into_any(self: Rc<Self>) -> Rc<dyn std::any::Any> {
        self
    }
}

/// Session modes implementation.
struct AcpSessionModes {
    session_id: acp::SessionId,
    connection: Rc<acp::ClientSideConnection>,
    state: Rc<RefCell<acp::SessionModeState>>,
}

impl AgentSessionModes for AcpSessionModes {
    fn current_mode(&self) -> acp::SessionModeId {
        self.state.borrow().current_mode_id.clone()
    }

    fn all_modes(&self) -> Vec<acp::SessionMode> {
        self.state.borrow().available_modes.clone()
    }

    fn set_mode(&self, mode_id: acp::SessionModeId, cx: &mut App) -> Task<Result<()>> {
        let connection = self.connection.clone();
        let session_id = self.session_id.clone();
        let old_mode_id = {
            let mut state = self.state.borrow_mut();
            let old = state.current_mode_id.clone();
            state.current_mode_id = mode_id.clone();
            old
        };
        let state = self.state.clone();

        cx.foreground_executor().spawn(async move {
            let result = connection
                .set_session_mode(acp::SetSessionModeRequest::new(session_id, mode_id))
                .await;

            if result.is_err() {
                state.borrow_mut().current_mode_id = old_mode_id;
            }

            result?;
            Ok(())
        })
    }
}

/// Model selector implementation.
struct AcpModelSelector {
    session_id: acp::SessionId,
    connection: Rc<acp::ClientSideConnection>,
    state: Rc<RefCell<acp::SessionModelState>>,
}

impl AgentModelSelector for AcpModelSelector {
    fn list_models(&self, _cx: &mut App) -> Task<Result<AgentModelList>> {
        Task::ready(Ok(AgentModelList::Flat(
            self.state
                .borrow()
                .available_models
                .clone()
                .into_iter()
                .map(AgentModelInfo::from)
                .collect(),
        )))
    }

    fn select_model(&self, model_id: acp::ModelId, cx: &mut App) -> Task<Result<()>> {
        let connection = self.connection.clone();
        let session_id = self.session_id.clone();
        let old_model_id = {
            let mut state = self.state.borrow_mut();
            let old = state.current_model_id.clone();
            state.current_model_id = model_id.clone();
            old
        };
        let state = self.state.clone();

        cx.foreground_executor().spawn(async move {
            let result = connection
                .set_session_model(acp::SetSessionModelRequest::new(session_id, model_id))
                .await;

            if result.is_err() {
                state.borrow_mut().current_model_id = old_model_id;
            }

            result?;
            Ok(())
        })
    }

    fn selected_model(&self, _cx: &mut App) -> Task<Result<AgentModelInfo>> {
        let state = self.state.borrow();
        Task::ready(
            state
                .available_models
                .iter()
                .find(|m| m.model_id == state.current_model_id)
                .cloned()
                .map(AgentModelInfo::from)
                .ok_or_else(|| anyhow::anyhow!("Model not found")),
        )
    }
}

/// Client delegate for handling ACP callbacks.
struct ClientDelegate {
    sessions: Rc<RefCell<HashMap<acp::SessionId, AcpSession>>>,
    cx: AsyncApp,
}

#[async_trait::async_trait(?Send)]
impl acp::Client for ClientDelegate {
    async fn request_permission(
        &self,
        arguments: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse, acp::Error> {
        let thread = {
            let sessions = self.sessions.borrow();
            let session = sessions
                .get(&arguments.session_id)
                .ok_or_else(|| acp::Error::internal_error().data("Session not found"))?;
            session.thread.clone()
        };

        // Request permission from the thread
        let task = thread
            .update(&mut self.cx.clone(), |thread, cx| {
                thread.request_permission(arguments.tool_call, arguments.options, cx)
            })
            .map_err(|e| acp::Error::internal_error().data(e.to_string()))??;

        let outcome = task.await;

        Ok(acp::RequestPermissionResponse::new(outcome))
    }

    async fn write_text_file(
        &self,
        arguments: acp::WriteTextFileRequest,
    ) -> Result<acp::WriteTextFileResponse, acp::Error> {
        // Simple file write - in production this would go through the thread
        std::fs::write(&arguments.path, &arguments.content)
            .map_err(|e| acp::Error::internal_error().data(format!("Failed to write file: {}", e)))?;

        Ok(acp::WriteTextFileResponse::default())
    }

    async fn read_text_file(
        &self,
        arguments: acp::ReadTextFileRequest,
    ) -> Result<acp::ReadTextFileResponse, acp::Error> {
        // Simple file read
        let content = std::fs::read_to_string(&arguments.path)
            .map_err(|e| acp::Error::internal_error().data(format!("Failed to read file: {}", e)))?;

        Ok(acp::ReadTextFileResponse::new(content))
    }

    async fn session_notification(
        &self,
        notification: acp::SessionNotification,
    ) -> Result<(), acp::Error> {
        let sessions = self.sessions.borrow();
        let session = sessions
            .get(&notification.session_id)
            .ok_or_else(|| acp::Error::internal_error().data("Session not found"))?;

        // Update mode if changed
        if let acp::SessionUpdate::CurrentModeUpdate(acp::CurrentModeUpdate {
            current_mode_id,
            ..
        }) = &notification.update
        {
            if let Some(session_modes) = &session.session_modes {
                session_modes.borrow_mut().current_mode_id = current_mode_id.clone();
            }
        }

        // Forward to the thread
        let result = session
            .thread
            .update(&mut self.cx.clone(), |thread, cx| {
                thread.handle_session_update(notification.update, cx)
            })
            .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

        result.map_err(|e| acp::Error::internal_error().data(e.to_string()))
    }

    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse, acp::Error> {
        let terminal_id = acp::TerminalId::new(uuid::Uuid::new_v4().to_string());

        // Register terminal with the thread
        let thread = {
            let sessions = self.sessions.borrow();
            let session = sessions
                .get(&args.session_id)
                .ok_or_else(|| acp::Error::internal_error().data("Session not found"))?;
            session.thread.clone()
        };

        thread
            .update(&mut self.cx.clone(), |thread, _cx| {
                thread.register_terminal(
                    terminal_id.clone(),
                    format!("{} {}", args.command, args.args.join(" ")),
                    args.cwd,
                );
            })
            .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

        Ok(acp::CreateTerminalResponse::new(terminal_id))
    }

    async fn kill_terminal_command(
        &self,
        _args: acp::KillTerminalCommandRequest,
    ) -> Result<acp::KillTerminalCommandResponse, acp::Error> {
        // TODO: Implement terminal killing
        Ok(acp::KillTerminalCommandResponse::default())
    }

    async fn ext_method(&self, _args: acp::ExtRequest) -> Result<acp::ExtResponse, acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn ext_notification(&self, _args: acp::ExtNotification) -> Result<(), acp::Error> {
        Err(acp::Error::method_not_found())
    }

    async fn release_terminal(
        &self,
        _args: acp::ReleaseTerminalRequest,
    ) -> Result<acp::ReleaseTerminalResponse, acp::Error> {
        // TODO: Implement terminal release
        Ok(acp::ReleaseTerminalResponse::default())
    }

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> Result<acp::TerminalOutputResponse, acp::Error> {
        let thread = {
            let sessions = self.sessions.borrow();
            let session = sessions
                .get(&args.session_id)
                .ok_or_else(|| acp::Error::internal_error().data("Session not found"))?;
            session.thread.clone()
        };

        let output = thread
            .read_with(&self.cx, |thread, _cx| {
                thread.get_terminal_output(&args.terminal_id)
            })
            .map_err(|e| acp::Error::internal_error().data(e.to_string()))?;

        Ok(acp::TerminalOutputResponse::new(output.unwrap_or_default(), false))
    }

    async fn wait_for_terminal_exit(
        &self,
        _args: acp::WaitForTerminalExitRequest,
    ) -> Result<acp::WaitForTerminalExitResponse, acp::Error> {
        // TODO: Implement terminal exit waiting
        Ok(acp::WaitForTerminalExitResponse::new(
            acp::TerminalExitStatus::new().exit_code(Some(0)),
        ))
    }
}
