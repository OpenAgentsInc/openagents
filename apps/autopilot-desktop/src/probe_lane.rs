use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(unix)]
use std::os::unix::net::UnixStream;

use probe_core::tools::{ProbeToolChoice, ToolDeniedAction, ToolLoopConfig};
use probe_protocol::backend::BackendProfile;
use probe_protocol::default_local_daemon_socket_path;
use probe_protocol::runtime::{
    CancelQueuedTurnRequest, CancelQueuedTurnResponse, ClientMessage, DetachedSessionEventPayload,
    EventEnvelope, InitializeRequest, InspectSessionTurnsResponse, InterruptTurnRequest,
    InterruptTurnResponse, QueueTurnResponse, QueuedTurnStatus, RequestEnvelope,
    ResolvePendingApprovalRequest, ResolvePendingApprovalResponse, ResponseBody, ResponseEnvelope,
    RuntimeProgressEvent, RuntimeRequest, RuntimeResponse, ServerEvent, ServerMessage,
    SessionLookupRequest, SessionSnapshot, StartSessionRequest, ToolApprovalRecipe, ToolChoice,
    ToolDeniedAction as ProtocolDeniedAction, ToolLongContextRecipe, ToolLoopRecipe,
    ToolOracleRecipe, ToolSetKind, TurnAuthor, TurnRequest, TurnResponse,
};
use probe_protocol::session::{
    PendingToolApproval, SessionBranchState, SessionChildSummary, SessionDeliveryState, SessionId,
    SessionMetadata, SessionMountRef, SessionWorkspaceState, ToolApprovalResolution,
};

const PROBE_LANE_POLL: Duration = Duration::from_millis(750);
const PROBE_DAEMON_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeCommandPlan {
    program: PathBuf,
    args: Vec<OsString>,
}

impl ProbeCommandPlan {
    fn into_command(self) -> Command {
        let mut command = Command::new(self.program);
        command.args(self.args);
        command
    }
}

#[derive(Clone, Debug)]
pub struct ProbeLaneConfig {
    pub probe_home: PathBuf,
    pub workspace_cwd: Option<PathBuf>,
    pub connect_on_startup: bool,
    pub client_name: String,
    pub client_version: Option<String>,
    pub server_binary: Option<PathBuf>,
    pub prefer_local_daemon: bool,
}

impl Default for ProbeLaneConfig {
    fn default() -> Self {
        Self {
            probe_home: default_probe_home_path(),
            workspace_cwd: std::env::current_dir().ok(),
            connect_on_startup: false,
            client_name: String::from("openagents-autopilot-desktop"),
            client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            server_binary: None,
            prefer_local_daemon: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeLaneLifecycle {
    Starting,
    Ready,
    Error,
    Disconnected,
    Stopped,
}

impl ProbeLaneLifecycle {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Ready => "ready",
            Self::Error => "error",
            Self::Disconnected => "disconnected",
            Self::Stopped => "stopped",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbeLaneSnapshot {
    pub lifecycle: ProbeLaneLifecycle,
    pub active_session_id: Option<String>,
    pub session_count: usize,
    pub last_error: Option<String>,
    pub last_status: Option<String>,
}

impl Default for ProbeLaneSnapshot {
    fn default() -> Self {
        Self {
            lifecycle: ProbeLaneLifecycle::Starting,
            active_session_id: None,
            session_count: 0,
            last_error: None,
            last_status: Some(String::from("Probe lane starting")),
        }
    }
}

impl ProbeLaneSnapshot {
    #[must_use]
    pub fn idle() -> Self {
        Self {
            lifecycle: ProbeLaneLifecycle::Stopped,
            active_session_id: None,
            session_count: 0,
            last_error: None,
            last_status: Some(String::from("Probe lane idle")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeLaneCommandKind {
    RefreshSessions,
    LoadSession,
    StartSession,
    RunTurn,
    QueueTurn,
    ResolvePendingApproval,
    InterruptTurn,
    CancelQueuedTurn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeLaneCommandStatus {
    Ok,
    Error,
}

#[derive(Clone, Debug)]
pub struct ProbeLaneCommandResponse {
    pub command_seq: u64,
    pub command: ProbeLaneCommandKind,
    pub status: ProbeLaneCommandStatus,
    pub session_id: Option<String>,
    pub message: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProbeLaneTurnSubmissionMode {
    Start,
    Continue,
}

#[derive(Clone, Debug)]
pub enum ProbeLaneCommand {
    RefreshSessions {
        workspace_cwd: Option<PathBuf>,
    },
    LoadSession {
        session_id: SessionId,
    },
    StartSession {
        title: Option<String>,
        cwd: PathBuf,
        profile: BackendProfile,
        system_prompt: Option<String>,
        mounted_refs: Vec<SessionMountRef>,
    },
    RunTurn {
        session_id: SessionId,
        prompt: String,
        profile: BackendProfile,
        tool_loop: Option<ToolLoopConfig>,
        submission_mode: ProbeLaneTurnSubmissionMode,
    },
    QueueTurn {
        session_id: SessionId,
        prompt: String,
        profile: BackendProfile,
        tool_loop: Option<ToolLoopConfig>,
    },
    ResolvePendingApproval {
        session_id: SessionId,
        call_id: String,
        resolution: ToolApprovalResolution,
        profile: BackendProfile,
        tool_loop: ToolLoopConfig,
    },
    InterruptTurn {
        session_id: SessionId,
    },
    CancelQueuedTurn {
        session_id: SessionId,
        turn_id: String,
    },
}

impl ProbeLaneCommand {
    #[must_use]
    pub const fn kind(&self) -> ProbeLaneCommandKind {
        match self {
            Self::RefreshSessions { .. } => ProbeLaneCommandKind::RefreshSessions,
            Self::LoadSession { .. } => ProbeLaneCommandKind::LoadSession,
            Self::StartSession { .. } => ProbeLaneCommandKind::StartSession,
            Self::RunTurn { .. } => ProbeLaneCommandKind::RunTurn,
            Self::QueueTurn { .. } => ProbeLaneCommandKind::QueueTurn,
            Self::ResolvePendingApproval { .. } => ProbeLaneCommandKind::ResolvePendingApproval,
            Self::InterruptTurn { .. } => ProbeLaneCommandKind::InterruptTurn,
            Self::CancelQueuedTurn { .. } => ProbeLaneCommandKind::CancelQueuedTurn,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProbeListedSession {
    pub session: SessionMetadata,
    pub control: Option<InspectSessionTurnsResponse>,
}

#[derive(Clone, Debug)]
pub enum ProbeLaneNotification {
    SessionsListed {
        sessions: Vec<ProbeListedSession>,
        workspace_session_id: Option<String>,
        workspace_collision_session_ids: Vec<String>,
    },
    SessionLoaded {
        snapshot: SessionSnapshot,
        control: InspectSessionTurnsResponse,
    },
    ChildSessionUpdated {
        session_id: String,
        child: SessionChildSummary,
    },
    WorkspaceStateUpdated {
        session_id: String,
        workspace_state: Option<SessionWorkspaceState>,
        branch_state: Option<SessionBranchState>,
        delivery_state: Option<SessionDeliveryState>,
    },
    RuntimeProgress {
        session_id: String,
        event: RuntimeProgressEvent,
    },
    PendingApprovalsUpdated {
        session_id: String,
        approvals: Vec<PendingToolApproval>,
    },
    TurnQueued {
        response: QueueTurnResponse,
        control: InspectSessionTurnsResponse,
    },
    TurnInterrupted {
        response: InterruptTurnResponse,
        control: InspectSessionTurnsResponse,
    },
    QueuedTurnCancelled {
        response: CancelQueuedTurnResponse,
        control: InspectSessionTurnsResponse,
    },
}

#[derive(Clone, Debug)]
pub enum ProbeLaneUpdate {
    Snapshot(Box<ProbeLaneSnapshot>),
    CommandResponse(ProbeLaneCommandResponse),
    Notification(ProbeLaneNotification),
}

struct SequencedProbeCommand {
    command_seq: u64,
    command: ProbeLaneCommand,
}

enum ProbeLaneControl {
    Command(SequencedProbeCommand),
    Shutdown,
}

#[derive(Clone)]
struct ProbeTransportHandle {
    inner: Arc<ProbeTransportInner>,
}

struct ProbeTransportInner {
    stdin: Mutex<Box<dyn Write + Send>>,
    pending: Mutex<HashMap<String, Sender<ProbeTransportMessage>>>,
    next_request_id: AtomicU64,
}

enum WorkspaceSessionSelection<'a> {
    None,
    Match(&'a SessionId),
    Collision(Vec<&'a SessionId>),
}

enum ProbeTransportMessage {
    Event(ServerEvent),
    Response(ResponseBody),
    Closed(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProbeTransportKind {
    LocalDaemon,
    SpawnedStdio,
}

impl ProbeTransportKind {
    const fn connected_status(self) -> &'static str {
        match self {
            Self::LocalDaemon => "Probe lane connected via local daemon",
            Self::SpawnedStdio => "Probe lane connected via direct server fallback",
        }
    }
}

struct ProbeTransport {
    kind: ProbeTransportKind,
    handle: ProbeTransportHandle,
    child: Option<Child>,
    reader_join: Option<JoinHandle<()>>,
    shutdown_on_drop: bool,
}

struct ProbeRequestStream {
    request_id: String,
    rx: Receiver<ProbeTransportMessage>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProbeSessionWatchSignature {
    updated_at_ms: u64,
    active_turn: Option<(String, QueuedTurnStatus, bool)>,
    queued_turns: Vec<(String, QueuedTurnStatus)>,
    recent_turns: Vec<(String, QueuedTurnStatus)>,
    pending_approval_ids: Vec<String>,
}

pub struct ProbeLaneWorker {
    command_tx: Sender<ProbeLaneControl>,
    update_rx: Receiver<ProbeLaneUpdate>,
    join_handle: Option<JoinHandle<()>>,
    shutdown_sent: bool,
}

impl ProbeLaneWorker {
    #[must_use]
    pub fn spawn(config: ProbeLaneConfig) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<ProbeLaneControl>();
        let (update_tx, update_rx) = mpsc::channel::<ProbeLaneUpdate>();
        let join_handle = thread::spawn(move || {
            run_probe_lane_loop(command_rx, update_tx, config);
        });

        Self {
            command_tx,
            update_rx,
            join_handle: Some(join_handle),
            shutdown_sent: false,
        }
    }

    pub fn enqueue(&self, command_seq: u64, command: ProbeLaneCommand) -> Result<(), String> {
        self.command_tx
            .send(ProbeLaneControl::Command(SequencedProbeCommand {
                command_seq,
                command,
            }))
            .map_err(|error| format!("Probe lane offline: {error}"))
    }

    #[must_use]
    pub fn drain_updates(&mut self) -> Vec<ProbeLaneUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown(&mut self) {
        if self.shutdown_sent {
            return;
        }
        self.shutdown_sent = true;
        let _ = self.command_tx.send(ProbeLaneControl::Shutdown);
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }

    pub fn shutdown_async(&mut self) {
        if self.shutdown_sent {
            return;
        }
        self.shutdown_sent = true;
        let _ = self.command_tx.send(ProbeLaneControl::Shutdown);
        if let Some(join_handle) = self.join_handle.take() {
            thread::spawn(move || {
                let _ = join_handle.join();
            });
        }
    }
}

impl Drop for ProbeLaneWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn run_probe_lane_loop(
    command_rx: Receiver<ProbeLaneControl>,
    update_tx: Sender<ProbeLaneUpdate>,
    config: ProbeLaneConfig,
) {
    let mut snapshot = ProbeLaneSnapshot::default();
    let mut watched_sessions = HashMap::<String, ProbeSessionWatchSignature>::new();
    let mut transport = establish_probe_transport(&config, &mut snapshot, &update_tx);

    if config.connect_on_startup {
        if let Some(current_transport) = transport.as_ref() {
            let _ = refresh_sessions_internal(
                current_transport.handle(),
                &config.workspace_cwd,
                &mut snapshot,
                &update_tx,
                &mut watched_sessions,
            );
            let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
        }
    }

    loop {
        match command_rx.recv_timeout(PROBE_LANE_POLL) {
            Ok(ProbeLaneControl::Shutdown) => break,
            Ok(ProbeLaneControl::Command(command)) => {
                if transport.is_none() {
                    transport = establish_probe_transport(&config, &mut snapshot, &update_tx);
                }
                let Some(current_transport) = transport.as_ref() else {
                    let error = String::from("Probe lane is not connected");
                    snapshot.lifecycle = ProbeLaneLifecycle::Error;
                    snapshot.last_error = Some(error.clone());
                    snapshot.last_status = Some(String::from("Probe lane disconnected"));
                    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                    let _ = update_tx.send(ProbeLaneUpdate::CommandResponse(
                        ProbeLaneCommandResponse {
                            command_seq: command.command_seq,
                            command: command.command.kind(),
                            status: ProbeLaneCommandStatus::Error,
                            session_id: None,
                            message: None,
                            error: Some(error),
                        },
                    ));
                    continue;
                };
                handle_probe_command(
                    current_transport.handle(),
                    command,
                    &config,
                    &mut snapshot,
                    &update_tx,
                    &mut watched_sessions,
                );
            }
            Err(RecvTimeoutError::Timeout) => {
                let Some(current_transport) = transport.as_ref() else {
                    continue;
                };
                let mut removed = Vec::new();
                for session_id in watched_sessions.keys().cloned().collect::<Vec<_>>() {
                    match inspect_session_bundle(
                        current_transport.handle(),
                        &SessionId::new(session_id.clone()),
                    ) {
                        Ok((snapshot_value, control)) => {
                            let next_signature =
                                probe_session_watch_signature(&snapshot_value, &control);
                            let previous = watched_sessions.get(&session_id);
                            if previous != Some(&next_signature) {
                                watched_sessions.insert(session_id.clone(), next_signature);
                                snapshot.active_session_id = Some(session_id.clone());
                                snapshot.last_status =
                                    Some(format!("Probe session {}", session_id));
                                snapshot.last_error = None;
                                let _ = update_tx.send(ProbeLaneUpdate::Notification(
                                    ProbeLaneNotification::SessionLoaded {
                                        snapshot: snapshot_value,
                                        control,
                                    },
                                ));
                                let _ = update_tx
                                    .send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                            }
                        }
                        Err(error) => {
                            snapshot.last_error = Some(error.clone());
                            snapshot.last_status = Some(String::from("Probe session poll failed"));
                            snapshot.lifecycle = ProbeLaneLifecycle::Disconnected;
                            removed.push(session_id.clone());
                            if let Some(mut current_transport) = transport.take() {
                                current_transport.shutdown();
                            }
                            let _ = update_tx
                                .send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                            break;
                        }
                    }
                }
                for session_id in removed {
                    watched_sessions.remove(&session_id);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    if let Some(mut transport) = transport.take() {
        transport.shutdown();
    }
}

fn handle_probe_command(
    transport: ProbeTransportHandle,
    sequenced: SequencedProbeCommand,
    config: &ProbeLaneConfig,
    snapshot: &mut ProbeLaneSnapshot,
    update_tx: &Sender<ProbeLaneUpdate>,
    watched_sessions: &mut HashMap<String, ProbeSessionWatchSignature>,
) {
    match sequenced.command {
        ProbeLaneCommand::RefreshSessions { workspace_cwd } => match refresh_sessions_internal(
            transport,
            &workspace_cwd,
            snapshot,
            update_tx,
            watched_sessions,
        ) {
            Ok(message) => send_probe_command_response(
                update_tx,
                sequenced.command_seq,
                ProbeLaneCommandKind::RefreshSessions,
                ProbeLaneCommandStatus::Ok,
                snapshot.active_session_id.clone(),
                Some(message),
                None,
            ),
            Err(error) => send_probe_command_response(
                update_tx,
                sequenced.command_seq,
                ProbeLaneCommandKind::RefreshSessions,
                ProbeLaneCommandStatus::Error,
                None,
                None,
                Some(error),
            ),
        },
        ProbeLaneCommand::LoadSession { session_id } => {
            let session_id_label = session_id.as_str().to_string();
            match inspect_session_bundle(transport, &session_id) {
                Ok((session_snapshot, control)) => {
                    snapshot.active_session_id = Some(session_id_label.clone());
                    snapshot.last_status =
                        Some(format!("Attached Probe session {session_id_label}"));
                    snapshot.last_error = None;
                    watched_sessions.insert(
                        session_id_label.clone(),
                        probe_session_watch_signature(&session_snapshot, &control),
                    );
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::SessionLoaded {
                            snapshot: session_snapshot,
                            control,
                        },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                    send_probe_command_response(
                        update_tx,
                        sequenced.command_seq,
                        ProbeLaneCommandKind::LoadSession,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe session attached")),
                        None,
                    );
                }
                Err(error) => send_probe_command_response(
                    update_tx,
                    sequenced.command_seq,
                    ProbeLaneCommandKind::LoadSession,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                ),
            }
        }
        ProbeLaneCommand::StartSession {
            title,
            cwd,
            profile,
            system_prompt,
            mounted_refs,
        } => match start_session_request(
            transport,
            &title,
            cwd,
            profile,
            system_prompt,
            mounted_refs,
        ) {
            Ok((session_snapshot, control)) => {
                let session_id = session_snapshot.session.id.as_str().to_string();
                snapshot.active_session_id = Some(session_id.clone());
                snapshot.session_count = snapshot.session_count.saturating_add(1);
                snapshot.last_status = Some(format!("Created Probe session {session_id}"));
                snapshot.last_error = None;
                watched_sessions.insert(
                    session_id.clone(),
                    probe_session_watch_signature(&session_snapshot, &control),
                );
                let _ = update_tx.send(ProbeLaneUpdate::Notification(
                    ProbeLaneNotification::SessionLoaded {
                        snapshot: session_snapshot,
                        control,
                    },
                ));
                let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                send_probe_command_response(
                    update_tx,
                    sequenced.command_seq,
                    ProbeLaneCommandKind::StartSession,
                    ProbeLaneCommandStatus::Ok,
                    Some(session_id),
                    Some(String::from("Probe session created")),
                    None,
                );
            }
            Err(error) => send_probe_command_response(
                update_tx,
                sequenced.command_seq,
                ProbeLaneCommandKind::StartSession,
                ProbeLaneCommandStatus::Error,
                None,
                None,
                Some(error),
            ),
        },
        ProbeLaneCommand::RunTurn {
            session_id,
            prompt,
            profile,
            tool_loop,
            submission_mode,
        } => {
            let session_id_label = session_id.as_str().to_string();
            spawn_turn_request_handler(
                transport,
                update_tx.clone(),
                sequenced.command_seq,
                session_id,
                prompt,
                profile,
                tool_loop,
                submission_mode,
            );
            snapshot.active_session_id = Some(session_id_label.clone());
            snapshot.last_status = Some(format!("Running Probe turn for {session_id_label}"));
            snapshot.last_error = None;
            let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
        }
        ProbeLaneCommand::QueueTurn {
            session_id,
            prompt,
            profile,
            tool_loop,
        } => {
            let session_id_label = session_id.as_str().to_string();
            match queue_turn_request(transport.clone(), &session_id, prompt, profile, tool_loop) {
                Ok((response, control, refreshed)) => {
                    watched_sessions.insert(
                        session_id_label.clone(),
                        probe_session_watch_signature(&refreshed, &control),
                    );
                    snapshot.active_session_id = Some(session_id_label.clone());
                    snapshot.last_status =
                        Some(format!("Queued Probe turn for {session_id_label}"));
                    snapshot.last_error = None;
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::TurnQueued { response, control },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::SessionLoaded {
                            snapshot: refreshed,
                            control: inspect_session_turns_only(
                                transport.clone(),
                                &SessionId::new(session_id_label.clone()),
                            )
                            .unwrap_or_else(|_| {
                                InspectSessionTurnsResponse {
                                    session_id: SessionId::new(session_id_label.clone()),
                                    active_turn: None,
                                    queued_turns: Vec::new(),
                                    recent_turns: Vec::new(),
                                }
                            }),
                        },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                    send_probe_command_response(
                        update_tx,
                        sequenced.command_seq,
                        ProbeLaneCommandKind::QueueTurn,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe turn queued")),
                        None,
                    );
                }
                Err(error) => send_probe_command_response(
                    update_tx,
                    sequenced.command_seq,
                    ProbeLaneCommandKind::QueueTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                ),
            }
        }
        ProbeLaneCommand::ResolvePendingApproval {
            session_id,
            call_id,
            resolution,
            profile,
            tool_loop,
        } => {
            let session_id_label = session_id.as_str().to_string();
            spawn_pending_approval_handler(
                transport,
                update_tx.clone(),
                sequenced.command_seq,
                session_id,
                call_id,
                resolution,
                profile,
                tool_loop,
            );
            snapshot.active_session_id = Some(session_id_label);
            snapshot.last_status = Some(String::from("Resolving Probe approval"));
            snapshot.last_error = None;
            let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
        }
        ProbeLaneCommand::InterruptTurn { session_id } => {
            let session_id_label = session_id.as_str().to_string();
            match interrupt_turn_request(transport.clone(), &session_id) {
                Ok((response, control, refreshed)) => {
                    watched_sessions.insert(
                        session_id_label.clone(),
                        probe_session_watch_signature(&refreshed, &control),
                    );
                    snapshot.active_session_id = Some(session_id_label.clone());
                    snapshot.last_status = Some(String::from("Interrupt processed"));
                    snapshot.last_error = None;
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::TurnInterrupted { response, control },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::SessionLoaded {
                            snapshot: refreshed,
                            control: inspect_session_turns_only(
                                transport.clone(),
                                &SessionId::new(session_id_label.clone()),
                            )
                            .unwrap_or_else(|_| {
                                InspectSessionTurnsResponse {
                                    session_id: SessionId::new(session_id_label.clone()),
                                    active_turn: None,
                                    queued_turns: Vec::new(),
                                    recent_turns: Vec::new(),
                                }
                            }),
                        },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                    send_probe_command_response(
                        update_tx,
                        sequenced.command_seq,
                        ProbeLaneCommandKind::InterruptTurn,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Interrupt request sent")),
                        None,
                    );
                }
                Err(error) => send_probe_command_response(
                    update_tx,
                    sequenced.command_seq,
                    ProbeLaneCommandKind::InterruptTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                ),
            }
        }
        ProbeLaneCommand::CancelQueuedTurn {
            session_id,
            turn_id,
        } => {
            let session_id_label = session_id.as_str().to_string();
            match cancel_queued_turn_request(transport.clone(), &session_id, turn_id.clone()) {
                Ok((response, control, refreshed)) => {
                    watched_sessions.insert(
                        session_id_label.clone(),
                        probe_session_watch_signature(&refreshed, &control),
                    );
                    snapshot.active_session_id = Some(session_id_label.clone());
                    snapshot.last_status = Some(String::from("Queued Probe turn cancelled"));
                    snapshot.last_error = None;
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::QueuedTurnCancelled { response, control },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::SessionLoaded {
                            snapshot: refreshed,
                            control: inspect_session_turns_only(
                                transport.clone(),
                                &SessionId::new(session_id_label.clone()),
                            )
                            .unwrap_or_else(|_| {
                                InspectSessionTurnsResponse {
                                    session_id: SessionId::new(session_id_label.clone()),
                                    active_turn: None,
                                    queued_turns: Vec::new(),
                                    recent_turns: Vec::new(),
                                }
                            }),
                        },
                    ));
                    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                    send_probe_command_response(
                        update_tx,
                        sequenced.command_seq,
                        ProbeLaneCommandKind::CancelQueuedTurn,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Queued Probe turn cancelled")),
                        None,
                    );
                }
                Err(error) => send_probe_command_response(
                    update_tx,
                    sequenced.command_seq,
                    ProbeLaneCommandKind::CancelQueuedTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                ),
            }
        }
    }

    let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
    let _ = config;
}

fn refresh_sessions_internal(
    transport: ProbeTransportHandle,
    workspace_cwd: &Option<PathBuf>,
    snapshot: &mut ProbeLaneSnapshot,
    update_tx: &Sender<ProbeLaneUpdate>,
    watched_sessions: &mut HashMap<String, ProbeSessionWatchSignature>,
) -> Result<String, String> {
    let sessions = list_sessions_request(transport.clone())?;
    let preferred_session_id = snapshot.active_session_id.as_deref();
    let workspace_selection = workspace_cwd
        .as_ref()
        .map(|cwd| select_workspace_session(&sessions, cwd, preferred_session_id))
        .unwrap_or(WorkspaceSessionSelection::None);
    let workspace_session_id = match &workspace_selection {
        WorkspaceSessionSelection::Match(session_id) => Some(session_id.as_str().to_string()),
        WorkspaceSessionSelection::None | WorkspaceSessionSelection::Collision(_) => None,
    };
    let workspace_collision_session_ids = match &workspace_selection {
        WorkspaceSessionSelection::Collision(session_ids) => session_ids
            .iter()
            .map(|session_id| session_id.as_str().to_string())
            .collect::<Vec<_>>(),
        WorkspaceSessionSelection::None | WorkspaceSessionSelection::Match(_) => Vec::new(),
    };
    let listed_sessions = sessions
        .iter()
        .map(|session| ProbeListedSession {
            session: session.clone(),
            control: inspect_session_turns_only(transport.clone(), &session.id).ok(),
        })
        .collect::<Vec<_>>();
    snapshot.session_count = sessions.len();
    snapshot.active_session_id = workspace_session_id.clone().or_else(|| {
        preferred_session_id.map(str::to_string).or_else(|| {
            sessions
                .first()
                .map(|session| session.id.as_str().to_string())
        })
    });
    snapshot.last_status = Some(if workspace_collision_session_ids.is_empty() {
        format!("Loaded {} Probe session(s)", sessions.len())
    } else {
        format!(
            "Loaded {} Probe session(s); multiple live workspace matches need selection",
            sessions.len()
        )
    });
    snapshot.last_error = None;
    let _ = update_tx.send(ProbeLaneUpdate::Notification(
        ProbeLaneNotification::SessionsListed {
            sessions: listed_sessions,
            workspace_session_id: workspace_session_id.clone(),
            workspace_collision_session_ids,
        },
    ));
    if let Some(session_id) = workspace_session_id {
        let session_id_value = SessionId::new(session_id.clone());
        let (session_snapshot, control) = inspect_session_bundle(transport, &session_id_value)?;
        watched_sessions.insert(
            session_id.clone(),
            probe_session_watch_signature(&session_snapshot, &control),
        );
        let _ = update_tx.send(ProbeLaneUpdate::Notification(
            ProbeLaneNotification::SessionLoaded {
                snapshot: session_snapshot,
                control,
            },
        ));
        return Ok(String::from(
            "Probe sessions refreshed and workspace session attached",
        ));
    }
    Ok(String::from("Probe sessions refreshed"))
}

fn spawn_turn_request_handler(
    transport: ProbeTransportHandle,
    update_tx: Sender<ProbeLaneUpdate>,
    command_seq: u64,
    session_id: SessionId,
    prompt: String,
    profile: BackendProfile,
    tool_loop: Option<ToolLoopConfig>,
    submission_mode: ProbeLaneTurnSubmissionMode,
) {
    let session_id_label = session_id.as_str().to_string();
    thread::spawn(move || {
        let tool_loop = match tool_loop
            .as_ref()
            .map(tool_loop_recipe_from_config)
            .transpose()
        {
            Ok(tool_loop) => tool_loop,
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::RunTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
                return;
            }
        };
        let request = TurnRequest {
            session_id: session_id.clone(),
            profile,
            prompt,
            author: Some(TurnAuthor {
                client_name: String::from("openagents-autopilot-desktop"),
                client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                display_name: Some(String::from("Autopilot")),
            }),
            tool_loop,
        };
        let runtime_request = match submission_mode {
            ProbeLaneTurnSubmissionMode::Start => RuntimeRequest::StartTurn(request),
            ProbeLaneTurnSubmissionMode::Continue => RuntimeRequest::ContinueTurn(request),
        };
        let stream = match transport.send_request(runtime_request) {
            Ok(stream) => stream,
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::RunTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
                return;
            }
        };
        match wait_for_runtime_response(stream, |event| {
            handle_runtime_event_notification(&update_tx, &session_id, event);
        }) {
            Ok(RuntimeResponse::StartTurn(response))
            | Ok(RuntimeResponse::ContinueTurn(response)) => match response {
                TurnResponse::Completed(completed) => {
                    let _ = refresh_loaded_session_notification(
                        transport.clone(),
                        &update_tx,
                        &completed.session.id,
                    );
                    send_probe_command_response(
                        &update_tx,
                        command_seq,
                        ProbeLaneCommandKind::RunTurn,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe turn completed")),
                        None,
                    );
                }
                TurnResponse::Paused(paused) => {
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::PendingApprovalsUpdated {
                            session_id: paused.session.id.as_str().to_string(),
                            approvals: paused.pending_approvals.clone(),
                        },
                    ));
                    let _ = refresh_loaded_session_notification(
                        transport.clone(),
                        &update_tx,
                        &paused.session.id,
                    );
                    send_probe_command_response(
                        &update_tx,
                        command_seq,
                        ProbeLaneCommandKind::RunTurn,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe turn paused for approval")),
                        None,
                    );
                }
            },
            Ok(other) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::RunTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(format!("unexpected Probe response: {other:?}")),
                );
            }
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::RunTurn,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
            }
        }
    });
}

fn spawn_pending_approval_handler(
    transport: ProbeTransportHandle,
    update_tx: Sender<ProbeLaneUpdate>,
    command_seq: u64,
    session_id: SessionId,
    call_id: String,
    resolution: ToolApprovalResolution,
    profile: BackendProfile,
    tool_loop: ToolLoopConfig,
) {
    let session_id_label = session_id.as_str().to_string();
    thread::spawn(move || {
        let tool_loop = match tool_loop_recipe_from_config(&tool_loop) {
            Ok(tool_loop) => tool_loop,
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::ResolvePendingApproval,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
                return;
            }
        };
        let stream = match transport.send_request(RuntimeRequest::ResolvePendingApproval(
            ResolvePendingApprovalRequest {
                session_id: session_id.clone(),
                profile,
                tool_loop,
                call_id,
                resolution,
            },
        )) {
            Ok(stream) => stream,
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::ResolvePendingApproval,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
                return;
            }
        };
        match wait_for_runtime_response(stream, |event| {
            handle_runtime_event_notification(&update_tx, &session_id, event);
        }) {
            Ok(RuntimeResponse::ResolvePendingApproval(response)) => match response {
                ResolvePendingApprovalResponse::StillPending {
                    session,
                    pending_approvals,
                } => {
                    let _ = update_tx.send(ProbeLaneUpdate::Notification(
                        ProbeLaneNotification::PendingApprovalsUpdated {
                            session_id: session.id.as_str().to_string(),
                            approvals: pending_approvals,
                        },
                    ));
                    let _ = refresh_loaded_session_notification(
                        transport.clone(),
                        &update_tx,
                        &session.id,
                    );
                    send_probe_command_response(
                        &update_tx,
                        command_seq,
                        ProbeLaneCommandKind::ResolvePendingApproval,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe approval still pending")),
                        None,
                    );
                }
                ResolvePendingApprovalResponse::Resumed(completed) => {
                    let _ = refresh_loaded_session_notification(
                        transport.clone(),
                        &update_tx,
                        &completed.session.id,
                    );
                    send_probe_command_response(
                        &update_tx,
                        command_seq,
                        ProbeLaneCommandKind::ResolvePendingApproval,
                        ProbeLaneCommandStatus::Ok,
                        Some(session_id_label),
                        Some(String::from("Probe approval resumed session")),
                        None,
                    );
                }
            },
            Ok(other) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::ResolvePendingApproval,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(format!("unexpected Probe response: {other:?}")),
                );
            }
            Err(error) => {
                send_probe_command_response(
                    &update_tx,
                    command_seq,
                    ProbeLaneCommandKind::ResolvePendingApproval,
                    ProbeLaneCommandStatus::Error,
                    Some(session_id_label),
                    None,
                    Some(error),
                );
            }
        }
    });
}

fn handle_runtime_event_notification(
    update_tx: &Sender<ProbeLaneUpdate>,
    session_id: &SessionId,
    event: ServerEvent,
) {
    match event {
        ServerEvent::RuntimeProgress { event, .. } => {
            let _ = update_tx.send(ProbeLaneUpdate::Notification(
                ProbeLaneNotification::RuntimeProgress {
                    session_id: session_id.as_str().to_string(),
                    event,
                },
            ));
        }
        ServerEvent::PendingApprovalsUpdated {
            session_id,
            approvals,
            ..
        } => {
            let _ = update_tx.send(ProbeLaneUpdate::Notification(
                ProbeLaneNotification::PendingApprovalsUpdated {
                    session_id: session_id.as_str().to_string(),
                    approvals,
                },
            ));
        }
        ServerEvent::DetachedSessionStream { record } => match record.payload {
            DetachedSessionEventPayload::ChildSessionUpdated { child } => {
                let _ = update_tx.send(ProbeLaneUpdate::Notification(
                    ProbeLaneNotification::ChildSessionUpdated {
                        session_id: record.session_id.as_str().to_string(),
                        child,
                    },
                ));
            }
            DetachedSessionEventPayload::WorkspaceStateUpdated {
                workspace_state,
                branch_state,
                delivery_state,
            } => {
                let _ = update_tx.send(ProbeLaneUpdate::Notification(
                    ProbeLaneNotification::WorkspaceStateUpdated {
                        session_id: record.session_id.as_str().to_string(),
                        workspace_state,
                        branch_state,
                        delivery_state,
                    },
                ));
            }
            DetachedSessionEventPayload::SummaryUpdated { .. }
            | DetachedSessionEventPayload::RuntimeProgress { .. }
            | DetachedSessionEventPayload::PendingApprovalsUpdated { .. }
            | DetachedSessionEventPayload::Note { .. } => {}
        },
    }
}

fn refresh_loaded_session_notification(
    transport: ProbeTransportHandle,
    update_tx: &Sender<ProbeLaneUpdate>,
    session_id: &SessionId,
) -> Result<(), String> {
    let (snapshot, control) = inspect_session_bundle(transport, session_id)?;
    update_tx
        .send(ProbeLaneUpdate::Notification(
            ProbeLaneNotification::SessionLoaded { snapshot, control },
        ))
        .map_err(|error| format!("failed to publish Probe session update: {error}"))?;
    Ok(())
}

fn queue_turn_request(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
    prompt: String,
    profile: BackendProfile,
    tool_loop: Option<ToolLoopConfig>,
) -> Result<
    (
        QueueTurnResponse,
        InspectSessionTurnsResponse,
        SessionSnapshot,
    ),
    String,
> {
    let tool_loop = tool_loop
        .as_ref()
        .map(tool_loop_recipe_from_config)
        .transpose()?;
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::QueueTurn(TurnRequest {
            session_id: session_id.clone(),
            profile,
            prompt,
            author: Some(TurnAuthor {
                client_name: String::from("openagents-autopilot-desktop"),
                client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                display_name: Some(String::from("Autopilot")),
            }),
            tool_loop,
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::QueueTurn(response) = response else {
        return Err(format!("unexpected Probe queue response: {response:?}"));
    };
    let control = inspect_session_turns_only(transport.clone(), session_id)?;
    let snapshot = inspect_session_only(transport, session_id)?;
    Ok((response, control, snapshot))
}

fn interrupt_turn_request(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
) -> Result<
    (
        InterruptTurnResponse,
        InspectSessionTurnsResponse,
        SessionSnapshot,
    ),
    String,
> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::InterruptTurn(InterruptTurnRequest {
            session_id: session_id.clone(),
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::InterruptTurn(response) = response else {
        return Err(format!("unexpected Probe interrupt response: {response:?}"));
    };
    let control = inspect_session_turns_only(transport.clone(), session_id)?;
    let snapshot = inspect_session_only(transport, session_id)?;
    Ok((response, control, snapshot))
}

fn cancel_queued_turn_request(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
    turn_id: String,
) -> Result<
    (
        CancelQueuedTurnResponse,
        InspectSessionTurnsResponse,
        SessionSnapshot,
    ),
    String,
> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::CancelQueuedTurn(CancelQueuedTurnRequest {
            session_id: session_id.clone(),
            turn_id,
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::CancelQueuedTurn(response) = response else {
        return Err(format!("unexpected Probe cancel response: {response:?}"));
    };
    let control = inspect_session_turns_only(transport.clone(), session_id)?;
    let snapshot = inspect_session_only(transport, session_id)?;
    Ok((response, control, snapshot))
}

fn list_sessions_request(transport: ProbeTransportHandle) -> Result<Vec<SessionMetadata>, String> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::ListSessions)?,
        |_| {},
    )?;
    let RuntimeResponse::ListSessions(response) = response else {
        return Err(format!(
            "unexpected Probe list sessions response: {response:?}"
        ));
    };
    let mut sessions = response.sessions;
    sessions.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| left.id.as_str().cmp(right.id.as_str()))
    });
    Ok(sessions)
}

fn inspect_session_only(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
) -> Result<SessionSnapshot, String> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::InspectSession(SessionLookupRequest {
            session_id: session_id.clone(),
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::InspectSession(snapshot) = response else {
        return Err(format!(
            "unexpected Probe inspect session response: {response:?}"
        ));
    };
    Ok(snapshot)
}

fn inspect_session_turns_only(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
) -> Result<InspectSessionTurnsResponse, String> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::InspectSessionTurns(SessionLookupRequest {
            session_id: session_id.clone(),
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::InspectSessionTurns(control) = response else {
        return Err(format!(
            "unexpected Probe inspect session turns response: {response:?}"
        ));
    };
    Ok(control)
}

fn inspect_session_bundle(
    transport: ProbeTransportHandle,
    session_id: &SessionId,
) -> Result<(SessionSnapshot, InspectSessionTurnsResponse), String> {
    let snapshot = inspect_session_only(transport.clone(), session_id)?;
    let control = inspect_session_turns_only(transport, session_id)?;
    Ok((snapshot, control))
}

fn start_session_request(
    transport: ProbeTransportHandle,
    title: &Option<String>,
    cwd: PathBuf,
    profile: BackendProfile,
    system_prompt: Option<String>,
    mounted_refs: Vec<SessionMountRef>,
) -> Result<(SessionSnapshot, InspectSessionTurnsResponse), String> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::StartSession(StartSessionRequest {
            title: title.clone(),
            cwd,
            profile,
            system_prompt,
            harness_profile: None,
            workspace_state: None,
            mounted_refs,
        }))?,
        |_| {},
    )?;
    let RuntimeResponse::StartSession(snapshot) = response else {
        return Err(format!(
            "unexpected Probe start session response: {response:?}"
        ));
    };
    let control = inspect_session_turns_only(transport, &snapshot.session.id)?;
    Ok((snapshot, control))
}

fn send_probe_command_response(
    update_tx: &Sender<ProbeLaneUpdate>,
    command_seq: u64,
    command: ProbeLaneCommandKind,
    status: ProbeLaneCommandStatus,
    session_id: Option<String>,
    message: Option<String>,
    error: Option<String>,
) {
    let _ = update_tx.send(ProbeLaneUpdate::CommandResponse(ProbeLaneCommandResponse {
        command_seq,
        command,
        status,
        session_id,
        message,
        error,
    }));
}

fn establish_probe_transport(
    config: &ProbeLaneConfig,
    snapshot: &mut ProbeLaneSnapshot,
    update_tx: &Sender<ProbeLaneUpdate>,
) -> Option<ProbeTransport> {
    match ProbeTransport::spawn(config) {
        Ok(transport) => {
            snapshot.lifecycle = ProbeLaneLifecycle::Ready;
            snapshot.last_status = Some(transport.kind.connected_status().to_string());
            snapshot.last_error = None;
            let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
            Some(transport)
        }
        Err(error) => {
            snapshot.lifecycle = ProbeLaneLifecycle::Error;
            snapshot.last_status = Some(String::from("Probe lane failed to start"));
            snapshot.last_error = Some(error);
            let _ = update_tx.send(ProbeLaneUpdate::Snapshot(Box::new(snapshot.clone())));
            None
        }
    }
}

impl ProbeTransport {
    fn spawn(config: &ProbeLaneConfig) -> Result<Self, String> {
        if config.prefer_local_daemon {
            match connect_or_autostart_local_daemon_transport(config) {
                Ok(transport) => {
                    initialize_probe_transport(transport.handle(), config)?;
                    return Ok(transport);
                }
                Err(daemon_error) => {
                    let transport = spawn_stdio_transport(config).map_err(|stdio_error| {
                        format!(
                            "failed to connect or autostart local probe-daemon: {daemon_error}; failed to spawn direct probe-server fallback: {stdio_error}"
                        )
                    })?;
                    initialize_probe_transport(transport.handle(), config)?;
                    return Ok(transport);
                }
            }
        }

        let transport = spawn_stdio_transport(config)?;
        initialize_probe_transport(transport.handle(), config)?;
        Ok(transport)
    }

    fn handle(&self) -> ProbeTransportHandle {
        self.handle.clone()
    }

    fn shutdown(&mut self) {
        if self.shutdown_on_drop
            && let Ok(stream) = self.handle.send_request(RuntimeRequest::Shutdown)
        {
            let _ = wait_for_runtime_response(stream, |_| {});
        }
        self.handle
            .close_pending(String::from("Probe lane shutting down"));
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(join_handle) = self.reader_join.take() {
            let _ = join_handle.join();
        }
    }
}

impl Drop for ProbeTransport {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ProbeTransportHandle {
    fn send_request(&self, request: RuntimeRequest) -> Result<ProbeRequestStream, String> {
        let request_id = format!(
            "req-{}",
            self.inner.next_request_id.fetch_add(1, Ordering::Relaxed)
        );
        let (tx, rx) = mpsc::channel::<ProbeTransportMessage>();
        self.inner
            .pending
            .lock()
            .map_err(|_| String::from("probe transport mutex poisoned"))?
            .insert(request_id.clone(), tx);

        let message = ClientMessage::Request(RequestEnvelope {
            request_id: request_id.clone(),
            request,
        });
        let write_result = {
            let mut stdin = self
                .inner
                .stdin
                .lock()
                .map_err(|_| String::from("probe transport stdin mutex poisoned"))?;
            serde_json::to_writer(&mut *stdin, &message)
                .map_err(|error| format!("failed to encode probe request: {error}"))?;
            stdin
                .write_all(b"\n")
                .map_err(|error| format!("failed to write probe request: {error}"))?;
            stdin
                .flush()
                .map_err(|error| format!("failed to flush probe request: {error}"))
        };

        if let Err(error) = write_result {
            self.inner
                .pending
                .lock()
                .ok()
                .and_then(|mut pending| pending.remove(&request_id));
            return Err(error);
        }

        Ok(ProbeRequestStream { request_id, rx })
    }

    fn close_pending(&self, reason: String) {
        let pending = self
            .inner
            .pending
            .lock()
            .map(|mut pending| std::mem::take(&mut *pending))
            .unwrap_or_default();
        for tx in pending.into_values() {
            let _ = tx.send(ProbeTransportMessage::Closed(reason.clone()));
        }
    }
}

fn initialize_probe_transport(
    transport: ProbeTransportHandle,
    config: &ProbeLaneConfig,
) -> Result<(), String> {
    initialize_probe_transport_with_identity(
        transport,
        config.client_name.as_str(),
        config.client_version.clone(),
    )
}

fn initialize_probe_transport_with_identity(
    transport: ProbeTransportHandle,
    client_name: &str,
    client_version: Option<String>,
) -> Result<(), String> {
    let response = wait_for_runtime_response(
        transport.send_request(RuntimeRequest::Initialize(InitializeRequest {
            client_name: client_name.to_string(),
            client_version,
            protocol_version: probe_protocol::PROBE_PROTOCOL_VERSION,
        }))?,
        |_| {},
    )?;
    match response {
        RuntimeResponse::Initialize(_) => Ok(()),
        other => Err(format!("unexpected Probe initialize response: {other:?}")),
    }
}

fn run_transport_reader(mut reader: Box<dyn BufRead + Send>, inner: Arc<ProbeTransportInner>) {
    loop {
        let mut line = String::new();
        let bytes = match reader.read_line(&mut line) {
            Ok(bytes) => bytes,
            Err(error) => {
                ProbeTransportHandle {
                    inner: Arc::clone(&inner),
                }
                .close_pending(format!("probe-server io error: {error}"));
                return;
            }
        };
        if bytes == 0 {
            ProbeTransportHandle {
                inner: Arc::clone(&inner),
            }
            .close_pending(String::from("probe-server exited"));
            return;
        }
        let message = match serde_json::from_str::<ServerMessage>(line.trim_end()) {
            Ok(message) => message,
            Err(error) => {
                ProbeTransportHandle {
                    inner: Arc::clone(&inner),
                }
                .close_pending(format!("probe-server protocol decode failed: {error}"));
                return;
            }
        };
        let (request_id, payload, remove_after_send) = match message {
            ServerMessage::Event(EventEnvelope { request_id, event }) => {
                (request_id, ProbeTransportMessage::Event(event), false)
            }
            ServerMessage::Response(ResponseEnvelope { request_id, body }) => {
                (request_id, ProbeTransportMessage::Response(body), true)
            }
        };
        let tx = {
            let mut pending = match inner.pending.lock() {
                Ok(pending) => pending,
                Err(_) => return,
            };
            let sender = pending.get(&request_id).cloned();
            if remove_after_send {
                pending.remove(&request_id);
            }
            sender
        };
        if let Some(tx) = tx {
            let _ = tx.send(payload);
        }
    }
}

fn wait_for_runtime_response(
    stream: ProbeRequestStream,
    mut on_event: impl FnMut(ServerEvent),
) -> Result<RuntimeResponse, String> {
    loop {
        match stream
            .rx
            .recv()
            .map_err(|error| format!("Probe request {} failed: {error}", stream.request_id))?
        {
            ProbeTransportMessage::Event(event) => on_event(event),
            ProbeTransportMessage::Response(ResponseBody::Ok { response }) => return Ok(response),
            ProbeTransportMessage::Response(ResponseBody::Error { error }) => {
                return Err(format!("{} ({})", error.message, error.code));
            }
            ProbeTransportMessage::Closed(reason) => {
                return Err(format!(
                    "Probe request {} terminated before response: {}",
                    stream.request_id, reason
                ));
            }
        }
    }
}

fn spawn_stdio_transport(config: &ProbeLaneConfig) -> Result<ProbeTransport, String> {
    let mut command = build_server_command(config)?;
    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to spawn probe-server: {error}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| String::from("probe-server child did not expose stdin"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| String::from("probe-server child did not expose stdout"))?;
    build_transport_from_streams(
        ProbeTransportKind::SpawnedStdio,
        Box::new(stdin),
        Box::new(BufReader::new(stdout)),
        Some(child),
        true,
    )
}

fn build_server_command(config: &ProbeLaneConfig) -> Result<Command, String> {
    let mut command = resolve_server_command_plan(config)?.into_command();
    command
        .arg("--probe-home")
        .arg(config.probe_home.as_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    Ok(command)
}

fn resolve_server_command_plan(config: &ProbeLaneConfig) -> Result<ProbeCommandPlan, String> {
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to resolve current exe: {error}"))?;
    Ok(resolve_server_command_plan_from(
        config.server_binary.clone(),
        env::var_os("PROBE_SERVER_BIN").map(PathBuf::from),
        current_exe,
    ))
}

fn resolve_server_command_plan_from(
    explicit_server_binary: Option<PathBuf>,
    env_server_binary: Option<PathBuf>,
    current_exe: PathBuf,
) -> ProbeCommandPlan {
    if let Some(path) = explicit_server_binary.or(env_server_binary) {
        return ProbeCommandPlan {
            program: path,
            args: Vec::new(),
        };
    }

    let sibling_server = sibling_probe_server_path(current_exe.as_path());
    if sibling_server.exists() {
        return ProbeCommandPlan {
            program: sibling_server,
            args: Vec::new(),
        };
    }

    ProbeCommandPlan {
        program: current_exe,
        args: vec![OsString::from(probe_client::INTERNAL_SERVER_SUBCOMMAND)],
    }
}

fn build_transport_from_streams(
    kind: ProbeTransportKind,
    stdin: Box<dyn Write + Send>,
    reader: Box<dyn BufRead + Send>,
    child: Option<Child>,
    shutdown_on_drop: bool,
) -> Result<ProbeTransport, String> {
    let inner = Arc::new(ProbeTransportInner {
        stdin: Mutex::new(stdin),
        pending: Mutex::new(HashMap::new()),
        next_request_id: AtomicU64::new(1),
    });
    let reader_inner = Arc::clone(&inner);
    let reader_join = thread::spawn(move || {
        run_transport_reader(reader, reader_inner);
    });
    Ok(ProbeTransport {
        kind,
        handle: ProbeTransportHandle { inner },
        child,
        reader_join: Some(reader_join),
        shutdown_on_drop,
    })
}

fn connect_or_autostart_local_daemon_transport(
    config: &ProbeLaneConfig,
) -> Result<ProbeTransport, String> {
    match connect_local_daemon_transport(config) {
        Ok(transport) => Ok(transport),
        Err(error) if is_missing_local_daemon_error(&error) => {
            spawn_local_daemon(config.probe_home.as_path())?;
            wait_for_local_daemon(config, PROBE_DAEMON_WAIT_TIMEOUT)?;
            connect_local_daemon_transport(config).map_err(|error| {
                format!("failed to connect to probe-daemon after autostart: {error}")
            })
        }
        Err(error) => Err(format!("failed to connect to probe-daemon: {error}")),
    }
}

#[cfg(unix)]
fn connect_local_daemon_transport(
    config: &ProbeLaneConfig,
) -> Result<ProbeTransport, std::io::Error> {
    let socket_path = default_local_daemon_socket_path(config.probe_home.as_path());
    let stream = UnixStream::connect(&socket_path)?;
    let writer = stream.try_clone()?;
    let reader = Box::new(BufReader::new(stream)) as Box<dyn BufRead + Send>;
    build_transport_from_streams(
        ProbeTransportKind::LocalDaemon,
        Box::new(writer),
        reader,
        None,
        false,
    )
    .map_err(std::io::Error::other)
}

#[cfg(not(unix))]
fn connect_local_daemon_transport(
    _config: &ProbeLaneConfig,
) -> Result<ProbeTransport, std::io::Error> {
    Err(std::io::Error::other(
        "local probe-daemon transport is only available on unix platforms",
    ))
}

fn is_missing_local_daemon_error(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
    )
}

fn spawn_local_daemon(probe_home: &Path) -> Result<(), String> {
    let mut command = build_daemon_command()?;
    command
        .arg("--probe-home")
        .arg(probe_home)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to spawn probe-daemon: {error}"))
}

fn build_daemon_command() -> Result<Command, String> {
    resolve_daemon_command_plan().map(ProbeCommandPlan::into_command)
}

fn resolve_daemon_command_plan() -> Result<ProbeCommandPlan, String> {
    let current_exe =
        env::current_exe().map_err(|error| format!("failed to resolve current exe: {error}"))?;
    Ok(resolve_daemon_command_plan_from(
        env::var_os("PROBE_DAEMON_BIN").map(PathBuf::from),
        current_exe,
    ))
}

fn resolve_daemon_command_plan_from(
    env_daemon_binary: Option<PathBuf>,
    current_exe: PathBuf,
) -> ProbeCommandPlan {
    if let Some(path) = env_daemon_binary {
        return ProbeCommandPlan {
            program: path,
            args: vec![OsString::from("run")],
        };
    }

    let sibling_daemon = sibling_named_binary_path(current_exe.as_path(), "probe-daemon");
    if sibling_daemon.exists() {
        return ProbeCommandPlan {
            program: sibling_daemon,
            args: vec![OsString::from("run")],
        };
    }

    ProbeCommandPlan {
        program: current_exe,
        args: vec![OsString::from(probe_client::INTERNAL_DAEMON_SUBCOMMAND)],
    }
}

fn wait_for_local_daemon(config: &ProbeLaneConfig, wait_timeout: Duration) -> Result<(), String> {
    let deadline = std::time::Instant::now() + wait_timeout;
    loop {
        match connect_local_daemon_transport(config) {
            Ok(mut transport) => {
                transport.shutdown();
                return Ok(());
            }
            Err(error)
                if is_missing_local_daemon_error(&error)
                    && std::time::Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                return Err(format!("failed waiting for probe-daemon socket: {error}"));
            }
        }
    }
}

fn sibling_probe_server_path(current_exe: &Path) -> PathBuf {
    sibling_named_binary_path(current_exe, "probe-server")
}

fn sibling_named_binary_path(current_exe: &Path, binary_name: &str) -> PathBuf {
    let base_dir = current_exe
        .parent()
        .and_then(|parent| {
            if parent.file_name().is_some_and(|name| name == "deps") {
                parent.parent()
            } else {
                Some(parent)
            }
        })
        .unwrap_or_else(|| Path::new("."));
    base_dir.join(format!("{binary_name}{}", env::consts::EXE_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_daemon_command_plan_from, resolve_server_command_plan_from,
        sibling_named_binary_path,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    #[test]
    fn server_command_plan_prefers_explicit_binary() {
        let current_exe = PathBuf::from("/tmp/autopilot-desktop");
        let explicit_server = PathBuf::from("/tmp/custom-probe-server");
        let plan =
            resolve_server_command_plan_from(Some(explicit_server.clone()), None, current_exe);
        assert_eq!(plan.program, explicit_server);
        assert!(plan.args.is_empty());
    }

    #[test]
    fn server_command_plan_prefers_env_override_before_sibling_or_internal_fallback() {
        let temp = tempdir().expect("temp dir");
        let current_exe = temp.path().join("autopilot-desktop");
        fs::write(&current_exe, "").expect("write current exe");
        let sibling_server = sibling_named_binary_path(&current_exe, "probe-server");
        fs::write(&sibling_server, "").expect("write sibling server");

        let env_server = temp.path().join("env-probe-server");
        let plan = resolve_server_command_plan_from(None, Some(env_server.clone()), current_exe);
        assert_eq!(plan.program, env_server);
        assert!(plan.args.is_empty());
    }

    #[test]
    fn server_command_plan_uses_sibling_binary_when_present() {
        let temp = tempdir().expect("temp dir");
        let current_exe = temp.path().join("autopilot-desktop");
        fs::write(&current_exe, "").expect("write current exe");
        let sibling_server = sibling_named_binary_path(&current_exe, "probe-server");
        fs::write(&sibling_server, "").expect("write sibling server");

        let plan = resolve_server_command_plan_from(None, None, current_exe);
        assert_eq!(plan.program, sibling_server);
        assert!(plan.args.is_empty());
    }

    #[test]
    fn server_command_plan_falls_back_to_internal_subcommand_when_no_binary_exists() {
        let temp = tempdir().expect("temp dir");
        let current_exe = temp.path().join("autopilot-desktop");
        fs::write(&current_exe, "").expect("write current exe");

        let plan = resolve_server_command_plan_from(None, None, current_exe.clone());
        assert_eq!(plan.program, current_exe);
        assert_eq!(
            plan.args,
            vec![OsString::from(probe_client::INTERNAL_SERVER_SUBCOMMAND)]
        );
    }

    #[test]
    fn daemon_command_plan_prefers_env_override() {
        let current_exe = PathBuf::from("/tmp/autopilot-desktop");
        let daemon_bin = PathBuf::from("/tmp/custom-probe-daemon");
        let plan = resolve_daemon_command_plan_from(Some(daemon_bin.clone()), current_exe);
        assert_eq!(plan.program, daemon_bin);
        assert_eq!(plan.args, vec![OsString::from("run")]);
    }

    #[test]
    fn daemon_command_plan_uses_sibling_binary_when_present() {
        let temp = tempdir().expect("temp dir");
        let current_exe = temp.path().join("autopilot-desktop");
        fs::write(&current_exe, "").expect("write current exe");
        let sibling_daemon = sibling_named_binary_path(&current_exe, "probe-daemon");
        fs::write(&sibling_daemon, "").expect("write sibling daemon");

        let plan = resolve_daemon_command_plan_from(None, current_exe);
        assert_eq!(plan.program, sibling_daemon);
        assert_eq!(plan.args, vec![OsString::from("run")]);
    }

    #[test]
    fn daemon_command_plan_falls_back_to_internal_subcommand_when_no_binary_exists() {
        let temp = tempdir().expect("temp dir");
        let current_exe = temp.path().join("autopilot-desktop");
        fs::write(&current_exe, "").expect("write current exe");

        let plan = resolve_daemon_command_plan_from(None, current_exe.clone());
        assert_eq!(plan.program, current_exe);
        assert_eq!(
            plan.args,
            vec![OsString::from(probe_client::INTERNAL_DAEMON_SUBCOMMAND)]
        );
    }
}

fn tool_loop_recipe_from_config(config: &ToolLoopConfig) -> Result<ToolLoopRecipe, String> {
    let tool_set = match config.registry.name() {
        "coding_bootstrap" => ToolSetKind::CodingBootstrap,
        other => return Err(format!("unsupported Probe tool registry: {other}")),
    };
    Ok(ToolLoopRecipe {
        tool_set,
        tool_choice: match &config.tool_choice {
            ProbeToolChoice::None => ToolChoice::None,
            ProbeToolChoice::Auto => ToolChoice::Auto,
            ProbeToolChoice::Required => ToolChoice::Required,
            ProbeToolChoice::Named(tool_name) => ToolChoice::Named {
                tool_name: tool_name.clone(),
            },
        },
        parallel_tool_calls: config.parallel_tool_calls,
        max_model_round_trips: config.max_model_round_trips,
        approval: ToolApprovalRecipe {
            allow_write_tools: config.approval.allow_write_tools,
            allow_network_shell: config.approval.allow_network_shell,
            allow_destructive_shell: config.approval.allow_destructive_shell,
            denied_action: match config.approval.denied_action {
                ToolDeniedAction::Refuse => ProtocolDeniedAction::Refuse,
                ToolDeniedAction::Pause => ProtocolDeniedAction::Pause,
            },
        },
        oracle: config.oracle.as_ref().map(|oracle| ToolOracleRecipe {
            profile: oracle.profile.clone(),
            max_calls: oracle.max_calls,
        }),
        long_context: config
            .long_context
            .as_ref()
            .map(|context| ToolLongContextRecipe {
                profile: context.profile.clone(),
                max_calls: context.max_calls,
                max_evidence_files: context.max_evidence_files,
                max_lines_per_file: context.max_lines_per_file,
            }),
    })
}

fn default_probe_home_path() -> PathBuf {
    if let Ok(value) = env::var("OPENAGENTS_PROBE_HOME")
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join("openagents")
        .join("probe")
}

fn select_workspace_session<'a>(
    sessions: &'a [SessionMetadata],
    workspace_cwd: &Path,
    preferred_session_id: Option<&str>,
) -> WorkspaceSessionSelection<'a> {
    let target = workspace_cwd
        .canonicalize()
        .unwrap_or_else(|_| workspace_cwd.to_path_buf());
    if let Some(preferred_session_id) = preferred_session_id
        && let Some(session) = sessions.iter().find(|session| {
            session.id.as_str() == preferred_session_id
                && session.state == probe_protocol::session::SessionState::Active
                && session
                    .cwd
                    .canonicalize()
                    .unwrap_or_else(|_| session.cwd.clone())
                    == target
        })
    {
        return WorkspaceSessionSelection::Match(&session.id);
    }
    let matches = sessions
        .iter()
        .filter(|session| {
            session.state == probe_protocol::session::SessionState::Active
                && session
                    .cwd
                    .canonicalize()
                    .unwrap_or_else(|_| session.cwd.clone())
                    == target
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => WorkspaceSessionSelection::None,
        [session] => WorkspaceSessionSelection::Match(&session.id),
        _ => WorkspaceSessionSelection::Collision(
            matches.into_iter().map(|session| &session.id).collect(),
        ),
    }
}

fn probe_session_watch_signature(
    snapshot: &SessionSnapshot,
    control: &InspectSessionTurnsResponse,
) -> ProbeSessionWatchSignature {
    ProbeSessionWatchSignature {
        updated_at_ms: snapshot.session.updated_at_ms,
        active_turn: control
            .active_turn
            .as_ref()
            .map(|turn| (turn.turn_id.clone(), turn.status, turn.awaiting_approval)),
        queued_turns: control
            .queued_turns
            .iter()
            .map(|turn| (turn.turn_id.clone(), turn.status))
            .collect(),
        recent_turns: control
            .recent_turns
            .iter()
            .take(6)
            .map(|turn| (turn.turn_id.clone(), turn.status))
            .collect(),
        pending_approval_ids: snapshot
            .pending_approvals
            .iter()
            .map(|approval| approval.tool_call_id.clone())
            .collect(),
    }
}
