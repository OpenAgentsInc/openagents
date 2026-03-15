use crate::local_inference_runtime::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance,
};
use futures_util::StreamExt;
use openagents_kernel_core::ids::sha256_prefixed_text;
use psionic_apple_fm::{
    AppleFmAdapterAttachRequest, AppleFmAdapterInventoryEntry, AppleFmAdapterLoadRequest,
    AppleFmAdapterSelection, AppleFmAsyncBridgeClient, AppleFmBridgeClient,
    AppleFmChatCompletionRequest, AppleFmChatMessage, AppleFmChatMessageRole,
    AppleFmGeneratedContent, AppleFmGenerationOptions, AppleFmGenerationSchema,
    AppleFmSessionCreateRequest, AppleFmSessionRespondRequest,
    AppleFmSessionStructuredGenerationRequest, AppleFmStructuredGenerationRequest,
    AppleFmSystemLanguageModel, AppleFmSystemLanguageModelGuardrails,
    AppleFmSystemLanguageModelUnavailableReason, AppleFmSystemLanguageModelUseCase,
    AppleFmTextGenerationRequest, AppleFmTextStreamEventKind, AppleFmTool, AppleFmToolCallError,
    AppleFmToolDefinition, DEFAULT_APPLE_FM_MODEL_ID,
};
use reqwest::Url;
use reqwest::blocking::Client as HttpClient;
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::runtime::Builder as TokioRuntimeBuilder;

const LANE_POLL: Duration = Duration::from_millis(120);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const BRIDGE_HEALTH_POLL: Duration = Duration::from_millis(100);
const BRIDGE_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_APPLE_FM_BASE_URL: &str = "http://127.0.0.1:11435";
const ENV_APPLE_FM_BASE_URL: &str = "OPENAGENTS_APPLE_FM_BASE_URL";
const ENV_APPLE_FM_BRIDGE_BIN: &str = "OPENAGENTS_APPLE_FM_BRIDGE_BIN";

/// Shown when the bridge binary is missing or build failed. Concrete steps and why.
const APPLE_FM_FIX_BUILD: &str = "Build the bridge: open Terminal, go to the repo folder, run: cd swift/foundation-bridge && ./build.sh. The bridge is written in Swift, so that command needs the Swift compiler. If the build fails, install it: Xcode from the App Store (free), or run xcode-select --install to install only the Command Line Tools (Swift compiler without the full Xcode app). Then restart the app.";

/// Shown when the system model is unavailable. Exact menu path.
const APPLE_FM_FIX_APPLE_INTELLIGENCE: &str = "Enable Apple Intelligence: open System Settings → Apple Intelligence (in the sidebar) → turn on Apple Intelligence. Requires macOS 26+ and Apple Silicon.";

/// Shown when the binary is missing but we appear to be running from a shipped .app (so user should not build).
const APPLE_FM_FIX_SHIPPED_MISSING: &str = "Apple FM bridge binary is missing from this app. The app was not packaged with the bridge. Reinstall from a build that includes it, or ask the vendor for a complete build.";

/// True when the current executable path looks like we're inside a macOS .app bundle (shipped app).
fn running_from_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(String::from))
        .map_or(false, |path| path.contains(".app/Contents/"))
}

#[derive(Clone, Debug)]
pub struct AppleFmBridgeConfig {
    pub base_url: String,
    pub auto_start: bool,
}

impl Default for AppleFmBridgeConfig {
    fn default() -> Self {
        Self {
            base_url: std::env::var(ENV_APPLE_FM_BASE_URL)
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_APPLE_FM_BASE_URL.to_string()),
            auto_start: cfg!(target_os = "macos"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppleFmBridgeStatus {
    UnsupportedPlatform,
    NotStarted,
    Starting,
    Running,
    Failed,
}

impl AppleFmBridgeStatus {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::UnsupportedPlatform => "unsupported_platform",
            Self::NotStarted => "not_started",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Failed => "failed",
        }
    }
}

impl Default for AppleFmBridgeStatus {
    fn default() -> Self {
        Self::NotStarted
    }
}

#[derive(Clone, Debug, Default)]
pub struct AppleFmBridgeSnapshot {
    pub base_url: String,
    pub bridge_status: Option<String>,
    pub reachable: bool,
    pub model_available: bool,
    pub system_model: AppleFmSystemLanguageModel,
    pub unavailable_reason: Option<AppleFmSystemLanguageModelUnavailableReason>,
    pub supported_use_cases: Vec<AppleFmSystemLanguageModelUseCase>,
    pub supported_guardrails: Vec<AppleFmSystemLanguageModelGuardrails>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub adapter_inventory_supported: bool,
    pub adapter_attach_supported: bool,
    pub loaded_adapters: Vec<AppleFmAdapterInventoryEntry>,
    pub availability_message: Option<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub last_request_id: Option<String>,
    pub last_metrics: Option<LocalInferenceExecutionMetrics>,
    pub refreshed_at: Option<Instant>,
}

impl AppleFmBridgeSnapshot {
    pub fn is_ready(&self) -> bool {
        self.reachable && self.model_available && self.ready_model.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmGenerateJob {
    pub request_id: String,
    pub prompt: String,
    pub requested_model: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchOperation {
    CreateSession,
    InspectSession,
    LoadAdapter,
    UnloadAdapter,
    AttachSessionAdapter,
    DetachSessionAdapter,
    RunText,
    RunChat,
    RunSession,
    RunStream,
    RunStructured,
    ExportTranscript,
    RestoreTranscript,
    ResetSession,
    DeleteSession,
}

impl AppleFmWorkbenchOperation {
    pub const fn label(&self) -> &'static str {
        match self {
            Self::CreateSession => "create_session",
            Self::InspectSession => "inspect_session",
            Self::LoadAdapter => "load_adapter",
            Self::UnloadAdapter => "unload_adapter",
            Self::AttachSessionAdapter => "attach_session_adapter",
            Self::DetachSessionAdapter => "detach_session_adapter",
            Self::RunText => "run_text",
            Self::RunChat => "run_chat",
            Self::RunSession => "run_session",
            Self::RunStream => "run_stream",
            Self::RunStructured => "run_structured",
            Self::ExportTranscript => "export_transcript",
            Self::RestoreTranscript => "restore_transcript",
            Self::ResetSession => "reset_session",
            Self::DeleteSession => "delete_session",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchToolMode {
    None,
    Demo,
    Failing,
}

#[derive(Clone, Debug)]
pub struct AppleFmWorkbenchCommand {
    pub request_id: String,
    pub operation: AppleFmWorkbenchOperation,
    pub instructions: Option<String>,
    pub prompt: Option<String>,
    pub requested_model: Option<String>,
    pub session_id: Option<String>,
    pub adapter_id: Option<String>,
    pub adapter_package_path: Option<String>,
    pub options: Option<AppleFmGenerationOptions>,
    pub schema_json: Option<String>,
    pub transcript_json: Option<String>,
    pub tool_mode: AppleFmWorkbenchToolMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchLogLevel {
    Info,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmWorkbenchStarted {
    pub request_id: String,
    pub operation: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmWorkbenchEvent {
    pub request_id: String,
    pub level: AppleFmWorkbenchLogLevel,
    pub line: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmWorkbenchCompleted {
    pub request_id: String,
    pub operation: String,
    pub summary: String,
    pub model: Option<String>,
    pub session_id: Option<String>,
    pub session_adapter: Option<AppleFmAdapterSelection>,
    pub response_text: String,
    pub adapter_json: Option<String>,
    pub session_json: Option<String>,
    pub structured_json: Option<String>,
    pub transcript_json: Option<String>,
    pub usage_json: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmWorkbenchFailed {
    pub request_id: String,
    pub operation: String,
    pub error: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppleFmWorkbenchUpdate {
    Started(AppleFmWorkbenchStarted),
    Event(AppleFmWorkbenchEvent),
    Completed(AppleFmWorkbenchCompleted),
    Failed(AppleFmWorkbenchFailed),
}

#[derive(Clone, Debug)]
pub struct AppleFmMissionControlSummaryCommand {
    pub request_id: String,
    pub instructions: String,
    pub prompt: String,
    pub requested_model: Option<String>,
    pub options: Option<AppleFmGenerationOptions>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmMissionControlSummaryStarted {
    pub request_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmMissionControlSummaryDelta {
    pub request_id: String,
    pub delta: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmMissionControlSummaryCompleted {
    pub request_id: String,
    pub summary: String,
    pub model: Option<String>,
    pub response_text: String,
    pub usage_json: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmMissionControlSummaryFailed {
    pub request_id: String,
    pub error: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppleFmMissionControlSummaryUpdate {
    Started(AppleFmMissionControlSummaryStarted),
    Delta(AppleFmMissionControlSummaryDelta),
    Completed(AppleFmMissionControlSummaryCompleted),
    Failed(AppleFmMissionControlSummaryFailed),
}

#[derive(Clone, Debug)]
pub enum AppleFmBridgeCommand {
    Refresh,
    EnsureBridgeRunning,
    StopBridge,
    Generate(AppleFmGenerateJob),
    Workbench(AppleFmWorkbenchCommand),
    MissionControlSummary(AppleFmMissionControlSummaryCommand),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmExecutionStarted {
    pub request_id: String,
    pub model: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmExecutionCompleted {
    pub request_id: String,
    pub model: String,
    pub output: String,
    pub metrics: LocalInferenceExecutionMetrics,
    pub provenance: LocalInferenceExecutionProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppleFmExecutionFailed {
    pub request_id: String,
    pub error: String,
}

#[derive(Clone, Debug)]
pub enum AppleFmBridgeUpdate {
    Snapshot(Box<AppleFmBridgeSnapshot>),
    Started(AppleFmExecutionStarted),
    Completed(AppleFmExecutionCompleted),
    Failed(AppleFmExecutionFailed),
    Workbench(Box<AppleFmWorkbenchUpdate>),
    MissionControlSummary(Box<AppleFmMissionControlSummaryUpdate>),
}

pub struct AppleFmBridgeWorker {
    command_tx: Sender<AppleFmBridgeCommand>,
    update_rx: Receiver<AppleFmBridgeUpdate>,
    shutdown_tx: Option<Sender<()>>,
    join_handle: Option<JoinHandle<()>>,
}

impl AppleFmBridgeWorker {
    pub fn spawn() -> Self {
        Self::spawn_with_config(AppleFmBridgeConfig::default())
    }

    pub fn spawn_with_config(config: AppleFmBridgeConfig) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<AppleFmBridgeCommand>();
        let (update_tx, update_rx) = mpsc::channel::<AppleFmBridgeUpdate>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let join_handle = std::thread::spawn(move || {
            run_apple_fm_loop(command_rx, update_tx, shutdown_rx, config)
        });

        Self {
            command_tx,
            update_rx,
            shutdown_tx: Some(shutdown_tx),
            join_handle: Some(join_handle),
        }
    }

    pub fn enqueue(&self, command: AppleFmBridgeCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|error| format!("Apple FM bridge worker offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<AppleFmBridgeUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }
}

impl Drop for AppleFmBridgeWorker {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        if let Some(join_handle) = self.join_handle.take() {
            let _ = join_handle.join();
        }
    }
}

#[derive(Default)]
struct AppleFmLocalBridge {
    child: Option<Child>,
    status: AppleFmBridgeStatus,
}

/// Sends current snapshot to the UI log when a sender is provided.
fn emit_snapshot_if(snapshot: &AppleFmBridgeSnapshot, emit: Option<&Sender<AppleFmBridgeUpdate>>) {
    if let Some(tx) = emit {
        let _ = tx.send(AppleFmBridgeUpdate::Snapshot(Box::new(snapshot.clone())));
    }
}

fn reset_snapshot_health(snapshot: &mut AppleFmBridgeSnapshot) {
    snapshot.available_models.clear();
    snapshot.loaded_adapters.clear();
    snapshot.ready_model = None;
    snapshot.system_model = AppleFmSystemLanguageModel::default();
    snapshot.unavailable_reason = None;
    snapshot.supported_use_cases.clear();
    snapshot.supported_guardrails.clear();
    snapshot.last_metrics = None;
    snapshot.reachable = false;
    snapshot.model_available = false;
    snapshot.adapter_inventory_supported = false;
    snapshot.adapter_attach_supported = false;
    snapshot.availability_message = None;
}

fn hydrate_snapshot_from_health(
    snapshot: &mut AppleFmBridgeSnapshot,
    health: &psionic_apple_fm::AppleFmHealthResponse,
) {
    let system_model_status = health.system_model_availability();
    snapshot.reachable = true;
    snapshot.model_available = system_model_status.available;
    snapshot.system_model = system_model_status.model.clone();
    snapshot.unavailable_reason = system_model_status.unavailable_reason;
    snapshot.supported_use_cases = system_model_status.supported_use_cases.clone();
    snapshot.supported_guardrails = system_model_status.supported_guardrails.clone();
    snapshot.adapter_inventory_supported = health.adapter_inventory_supported;
    snapshot.adapter_attach_supported = health.adapter_attach_supported;
    snapshot.loaded_adapters = health.loaded_adapters.clone();
    snapshot.availability_message = system_model_status.availability_message.clone();
    if system_model_status.available {
        let model = if snapshot.system_model.id.trim().is_empty() {
            DEFAULT_APPLE_FM_MODEL_ID.to_string()
        } else {
            snapshot.system_model.id.clone()
        };
        if !snapshot
            .available_models
            .iter()
            .any(|candidate| candidate == &model)
        {
            snapshot.available_models.push(model.clone());
        }
        snapshot.ready_model = Some(model);
    } else {
        snapshot.ready_model = None;
    }
}

fn sync_snapshot_adapter_inventory(
    snapshot: &mut AppleFmBridgeSnapshot,
    client: &AppleFmBridgeClient,
) -> Result<(), String> {
    let adapters = client.list_adapters().map_err(|error| error.to_string())?;
    snapshot.loaded_adapters = adapters.adapters;
    if let Some(attach_supported) = adapters.attach_supported {
        snapshot.adapter_attach_supported = attach_supported;
    }
    Ok(())
}

fn mark_snapshot_request_success(snapshot: &mut AppleFmBridgeSnapshot, served_model: Option<&str>) {
    snapshot.reachable = true;
    snapshot.model_available = true;
    snapshot.unavailable_reason = None;
    snapshot.bridge_status = Some(AppleFmBridgeStatus::Running.label().to_string());
    snapshot.availability_message = Some("Foundation Models is available".to_string());
    let model = served_model
        .map(str::to_string)
        .or_else(|| snapshot.ready_model.clone())
        .or_else(|| {
            (!snapshot.system_model.id.trim().is_empty()).then(|| snapshot.system_model.id.clone())
        })
        .unwrap_or_else(|| DEFAULT_APPLE_FM_MODEL_ID.to_string());
    if !snapshot
        .available_models
        .iter()
        .any(|candidate| candidate == &model)
    {
        snapshot.available_models.push(model.clone());
    }
    snapshot.ready_model = Some(model);
}

impl AppleFmLocalBridge {
    fn ensure_running(
        &mut self,
        config: &AppleFmBridgeConfig,
        client: &AppleFmBridgeClient,
        snapshot: &mut AppleFmBridgeSnapshot,
        emit: Option<&Sender<AppleFmBridgeUpdate>>,
    ) -> Result<(), String> {
        if !cfg!(target_os = "macos") {
            self.status = AppleFmBridgeStatus::UnsupportedPlatform;
            reset_snapshot_health(snapshot);
            snapshot.bridge_status = Some(self.status.label().to_string());
            snapshot.last_error =
                Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());
            snapshot.last_action = Some("Apple FM bridge unavailable on this platform".to_string());
            emit_snapshot_if(snapshot, emit);
            return Err(snapshot.last_error.clone().unwrap_or_default());
        }

        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                self.child = None;
                self.status = AppleFmBridgeStatus::Failed;
                reset_snapshot_health(snapshot);
                snapshot.last_error =
                    Some(format!("Apple FM bridge exited unexpectedly: {status}"));
            }
        }
        if self.child.is_none() {
            if let Ok(health) = client.health() {
                self.status = AppleFmBridgeStatus::Running;
                hydrate_snapshot_from_health(snapshot, &health);
                snapshot.bridge_status = Some(self.status.label().to_string());
                snapshot.last_action =
                    Some("Apple FM bridge already responding on configured URL.".to_string());
                snapshot.last_error = None;
                emit_snapshot_if(snapshot, emit);
                return Ok(());
            }

            snapshot.last_action = Some("Looking for Apple FM bridge binary...".to_string());
            snapshot.last_error = None;
            emit_snapshot_if(snapshot, emit);

            let binary = find_bridge_binary().or_else(|| {
                snapshot.last_action =
                    Some("Apple FM bridge not found; running build.sh to build it...".to_string());
                emit_snapshot_if(snapshot, emit);
                let built = try_build_bridge(snapshot);
                emit_snapshot_if(snapshot, emit);
                if built {
                    snapshot.last_action = Some(
                        "Build finished; looking for Apple FM bridge binary again...".to_string(),
                    );
                    emit_snapshot_if(snapshot, emit);
                }
                find_bridge_binary()
            });

            let binary = binary.ok_or_else(|| {
                let fix_msg = if running_from_app_bundle() {
                    APPLE_FM_FIX_SHIPPED_MISSING
                } else {
                    APPLE_FM_FIX_BUILD
                };
                reset_snapshot_health(snapshot);
                snapshot.last_action = Some("Apple FM bridge binary not found.".to_string());
                snapshot.last_error = Some(fix_msg.to_string());
                emit_snapshot_if(snapshot, emit);
                format!("Apple FM bridge binary not found. {fix_msg}")
            })?;

            snapshot.last_action =
                Some("Found Apple FM bridge binary; starting process...".to_string());
            snapshot.last_error = None;
            emit_snapshot_if(snapshot, emit);

            let port = port_from_base_url(config.base_url.as_str()).unwrap_or(11435);
            let child = Command::new(&binary)
                .arg(port.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| {
                    reset_snapshot_health(snapshot);
                    snapshot.last_action =
                        Some("Apple FM bridge process failed to start.".to_string());
                    snapshot.last_error = Some(error.to_string());
                    emit_snapshot_if(snapshot, emit);
                    format!("failed to start Apple FM bridge: {error}")
                })?;
            self.child = Some(child);
            self.status = AppleFmBridgeStatus::Starting;
            snapshot.bridge_status = Some(self.status.label().to_string());
            snapshot.last_action =
                Some("Apple FM bridge process started; waiting for health...".to_string());
            snapshot.last_error = None;
            emit_snapshot_if(snapshot, emit);
        }

        snapshot.last_action = Some("Waiting for Apple FM bridge to respond...".to_string());
        emit_snapshot_if(snapshot, emit);
        wait_for_bridge_health(client).map_err(|e| {
            reset_snapshot_health(snapshot);
            snapshot.last_action = Some("Apple FM bridge health check timed out.".to_string());
            snapshot.last_error = Some(e.clone());
            emit_snapshot_if(snapshot, emit);
            e
        })?;
        self.status = AppleFmBridgeStatus::Running;
        snapshot.bridge_status = Some(self.status.label().to_string());
        snapshot.last_action = Some("Apple FM bridge running and healthy.".to_string());
        snapshot.last_error = None;
        emit_snapshot_if(snapshot, emit);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.status = AppleFmBridgeStatus::NotStarted;
    }
}

struct AppleFmBridgeState {
    config: AppleFmBridgeConfig,
    snapshot: AppleFmBridgeSnapshot,
    client: Option<AppleFmBridgeClient>,
    client_error: Option<String>,
    bridge: AppleFmLocalBridge,
}

impl AppleFmBridgeState {
    fn new(config: AppleFmBridgeConfig) -> Self {
        let (client, client_error) = match HttpClient::builder()
            .timeout(REQUEST_TIMEOUT)
            .no_proxy()
            .build()
        {
            Ok(http_client) => {
                match AppleFmBridgeClient::with_http_client(config.base_url.clone(), http_client) {
                    Ok(client) => (Some(client), None),
                    Err(error) => (None, Some(error.to_string())),
                }
            }
            Err(error) => (
                None,
                Some(format!(
                    "failed to initialize Apple FM HTTP client: {error}"
                )),
            ),
        };
        let mut snapshot = AppleFmBridgeSnapshot {
            base_url: config.base_url.clone(),
            bridge_status: Some(if cfg!(target_os = "macos") {
                AppleFmBridgeStatus::NotStarted.label().to_string()
            } else {
                AppleFmBridgeStatus::UnsupportedPlatform.label().to_string()
            }),
            last_action: Some("Apple FM bridge worker starting".to_string()),
            ..AppleFmBridgeSnapshot::default()
        };
        if !cfg!(target_os = "macos") {
            snapshot.last_error =
                Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());
        }
        if let Some(error) = client_error.clone() {
            snapshot.last_error = Some(error);
        }
        Self {
            config,
            snapshot,
            client,
            client_error,
            bridge: AppleFmLocalBridge::default(),
        }
    }

    fn publish_snapshot(&self, update_tx: &Sender<AppleFmBridgeUpdate>) {
        let _ = update_tx.send(AppleFmBridgeUpdate::Snapshot(Box::new(
            self.snapshot.clone(),
        )));
    }

    fn handle_refresh(&mut self, update_tx: &Sender<AppleFmBridgeUpdate>) {
        self.snapshot.base_url = self.config.base_url.clone();
        self.snapshot.refreshed_at = Some(Instant::now());

        self.snapshot.last_action = Some("Checking Apple FM bridge health...".to_string());
        self.publish_snapshot(update_tx);

        if !cfg!(target_os = "macos") {
            reset_snapshot_health(&mut self.snapshot);
            self.snapshot.bridge_status =
                Some(AppleFmBridgeStatus::UnsupportedPlatform.label().to_string());
            self.snapshot.last_action =
                Some("Apple FM bridge unsupported on this platform".to_string());
            self.snapshot.last_error =
                Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());
            self.publish_snapshot(update_tx);
            return;
        }

        let Some(client) = self.client.as_ref() else {
            reset_snapshot_health(&mut self.snapshot);
            self.snapshot.last_action =
                Some("Apple FM refresh failed: HTTP client unavailable".to_string());
            self.snapshot.last_error = Some(
                self.client_error
                    .clone()
                    .unwrap_or_else(|| "Apple FM HTTP client unavailable".to_string()),
            );
            self.publish_snapshot(update_tx);
            return;
        };

        match client.health() {
            Ok(health) => {
                let system_model_status = health.system_model_availability();
                hydrate_snapshot_from_health(&mut self.snapshot, &health);
                self.snapshot.bridge_status =
                    Some(AppleFmBridgeStatus::Running.label().to_string());
                self.snapshot.last_error = None;
                self.snapshot.last_action =
                    Some("Apple FM bridge health OK; listing models...".to_string());
                self.publish_snapshot(update_tx);
                match client.list_models() {
                    Ok(models) => {
                        self.snapshot.available_models = models.model_ids();
                        self.snapshot.ready_model = system_model_status.available.then(|| {
                            self.snapshot
                                .available_models
                                .first()
                                .cloned()
                                .unwrap_or_else(|| self.snapshot.system_model.id.clone())
                        });
                        match sync_snapshot_adapter_inventory(&mut self.snapshot, client) {
                            Ok(()) => {
                                self.snapshot.last_action = Some(
                                    if system_model_status.available {
                                        format!(
                                            "Refreshed Apple FM bridge health; model ready with {} loaded adapter(s).",
                                            self.snapshot.loaded_adapters.len()
                                        )
                                    } else {
                                        "Refreshed Apple FM bridge health; system model not available yet."
                                        .to_string()
                                    },
                                );
                            }
                            Err(error) => {
                                self.snapshot.last_error = Some(error);
                                self.snapshot.last_action = Some(
                                    "Apple FM bridge health OK but adapter inventory refresh failed."
                                        .to_string(),
                                );
                            }
                        }
                    }
                    Err(error) => {
                        self.snapshot.last_error = Some(error.to_string());
                        self.snapshot.ready_model = system_model_status
                            .available
                            .then(|| self.snapshot.system_model.id.clone());
                        self.snapshot.last_action =
                            Some("Apple FM bridge health OK but list_models failed.".to_string());
                    }
                }
            }
            Err(error) => {
                reset_snapshot_health(&mut self.snapshot);
                self.snapshot.bridge_status = Some(self.bridge.status.label().to_string());
                self.snapshot.last_error = Some(error.to_string());
                self.snapshot.last_action =
                    Some("Apple FM bridge health check failed.".to_string());
            }
        }
        self.publish_snapshot(update_tx);
    }

    fn handle_ensure_bridge_running(&mut self, update_tx: &Sender<AppleFmBridgeUpdate>) {
        let Some(client) = self.client.as_ref() else {
            reset_snapshot_health(&mut self.snapshot);
            self.snapshot.last_error = Some(
                self.client_error
                    .clone()
                    .unwrap_or_else(|| "Apple FM HTTP client unavailable".to_string()),
            );
            self.snapshot.last_action = Some("Apple FM bridge start failed".to_string());
            self.publish_snapshot(update_tx);
            return;
        };
        match self
            .bridge
            .ensure_running(&self.config, client, &mut self.snapshot, Some(update_tx))
        {
            Ok(()) => {
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some("Apple FM bridge running".to_string());
                self.handle_refresh(update_tx);
            }
            Err(error) => {
                self.snapshot.bridge_status = Some(self.bridge.status.label().to_string());
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action =
                    Some("Apple FM bridge start failed. See error for steps.".to_string());
                self.publish_snapshot(update_tx);
            }
        }
    }

    fn handle_generate(
        &mut self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        job: AppleFmGenerateJob,
    ) {
        let Some(client) = self.client.as_ref() else {
            let _ = update_tx.send(AppleFmBridgeUpdate::Failed(AppleFmExecutionFailed {
                request_id: job.request_id,
                error: self
                    .client_error
                    .clone()
                    .unwrap_or_else(|| "Apple FM HTTP client unavailable".to_string()),
            }));
            return;
        };

        if self.config.auto_start {
            let _ = self
                .bridge
                .ensure_running(&self.config, client, &mut self.snapshot, None);
        }

        let model = job
            .requested_model
            .clone()
            .or_else(|| self.snapshot.ready_model.clone())
            .unwrap_or_else(|| self.snapshot.system_model.id.clone());

        tracing::info!(
            target: "autopilot_desktop::provider",
            "Apple FM bridge dispatching request_id={} model={} prompt_chars={}",
            job.request_id,
            model,
            job.prompt.chars().count()
        );

        let _ = update_tx.send(AppleFmBridgeUpdate::Started(AppleFmExecutionStarted {
            request_id: job.request_id.clone(),
            model: model.clone(),
        }));

        let start = Instant::now();
        match client.completion_from_prompt(
            job.prompt.clone(),
            Some(model.clone()),
            Some(1024),
            None,
        ) {
            Ok(result) => {
                let total_duration_ns =
                    Some(start.elapsed().as_nanos().min(u64::MAX as u128) as u64);
                let metrics = LocalInferenceExecutionMetrics {
                    total_duration_ns,
                    load_duration_ns: None,
                    prompt_eval_count: result.prompt_tokens,
                    prompt_eval_duration_ns: None,
                    eval_count: result.completion_tokens,
                    eval_duration_ns: None,
                };
                let normalized_options_json = "{}".to_string();
                let provenance = LocalInferenceExecutionProvenance {
                    backend: "apple_foundation_models".to_string(),
                    requested_model: job.requested_model.clone(),
                    served_model: result.model.clone(),
                    normalized_prompt_digest: sha256_prefixed_text(job.prompt.as_str()),
                    normalized_options_json: normalized_options_json.clone(),
                    normalized_options_digest: sha256_prefixed_text(
                        normalized_options_json.as_str(),
                    ),
                    base_url: self.config.base_url.trim_end_matches('/').to_string(),
                    total_duration_ns: metrics.total_duration_ns,
                    load_duration_ns: None,
                    prompt_token_count: result.prompt_tokens,
                    generated_token_count: result.completion_tokens,
                    warm_start: None,
                };
                self.snapshot.reachable = true;
                self.snapshot.model_available = true;
                self.snapshot.ready_model = Some(result.model.clone());
                self.snapshot.last_error = None;
                self.snapshot.last_request_id = Some(job.request_id.clone());
                self.snapshot.last_metrics = Some(metrics.clone());
                self.snapshot.last_action = Some(format!(
                    "Completed Apple FM generation for {}",
                    job.request_id
                ));
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                tracing::info!(
                    target: "autopilot_desktop::provider",
                    "Apple FM bridge completed request_id={} model={} output_chars={} total_duration_ms={}",
                    job.request_id,
                    result.model,
                    result.output.chars().count(),
                    start.elapsed().as_millis()
                );
                let _ = update_tx.send(AppleFmBridgeUpdate::Completed(AppleFmExecutionCompleted {
                    request_id: job.request_id,
                    model: result.model,
                    output: result.output,
                    metrics,
                    provenance,
                }));
            }
            Err(error) => {
                self.snapshot.last_error = Some(error.to_string());
                self.snapshot.last_action = Some("Apple FM generation failed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                tracing::error!(
                    target: "autopilot_desktop::provider",
                    "Apple FM bridge failed request_id={} error={}",
                    job.request_id,
                    error
                );
                let _ = update_tx.send(AppleFmBridgeUpdate::Failed(AppleFmExecutionFailed {
                    request_id: job.request_id,
                    error: error.to_string(),
                }));
            }
        }
    }

    fn handle_workbench(
        &mut self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        command: AppleFmWorkbenchCommand,
    ) {
        let Some(client) = self.client.clone() else {
            self.publish_workbench_failed(
                update_tx,
                command.request_id,
                command.operation.label().to_string(),
                self.client_error
                    .clone()
                    .unwrap_or_else(|| "Apple FM HTTP client unavailable".to_string()),
            );
            return;
        };

        if self.config.auto_start {
            let _ = self
                .bridge
                .ensure_running(&self.config, &client, &mut self.snapshot, None);
        }

        self.publish_workbench_started(
            update_tx,
            command.request_id.as_str(),
            command.operation.label(),
        );

        match self.execute_workbench_command(&client, update_tx, &command) {
            Ok(completed) => {
                let adapter_sync_error = if self.snapshot.adapter_inventory_supported
                    || matches!(
                        command.operation,
                        AppleFmWorkbenchOperation::LoadAdapter
                            | AppleFmWorkbenchOperation::UnloadAdapter
                            | AppleFmWorkbenchOperation::AttachSessionAdapter
                            | AppleFmWorkbenchOperation::DetachSessionAdapter
                    ) {
                    sync_snapshot_adapter_inventory(&mut self.snapshot, &client).err()
                } else {
                    None
                };
                mark_snapshot_request_success(&mut self.snapshot, completed.model.as_deref());
                self.snapshot.last_error = adapter_sync_error.clone();
                self.snapshot.last_request_id = Some(completed.request_id.clone());
                self.snapshot.last_action = Some(match adapter_sync_error {
                    Some(error) => format!(
                        "Apple FM workbench completed {} but adapter inventory refresh failed: {}",
                        completed.operation, error
                    ),
                    None => format!("Apple FM workbench completed {}", completed.operation),
                });
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                self.publish_workbench_update(
                    update_tx,
                    AppleFmWorkbenchUpdate::Completed(completed),
                );
            }
            Err(error) => {
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_request_id = Some(command.request_id.clone());
                self.snapshot.last_action = Some(format!(
                    "Apple FM workbench failed {}",
                    command.operation.label()
                ));
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                self.publish_workbench_failed(
                    update_tx,
                    command.request_id,
                    command.operation.label().to_string(),
                    error,
                );
            }
        }
    }

    fn handle_mission_control_summary(
        &mut self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        command: AppleFmMissionControlSummaryCommand,
    ) {
        let Some(client) = self.client.clone() else {
            self.publish_mission_control_summary_update(
                update_tx,
                AppleFmMissionControlSummaryUpdate::Failed(AppleFmMissionControlSummaryFailed {
                    request_id: command.request_id,
                    error: self
                        .client_error
                        .clone()
                        .unwrap_or_else(|| "Apple FM HTTP client unavailable".to_string()),
                }),
            );
            return;
        };

        if self.config.auto_start {
            let _ = self
                .bridge
                .ensure_running(&self.config, &client, &mut self.snapshot, None);
        }

        self.publish_mission_control_summary_update(
            update_tx,
            AppleFmMissionControlSummaryUpdate::Started(AppleFmMissionControlSummaryStarted {
                request_id: command.request_id.clone(),
            }),
        );

        match self.execute_mission_control_summary(&client, update_tx, &command) {
            Ok(completed) => {
                mark_snapshot_request_success(&mut self.snapshot, completed.model.as_deref());
                self.snapshot.last_error = None;
                self.snapshot.last_request_id = Some(completed.request_id.clone());
                self.snapshot.last_action =
                    Some("Apple FM Mission Control summary completed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                self.publish_mission_control_summary_update(
                    update_tx,
                    AppleFmMissionControlSummaryUpdate::Completed(completed),
                );
            }
            Err(error) => {
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_request_id = Some(command.request_id.clone());
                self.snapshot.last_action =
                    Some("Apple FM Mission Control summary failed".to_string());
                self.snapshot.refreshed_at = Some(Instant::now());
                self.publish_snapshot(update_tx);
                self.publish_mission_control_summary_update(
                    update_tx,
                    AppleFmMissionControlSummaryUpdate::Failed(
                        AppleFmMissionControlSummaryFailed {
                            request_id: command.request_id,
                            error,
                        },
                    ),
                );
            }
        }
    }

    fn execute_mission_control_summary(
        &mut self,
        client: &AppleFmBridgeClient,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        command: &AppleFmMissionControlSummaryCommand,
    ) -> Result<AppleFmMissionControlSummaryCompleted, String> {
        let session = client
            .create_session(&AppleFmSessionCreateRequest {
                instructions: Some(command.instructions.clone()),
                model: requested_system_model(command.requested_model.as_deref()),
                tools: Vec::new(),
                adapter: None,
                tool_callback: None,
                transcript_json: None,
                transcript: None,
            })
            .map_err(|error| error.to_string())?;
        let session_id = session.id.clone();
        let request_id = command.request_id.clone();
        let async_client = AppleFmAsyncBridgeClient::new(self.config.base_url.clone())
            .map_err(|error| error.to_string())?;
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| format!("build Apple FM stream runtime: {error}"))?;
        let request = AppleFmSessionRespondRequest {
            prompt: command.prompt.clone(),
            options: command.options.clone(),
            adapter: None,
        };
        let mut stream = match runtime
            .block_on(async_client.stream_session_response(session_id.as_str(), &request))
        {
            Ok(stream) => stream,
            Err(error) => {
                let _ = client.delete_session(session_id.as_str());
                return Err(error.to_string());
            }
        };
        let mut output = String::new();
        let mut model = Some(session.model.id.clone());
        let mut usage_json = None::<String>;
        let mut last_chars = 0usize;
        let stream_result = runtime.block_on(async {
            while let Some(event) = stream.next().await {
                match event {
                    Ok(event) => {
                        model = Some(event.model.clone());
                        let chars = event.output.chars().count();
                        let delta = if chars >= last_chars {
                            event.output.chars().skip(last_chars).collect::<String>()
                        } else {
                            event.output.clone()
                        };
                        last_chars = chars;
                        output = event.output.clone();
                        if let Some(usage) = event.usage.as_ref() {
                            usage_json = Some(pretty_json(usage));
                        }
                        if !delta.is_empty() {
                            self.publish_mission_control_summary_update(
                                update_tx,
                                AppleFmMissionControlSummaryUpdate::Delta(
                                    AppleFmMissionControlSummaryDelta {
                                        request_id: request_id.clone(),
                                        delta,
                                    },
                                ),
                            );
                        } else if event.kind == AppleFmTextStreamEventKind::Completed
                            && output.is_empty()
                        {
                            self.publish_mission_control_summary_update(
                                update_tx,
                                AppleFmMissionControlSummaryUpdate::Delta(
                                    AppleFmMissionControlSummaryDelta {
                                        request_id: request_id.clone(),
                                        delta: "[no summary output]".to_string(),
                                    },
                                ),
                            );
                        }
                    }
                    Err(error) => return Err::<(), String>(error.to_string()),
                }
            }
            Ok::<(), String>(())
        });
        let delete_result = client.delete_session(session_id.as_str());
        if let Err(error) = stream_result {
            let _ = delete_result;
            return Err(error);
        }
        if let Err(error) = delete_result {
            tracing::warn!(
                "Apple FM Mission Control summary session cleanup failed session_id={} error={}",
                session_id,
                error
            );
        }
        Ok(AppleFmMissionControlSummaryCompleted {
            request_id: command.request_id.clone(),
            summary: format!(
                "streamed Mission Control summary via {}",
                model.as_deref().unwrap_or("apple_foundation_models")
            ),
            model,
            response_text: output,
            usage_json,
        })
    }

    fn execute_workbench_command(
        &mut self,
        client: &AppleFmBridgeClient,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        command: &AppleFmWorkbenchCommand,
    ) -> Result<AppleFmWorkbenchCompleted, String> {
        match command.operation {
            AppleFmWorkbenchOperation::CreateSession => {
                let request = AppleFmSessionCreateRequest {
                    instructions: command.instructions.clone(),
                    model: requested_system_model(command.requested_model.as_deref()),
                    tools: Vec::new(),
                    adapter: resolved_adapter_selection(
                        command,
                        self.snapshot.loaded_adapters.as_slice(),
                    ),
                    tool_callback: None,
                    transcript_json: None,
                    transcript: None,
                };
                let session = match command.tool_mode {
                    AppleFmWorkbenchToolMode::None => client.create_session(&request),
                    mode => client.create_session_with_tools(&request, sample_tools(mode)?),
                }
                .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("created session {}", session.id),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: None,
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::InspectSession => {
                let session_id = required_session_id(command)?;
                let session = client
                    .session(session_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("loaded session {}", session.id),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: session.transcript_json.clone(),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::LoadAdapter => {
                let package_path = required_adapter_package_path(command)?;
                let adapter = client
                    .load_adapter(&AppleFmAdapterLoadRequest {
                        package_path,
                        requested_adapter_id: normalized_optional_text(
                            command.adapter_id.as_deref(),
                        ),
                    })
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("loaded adapter {}", adapter.adapter.adapter_id.as_str()),
                    model: None,
                    session_id: None,
                    session_adapter: None,
                    response_text: format!(
                        "adapter {} compatibility={} draft_model_present={} attached_sessions={}",
                        adapter.adapter.adapter_id,
                        adapter.compatibility.compatible,
                        adapter.draft_model_present,
                        adapter.attached_session_ids.len()
                    ),
                    adapter_json: Some(pretty_json(&adapter)),
                    session_json: None,
                    structured_json: None,
                    transcript_json: None,
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::UnloadAdapter => {
                let adapter_id = required_adapter_id(command)?;
                client
                    .unload_adapter(adapter_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("unloaded adapter {adapter_id}"),
                    model: None,
                    session_id: None,
                    session_adapter: None,
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: None,
                    structured_json: None,
                    transcript_json: None,
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::AttachSessionAdapter => {
                let session_id = required_session_id(command)?;
                let adapter =
                    required_adapter_selection(command, self.snapshot.loaded_adapters.as_slice())?;
                let session = client
                    .attach_session_adapter(
                        session_id.as_str(),
                        &AppleFmAdapterAttachRequest { adapter },
                    )
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!(
                        "attached adapter {} to session {}",
                        session
                            .adapter
                            .as_ref()
                            .map(|adapter| adapter.adapter_id.as_str())
                            .unwrap_or("unknown"),
                        session.id
                    ),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: session.transcript_json.clone(),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::DetachSessionAdapter => {
                let session_id = required_session_id(command)?;
                let session = client
                    .detach_session_adapter(session_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("detached adapter from session {}", session.id),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: session.transcript_json.clone(),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::RunText => {
                let prompt = required_prompt(command)?;
                let response = client
                    .generate_text(&AppleFmTextGenerationRequest {
                        model: command.requested_model.clone(),
                        prompt,
                        options: command.options.clone(),
                        adapter: resolved_adapter_selection(
                            command,
                            self.snapshot.loaded_adapters.as_slice(),
                        ),
                    })
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("generated text via {}", response.model),
                    model: Some(response.model.clone()),
                    session_id: None,
                    session_adapter: None,
                    response_text: response.output.clone(),
                    adapter_json: None,
                    session_json: None,
                    structured_json: None,
                    transcript_json: None,
                    usage_json: response.usage.as_ref().map(pretty_json),
                })
            }
            AppleFmWorkbenchOperation::RunChat => {
                let prompt = required_prompt(command)?;
                let response = client
                    .chat_completion(&AppleFmChatCompletionRequest {
                        model: command.requested_model.clone(),
                        messages: build_chat_messages(command.instructions.as_deref(), prompt),
                        temperature: command
                            .options
                            .as_ref()
                            .and_then(|options| options.temperature),
                        max_tokens: command
                            .options
                            .as_ref()
                            .and_then(|options| options.maximum_response_tokens),
                        options: command.options.clone(),
                        adapter: resolved_adapter_selection(
                            command,
                            self.snapshot.loaded_adapters.as_slice(),
                        ),
                        stream: false,
                    })
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("generated chat completion via {}", response.model),
                    model: Some(response.model.clone()),
                    session_id: None,
                    session_adapter: None,
                    response_text: response
                        .first_text_content()
                        .unwrap_or_default()
                        .to_string(),
                    adapter_json: None,
                    session_json: None,
                    structured_json: None,
                    transcript_json: None,
                    usage_json: response.usage.as_ref().map(pretty_json),
                })
            }
            AppleFmWorkbenchOperation::RunSession => {
                let session_id = required_session_id(command)?;
                let prompt = required_prompt(command)?;
                let response = client
                    .respond_in_session(
                        session_id.as_str(),
                        &AppleFmSessionRespondRequest {
                            prompt,
                            options: command.options.clone(),
                            adapter: resolved_adapter_selection(
                                command,
                                self.snapshot.loaded_adapters.as_slice(),
                            ),
                        },
                    )
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("session {} responded", response.session.id),
                    model: Some(response.model.clone()),
                    session_id: Some(response.session.id.clone()),
                    session_adapter: response.session.adapter.clone(),
                    response_text: response.output.clone(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&response.session)),
                    structured_json: None,
                    transcript_json: response.session.transcript_json.clone(),
                    usage_json: response.usage.as_ref().map(pretty_json),
                })
            }
            AppleFmWorkbenchOperation::RunStream => {
                let session_id = required_session_id(command)?;
                let prompt = required_prompt(command)?;
                let async_client = AppleFmAsyncBridgeClient::new(self.config.base_url.clone())
                    .map_err(|error| error.to_string())?;
                let runtime = TokioRuntimeBuilder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| format!("build Apple FM stream runtime: {error}"))?;
                let request = AppleFmSessionRespondRequest {
                    prompt,
                    options: command.options.clone(),
                    adapter: resolved_adapter_selection(
                        command,
                        self.snapshot.loaded_adapters.as_slice(),
                    ),
                };
                let mut stream = runtime
                    .block_on(async_client.stream_session_response(session_id.as_str(), &request))
                    .map_err(|error| error.to_string())?;
                let request_id = command.request_id.clone();
                let mut output = String::new();
                let mut model = None::<String>;
                let mut session_json = None::<String>;
                let mut transcript_json = None::<String>;
                let mut usage_json = None::<String>;
                let mut session_adapter = None::<AppleFmAdapterSelection>;
                let mut last_chars = 0usize;
                runtime.block_on(async {
                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(event) => {
                                model = Some(event.model.clone());
                                let chars = event.output.chars().count();
                                let delta = if chars >= last_chars {
                                    event.output.chars().skip(last_chars).collect::<String>()
                                } else {
                                    event.output.clone()
                                };
                                last_chars = chars;
                                output = event.output.clone();
                                if let Some(session) = event.session.as_ref() {
                                    session_adapter = session.adapter.clone();
                                    transcript_json = session.transcript_json.clone();
                                    session_json = Some(pretty_json(session));
                                }
                                if let Some(usage) = event.usage.as_ref() {
                                    usage_json = Some(pretty_json(usage));
                                }
                                let event_label = match event.kind {
                                    AppleFmTextStreamEventKind::Snapshot => "snapshot",
                                    AppleFmTextStreamEventKind::Completed => "completed",
                                };
                                let line = if delta.trim().is_empty() {
                                    format!("{event_label} chars={chars}")
                                } else {
                                    format!("{event_label} delta: {}", delta.trim())
                                };
                                self.publish_workbench_event(
                                    update_tx,
                                    request_id.as_str(),
                                    AppleFmWorkbenchLogLevel::Info,
                                    line,
                                );
                            }
                            Err(error) => {
                                self.publish_workbench_event(
                                    update_tx,
                                    request_id.as_str(),
                                    AppleFmWorkbenchLogLevel::Error,
                                    format!("stream error: {error}"),
                                );
                                return Err::<(), String>(error.to_string());
                            }
                        }
                    }
                    Ok::<(), String>(())
                })?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("stream completed for {}", session_id),
                    model,
                    session_id: Some(session_id),
                    session_adapter,
                    response_text: output,
                    adapter_json: None,
                    session_json,
                    structured_json: None,
                    transcript_json,
                    usage_json,
                })
            }
            AppleFmWorkbenchOperation::RunStructured => {
                let schema = required_schema(command)?;
                if let Some(session_id) = normalized_optional_text(command.session_id.as_deref()) {
                    let prompt = required_prompt(command)?;
                    let response = client
                        .respond_structured_in_session(
                            session_id.as_str(),
                            &AppleFmSessionStructuredGenerationRequest {
                                prompt,
                                schema,
                                options: command.options.clone(),
                                adapter: resolved_adapter_selection(
                                    command,
                                    self.snapshot.loaded_adapters.as_slice(),
                                ),
                            },
                        )
                        .map_err(|error| error.to_string())?;
                    Ok(AppleFmWorkbenchCompleted {
                        request_id: command.request_id.clone(),
                        operation: command.operation.label().to_string(),
                        summary: format!(
                            "session {} structured response completed",
                            response.session.id
                        ),
                        model: Some(response.model.clone()),
                        session_id: Some(response.session.id.clone()),
                        session_adapter: response.session.adapter.clone(),
                        response_text: String::new(),
                        adapter_json: None,
                        session_json: Some(pretty_json(&response.session)),
                        structured_json: Some(pretty_json(&response.content.content)),
                        transcript_json: response.session.transcript_json.clone(),
                        usage_json: response.usage.as_ref().map(pretty_json),
                    })
                } else {
                    let prompt = required_prompt(command)?;
                    let response = client
                        .generate_structured(&AppleFmStructuredGenerationRequest {
                            model: command.requested_model.clone(),
                            prompt,
                            schema,
                            options: command.options.clone(),
                            adapter: resolved_adapter_selection(
                                command,
                                self.snapshot.loaded_adapters.as_slice(),
                            ),
                        })
                        .map_err(|error| error.to_string())?;
                    Ok(AppleFmWorkbenchCompleted {
                        request_id: command.request_id.clone(),
                        operation: command.operation.label().to_string(),
                        summary: format!("one-shot structured response via {}", response.model),
                        model: Some(response.model.clone()),
                        session_id: None,
                        session_adapter: None,
                        response_text: String::new(),
                        adapter_json: None,
                        session_json: None,
                        structured_json: Some(pretty_json(&response.content.content)),
                        transcript_json: None,
                        usage_json: response.usage.as_ref().map(pretty_json),
                    })
                }
            }
            AppleFmWorkbenchOperation::ExportTranscript => {
                let session_id = required_session_id(command)?;
                let transcript = client
                    .session_transcript(session_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("exported transcript for {}", session_id),
                    model: None,
                    session_id: Some(session_id),
                    session_adapter: None,
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: None,
                    structured_json: None,
                    transcript_json: Some(pretty_json(&transcript)),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::RestoreTranscript => {
                let transcript_json = command
                    .transcript_json
                    .as_deref()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| "Transcript JSON is required for restore".to_string())?;
                let mut request = AppleFmSessionCreateRequest::from_transcript_json(
                    transcript_json,
                    requested_system_model(command.requested_model.as_deref()),
                    Vec::new(),
                );
                request.adapter =
                    resolved_adapter_selection(command, self.snapshot.loaded_adapters.as_slice());
                let session = match command.tool_mode {
                    AppleFmWorkbenchToolMode::None => client.create_session(&request),
                    mode => client.create_session_with_tools(&request, sample_tools(mode)?),
                }
                .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("restored session {}", session.id),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: session.transcript_json.clone(),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::ResetSession => {
                let session_id = required_session_id(command)?;
                let session = client
                    .reset_session(session_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("reset session {}", session.id),
                    model: Some(session.model.id.clone()),
                    session_id: Some(session.id.clone()),
                    session_adapter: session.adapter.clone(),
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: Some(pretty_json(&session)),
                    structured_json: None,
                    transcript_json: session.transcript_json.clone(),
                    usage_json: None,
                })
            }
            AppleFmWorkbenchOperation::DeleteSession => {
                let session_id = required_session_id(command)?;
                client
                    .delete_session(session_id.as_str())
                    .map_err(|error| error.to_string())?;
                Ok(AppleFmWorkbenchCompleted {
                    request_id: command.request_id.clone(),
                    operation: command.operation.label().to_string(),
                    summary: format!("deleted session {}", session_id),
                    model: None,
                    session_id: None,
                    session_adapter: None,
                    response_text: String::new(),
                    adapter_json: None,
                    session_json: None,
                    structured_json: None,
                    transcript_json: None,
                    usage_json: None,
                })
            }
        }
    }

    fn publish_workbench_started(
        &self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        request_id: &str,
        operation: &str,
    ) {
        self.publish_workbench_update(
            update_tx,
            AppleFmWorkbenchUpdate::Started(AppleFmWorkbenchStarted {
                request_id: request_id.to_string(),
                operation: operation.to_string(),
            }),
        );
    }

    fn publish_workbench_event(
        &self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        request_id: &str,
        level: AppleFmWorkbenchLogLevel,
        line: impl Into<String>,
    ) {
        self.publish_workbench_update(
            update_tx,
            AppleFmWorkbenchUpdate::Event(AppleFmWorkbenchEvent {
                request_id: request_id.to_string(),
                level,
                line: line.into(),
            }),
        );
    }

    fn publish_workbench_failed(
        &self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        request_id: String,
        operation: String,
        error: String,
    ) {
        self.publish_workbench_update(
            update_tx,
            AppleFmWorkbenchUpdate::Failed(AppleFmWorkbenchFailed {
                request_id,
                operation,
                error,
            }),
        );
    }

    fn publish_workbench_update(
        &self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        update: AppleFmWorkbenchUpdate,
    ) {
        let _ = update_tx.send(AppleFmBridgeUpdate::Workbench(Box::new(update)));
    }

    fn publish_mission_control_summary_update(
        &self,
        update_tx: &Sender<AppleFmBridgeUpdate>,
        update: AppleFmMissionControlSummaryUpdate,
    ) {
        let _ = update_tx.send(AppleFmBridgeUpdate::MissionControlSummary(Box::new(update)));
    }
}

fn run_apple_fm_loop(
    command_rx: Receiver<AppleFmBridgeCommand>,
    update_tx: Sender<AppleFmBridgeUpdate>,
    shutdown_rx: Receiver<()>,
    config: AppleFmBridgeConfig,
) {
    let mut state = AppleFmBridgeState::new(config);
    state.publish_snapshot(&update_tx);

    loop {
        if shutdown_rx.try_recv().is_ok() {
            state.bridge.stop();
            return;
        }

        match command_rx.recv_timeout(LANE_POLL) {
            Ok(AppleFmBridgeCommand::Refresh) => state.handle_refresh(&update_tx),
            Ok(AppleFmBridgeCommand::EnsureBridgeRunning) => {
                state.handle_ensure_bridge_running(&update_tx)
            }
            Ok(AppleFmBridgeCommand::StopBridge) => {
                state.bridge.stop();
                state.handle_refresh(&update_tx);
            }
            Ok(AppleFmBridgeCommand::Generate(job)) => state.handle_generate(&update_tx, job),
            Ok(AppleFmBridgeCommand::Workbench(command)) => {
                state.handle_workbench(&update_tx, command)
            }
            Ok(AppleFmBridgeCommand::MissionControlSummary(command)) => {
                state.handle_mission_control_summary(&update_tx, command)
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                state.bridge.stop();
                return;
            }
        }
    }
}

fn wait_for_bridge_health(client: &AppleFmBridgeClient) -> Result<(), String> {
    let deadline = Instant::now() + BRIDGE_STARTUP_TIMEOUT;
    while Instant::now() < deadline {
        if client.health().is_ok() {
            return Ok(());
        }
        std::thread::sleep(BRIDGE_HEALTH_POLL);
    }
    Err("Apple FM bridge health check timed out".to_string())
}

fn required_prompt(command: &AppleFmWorkbenchCommand) -> Result<String, String> {
    command
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "Prompt is required for this Apple FM workbench action".to_string())
}

fn required_session_id(command: &AppleFmWorkbenchCommand) -> Result<String, String> {
    normalized_optional_text(command.session_id.as_deref())
        .ok_or_else(|| "Session id is required for this Apple FM workbench action".to_string())
}

fn required_adapter_id(command: &AppleFmWorkbenchCommand) -> Result<String, String> {
    normalized_optional_text(command.adapter_id.as_deref())
        .ok_or_else(|| "Adapter id is required for this Apple FM workbench action".to_string())
}

fn required_adapter_package_path(command: &AppleFmWorkbenchCommand) -> Result<String, String> {
    normalized_optional_text(command.adapter_package_path.as_deref()).ok_or_else(|| {
        "Adapter package path is required for this Apple FM workbench action".to_string()
    })
}

fn required_schema(command: &AppleFmWorkbenchCommand) -> Result<AppleFmGenerationSchema, String> {
    let schema_json = command
        .schema_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Schema JSON is required for this Apple FM workbench action".to_string())?;
    AppleFmGenerationSchema::from_json_str(schema_json).map_err(|error| error.to_string())
}

fn normalized_optional_text(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn resolved_adapter_selection(
    command: &AppleFmWorkbenchCommand,
    loaded_adapters: &[AppleFmAdapterInventoryEntry],
) -> Option<AppleFmAdapterSelection> {
    let adapter_id = normalized_optional_text(command.adapter_id.as_deref())?;
    let package_digest = loaded_adapters
        .iter()
        .find(|entry| entry.adapter.adapter_id == adapter_id)
        .and_then(|entry| entry.adapter.package_digest.clone());
    Some(AppleFmAdapterSelection {
        adapter_id,
        package_digest,
    })
}

fn required_adapter_selection(
    command: &AppleFmWorkbenchCommand,
    loaded_adapters: &[AppleFmAdapterInventoryEntry],
) -> Result<AppleFmAdapterSelection, String> {
    resolved_adapter_selection(command, loaded_adapters)
        .ok_or_else(|| "Adapter id is required for this Apple FM workbench action".to_string())
}

fn requested_system_model(model: Option<&str>) -> Option<AppleFmSystemLanguageModel> {
    normalized_optional_text(model).map(|id| AppleFmSystemLanguageModel {
        id,
        ..AppleFmSystemLanguageModel::default()
    })
}

fn build_chat_messages(instructions: Option<&str>, prompt: String) -> Vec<AppleFmChatMessage> {
    let mut messages = Vec::new();
    if let Some(instructions) = normalized_optional_text(instructions) {
        messages.push(AppleFmChatMessage {
            role: AppleFmChatMessageRole::System,
            content: instructions,
        });
    }
    messages.push(AppleFmChatMessage {
        role: AppleFmChatMessageRole::User,
        content: prompt,
    });
    messages
}

fn pretty_json<T>(value: &T) -> String
where
    T: Serialize,
{
    serde_json::to_string_pretty(value).unwrap_or_else(|error| {
        format!(
            "{{\"error\":\"failed to serialize Apple FM workbench payload\",\"detail\":\"{}\"}}",
            error
        )
    })
}

fn sample_tools(mode: AppleFmWorkbenchToolMode) -> Result<Vec<Arc<dyn AppleFmTool>>, String> {
    match mode {
        AppleFmWorkbenchToolMode::None => Ok(Vec::new()),
        AppleFmWorkbenchToolMode::Demo => Ok(vec![
            Arc::new(DemoSecretTool::new()?) as Arc<dyn AppleFmTool>,
            Arc::new(DemoProfileTool::new()?) as Arc<dyn AppleFmTool>,
        ]),
        AppleFmWorkbenchToolMode::Failing => Ok(vec![
            Arc::new(FailingSecretTool::new()?) as Arc<dyn AppleFmTool>
        ]),
    }
}

#[derive(Clone)]
struct DemoSecretTool {
    definition: AppleFmToolDefinition,
}

impl DemoSecretTool {
    fn new() -> Result<Self, String> {
        Ok(Self {
            definition: AppleFmToolDefinition::new(
                "lookup_secret_code",
                Some("Return a deterministic secret code for a subject."),
                AppleFmGenerationSchema::from_json_str(
                    r#"{
                        "type": "object",
                        "properties": {
                            "subject": { "type": "string" }
                        },
                        "required": ["subject"]
                    }"#,
                )
                .map_err(|error| error.to_string())?,
            ),
        })
    }
}

impl AppleFmTool for DemoSecretTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let subject = arguments
            .property::<String>("subject")
            .map_err(|error| AppleFmToolCallError::new("lookup_secret_code", error.to_string()))?
            .unwrap_or_else(|| "unknown".to_string());
        Ok(json!({
            "subject": subject,
            "secret_code": "OA-314159",
            "source": "apple-fm-workbench",
        })
        .to_string())
    }
}

#[derive(Clone)]
struct DemoProfileTool {
    definition: AppleFmToolDefinition,
}

impl DemoProfileTool {
    fn new() -> Result<Self, String> {
        Ok(Self {
            definition: AppleFmToolDefinition::new(
                "lookup_user_profile",
                Some("Return a deterministic user profile record."),
                AppleFmGenerationSchema::from_json_str(
                    r#"{
                        "type": "object",
                        "properties": {
                            "username": { "type": "string" }
                        },
                        "required": ["username"]
                    }"#,
                )
                .map_err(|error| error.to_string())?,
            ),
        })
    }
}

impl AppleFmTool for DemoProfileTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let username = arguments
            .property::<String>("username")
            .map_err(|error| AppleFmToolCallError::new("lookup_user_profile", error.to_string()))?
            .unwrap_or_else(|| "unknown".to_string());
        Ok(json!({
            "username": username,
            "reputation": "trusted",
            "city": "Austin",
            "source": "apple-fm-workbench",
        })
        .to_string())
    }
}

#[derive(Clone)]
struct FailingSecretTool {
    definition: AppleFmToolDefinition,
}

impl FailingSecretTool {
    fn new() -> Result<Self, String> {
        Ok(Self {
            definition: DemoSecretTool::new()?.definition,
        })
    }
}

impl AppleFmTool for FailingSecretTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let subject = arguments
            .property::<String>("subject")
            .map_err(|error| AppleFmToolCallError::new("lookup_secret_code", error.to_string()))?
            .unwrap_or_else(|| "unknown".to_string());
        Err(AppleFmToolCallError::new(
            "lookup_secret_code",
            format!("intentional Apple FM workbench failure for {subject}"),
        ))
    }
}

fn port_from_base_url(base_url: &str) -> Option<u16> {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.port_or_known_default())
}

/// Walks up from the current executable path to find the repo root (directory
/// containing swift/foundation-bridge or bin/foundation-bridge). Ensures the
/// app finds the bridge regardless of current working directory.
fn find_repo_root_from_exe() -> Option<PathBuf> {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))?;
    loop {
        if !dir.as_os_str().is_empty()
            && (dir.join("swift/foundation-bridge").exists()
                || dir.join("bin/foundation-bridge").exists())
        {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

/// Tries to build the foundation-bridge via build.sh. Returns true if build
/// succeeded and the binary is now present. Updates snapshot.last_action for UI.
fn try_build_bridge(snapshot: &mut AppleFmBridgeSnapshot) -> bool {
    let root = match find_repo_root_from_exe() {
        Some(r) => r,
        None => {
            snapshot.last_action =
                Some("Cannot build: repo root not found from executable path.".to_string());
            return false;
        }
    };
    let bridge_dir = root.join("swift/foundation-bridge");
    if !bridge_dir.exists() {
        snapshot.last_action =
            Some("Cannot build: swift/foundation-bridge directory not found.".to_string());
        return false;
    }
    snapshot.last_action = Some("Building Apple FM bridge (running build.sh)...".to_string());
    let status = Command::new("./build.sh")
        .current_dir(&bridge_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status();
    let ok = matches!(status, Ok(s) if s.success());
    snapshot.last_action = Some(if ok {
        "Apple FM bridge build finished successfully.".to_string()
    } else {
        format!("Apple FM bridge build failed. {APPLE_FM_FIX_BUILD}")
    });
    ok
}

fn find_bridge_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_APPLE_FM_BRIDGE_BIN) {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    // CWD-relative (e.g. when run from repo root).
    let cwd_candidates = [
        PathBuf::from("bin/foundation-bridge"),
        PathBuf::from("swift/foundation-bridge/.build/release/foundation-bridge"),
        PathBuf::from(
            "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
        ),
    ];
    for candidate in &cwd_candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }

    // Exe-relative repo root (works when run from target/debug or anywhere under repo).
    if let Some(root) = find_repo_root_from_exe() {
        let repo_candidates = [
            root.join("bin/foundation-bridge"),
            root.join("swift/foundation-bridge/.build/release/foundation-bridge"),
            root.join(
                "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
            ),
        ];
        for candidate in &repo_candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }
    }

    // Bundled with the app: next to executable (e.g. MyApp.app/Contents/MacOS/foundation-bridge)
    // or in Resources (e.g. MyApp.app/Contents/Resources/foundation-bridge).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(macos_dir) = exe.parent() {
            let next_to_exe = macos_dir.join("foundation-bridge");
            if next_to_exe.exists() {
                return Some(next_to_exe);
            }
            if let Some(contents_dir) = macos_dir.parent() {
                let in_resources = contents_dir.join("Resources").join("foundation-bridge");
                if in_resources.exists() {
                    return Some(in_resources);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{
        AppleFmBridgeCommand, AppleFmBridgeConfig, AppleFmBridgeUpdate, AppleFmBridgeWorker,
        AppleFmMissionControlSummaryCommand, AppleFmMissionControlSummaryUpdate,
        AppleFmSystemLanguageModelGuardrails, AppleFmSystemLanguageModelUseCase,
        AppleFmWorkbenchCommand, AppleFmWorkbenchOperation, AppleFmWorkbenchToolMode,
        AppleFmWorkbenchUpdate,
    };
    use psionic_apple_fm::{AppleFmGenerationOptions, AppleFmSamplingMode};
    use std::collections::HashMap;
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    fn spawn_mock_bridge() -> (String, Arc<AtomicBool>, JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock apple fm bridge");
        listener
            .set_nonblocking(true)
            .expect("set mock bridge listener nonblocking");
        let address = listener.local_addr().expect("listener addr");
        let saw_chat_completion = Arc::new(AtomicBool::new(false));
        let saw_chat_completion_handle = Arc::clone(&saw_chat_completion);
        let session_counter = Arc::new(AtomicUsize::new(0));
        let session_counter_handle = Arc::clone(&session_counter);
        let loaded_adapter_id = Arc::new(Mutex::new(String::new()));
        let loaded_adapter_id_handle = Arc::clone(&loaded_adapter_id);
        let attached_sessions = Arc::new(Mutex::new(HashMap::<String, String>::new()));
        let attached_sessions_handle = Arc::clone(&attached_sessions);

        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(error) => panic!("accept mock apple fm request: {error}"),
                };
                stream
                    .set_nonblocking(false)
                    .expect("set mock apple fm stream blocking");
                let (method, path, body) = read_http_request(&mut stream);
                match (method.as_str(), path.as_str()) {
                    ("GET", "/health") => write_http_response(
                        &mut stream,
                        200,
                        serde_json::json!({
                            "status": "ok",
                            "model_available": true,
                            "availability_message": null,
                            "default_use_case": "general",
                            "default_guardrails": "default",
                            "supported_use_cases": ["general", "content_tagging"],
                            "supported_guardrails": ["default", "permissive_content_transformations"],
                            "adapter_inventory_supported": true,
                            "adapter_attach_supported": true,
                            "loaded_adapters": adapter_inventory_json(
                                loaded_adapter_id_handle.lock().expect("adapter id lock").as_str(),
                                &attached_sessions_handle.lock().expect("attached sessions lock"),
                            ),
                        })
                        .to_string()
                        .as_str(),
                    ),
                    ("GET", "/v1/adapters") => write_http_response(
                        &mut stream,
                        200,
                        serde_json::json!({
                            "adapters": adapter_inventory_json(
                                loaded_adapter_id_handle.lock().expect("adapter id lock").as_str(),
                                &attached_sessions_handle.lock().expect("attached sessions lock"),
                            ),
                            "attach_supported": true,
                        })
                        .to_string()
                        .as_str(),
                    ),
                    ("POST", "/v1/adapters/load") => {
                        let request_json: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("adapter load body");
                        let next_adapter_id = request_json["requested_adapter_id"]
                            .as_str()
                            .unwrap_or("fixture-chat-adapter")
                            .to_string();
                        *loaded_adapter_id_handle.lock().expect("adapter id lock") =
                            next_adapter_id.clone();
                        write_http_response(
                            &mut stream,
                            200,
                            serde_json::json!({
                                "adapter": adapter_entry_json(
                                    next_adapter_id.as_str(),
                                    &attached_sessions_handle
                                        .lock()
                                        .expect("attached sessions lock"),
                                ),
                            })
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("DELETE", path) if path.starts_with("/v1/adapters/") => {
                        let adapter_id = path.trim_start_matches("/v1/adapters/");
                        let mut loaded_adapter_id =
                            loaded_adapter_id_handle.lock().expect("adapter id lock");
                        if loaded_adapter_id.as_str() == adapter_id {
                            loaded_adapter_id.clear();
                            attached_sessions_handle
                                .lock()
                                .expect("attached sessions lock")
                                .clear();
                        }
                        write_http_response(&mut stream, 200, "{}");
                    }
                    ("GET", "/v1/models") => write_http_response(
                        &mut stream,
                        200,
                        serde_json::json!({
                            "data": [{
                                "id": "apple-foundation-model",
                                "default_use_case": "general",
                                "default_guardrails": "default",
                                "supported_use_cases": ["general", "content_tagging"],
                                "supported_guardrails": ["default", "permissive_content_transformations"],
                                "available": true
                            }],
                        })
                        .to_string()
                        .as_str(),
                    ),
                    ("POST", "/v1/chat/completions") => {
                        saw_chat_completion_handle.store(true, Ordering::SeqCst);
                        write_http_response(
                            &mut stream,
                            200,
                            serde_json::json!({
                                "model": "apple-foundation-model",
                                "choices": [{
                                    "message": {
                                        "role": "assistant",
                                        "content": "hello from apple fm"
                                    }
                                }],
                                "usage": {
                                    "prompt_tokens": 9,
                                    "completion_tokens": 4,
                                    "total_tokens": 13
                                }
                            })
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("POST", "/v1/sessions") => {
                        let request_json: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("session create body");
                        let session_id = format!(
                            "sess-{}",
                            session_counter_handle.fetch_add(1, Ordering::SeqCst) + 1
                        );
                        let transcript_json = request_json["transcript_json"]
                            .as_str()
                            .map(ToString::to_string)
                            .unwrap_or_else(empty_transcript_json);
                        write_http_response(
                            &mut stream,
                            200,
                            session_create_response(
                                session_id.as_str(),
                                request_json["instructions"].as_str(),
                                tool_metadata_json(&request_json["tools"]),
                                request_json.get("adapter"),
                                transcript_json.as_str(),
                            )
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("GET", path)
                        if path.starts_with("/v1/sessions/") && path.ends_with("/transcript") =>
                    {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/transcript")
                            .trim_end_matches('/');
                        let transcript_json = if session_id == "sess-1" {
                            empty_transcript_json()
                        } else {
                            non_empty_transcript_json()
                        };
                        write_http_response(&mut stream, 200, transcript_json.as_str());
                    }
                    ("GET", path) if path.starts_with("/v1/sessions/") => {
                        let session_id = path.trim_start_matches("/v1/sessions/");
                        let transcript_json = if session_id == "sess-1" {
                            empty_transcript_json()
                        } else {
                            non_empty_transcript_json()
                        };
                        let session_adapter = attached_sessions_handle
                            .lock()
                            .expect("attached sessions lock")
                            .get(session_id)
                            .cloned();
                        write_http_response(
                            &mut stream,
                            200,
                            session_state_json(
                                session_id,
                                Some("You are a helper"),
                                &[serde_json::json!({
                                    "name": "lookup_secret_code",
                                    "description": "Return a deterministic secret code for a subject."
                                })],
                                session_adapter.as_deref(),
                                transcript_json.as_str(),
                            )
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("POST", path)
                        if path.starts_with("/v1/sessions/")
                            && path.ends_with("/responses/structured") =>
                    {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/responses/structured")
                            .trim_end_matches('/');
                        let request_json: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("structured body");
                        assert_eq!(request_json["prompt"], "summarize this task");
                        write_http_response(
                            &mut stream,
                            200,
                            serde_json::json!({
                                "session": session_state_json(
                                    session_id,
                                    Some("You are a helper"),
                                    &[],
                                    attached_sessions_handle
                                        .lock()
                                        .expect("attached sessions lock")
                                        .get(session_id)
                                        .map(String::as_str),
                                    non_empty_transcript_json().as_str(),
                                ),
                                "model": "apple-foundation-model",
                                "content": {
                                    "generation_id": "gen-1",
                                    "content": {
                                        "summary": "Ship Apple FM",
                                        "confidence": 0.94
                                    },
                                    "is_complete": true
                                },
                                "usage": {
                                    "prompt_tokens": 7,
                                    "completion_tokens": 6,
                                    "total_tokens": 13
                                }
                            })
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("POST", path)
                        if path.starts_with("/v1/sessions/")
                            && path.ends_with("/responses/stream") =>
                    {
                        let response = "event: snapshot\n\
data: {\"kind\":\"snapshot\",\"model\":\"apple-foundation-model\",\"output\":\"hello\"}\n\n\
event: completed\n\
data: {\"kind\":\"completed\",\"model\":\"apple-foundation-model\",\"output\":\"hello world\",\"usage\":{\"total_tokens\":11},\"session\":{\"id\":\"sess-1\",\"instructions\":\"You are a helper\",\"model\":{\"id\":\"apple-foundation-model\",\"use_case\":\"general\",\"guardrails\":\"default\"},\"tools\":[],\"is_responding\":false,\"transcript_json\":\"{\\\"version\\\":1,\\\"type\\\":\\\"FoundationModels.Transcript\\\",\\\"transcript\\\":{\\\"entries\\\":[{\\\"role\\\":\\\"assistant\\\",\\\"content\\\":\\\"hello world\\\"}]}}\"}}\n\n";
                        write_http_response_with_content_type(
                            &mut stream,
                            200,
                            "text/event-stream",
                            response,
                        );
                    }
                    ("POST", path)
                        if path.starts_with("/v1/sessions/")
                            && path.ends_with("/responses") =>
                    {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/responses")
                            .trim_end_matches('/');
                        let request_json: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("session response body");
                        assert_eq!(request_json["prompt"], "hello");
                        assert_eq!(request_json["options"]["temperature"], 0.4);
                        assert_eq!(request_json["options"]["sampling"]["mode"], "random");
                        assert_eq!(request_json["options"]["sampling"]["top_k"], 32);
                        assert_eq!(request_json["options"]["sampling"]["seed"], 7);
                        write_http_response(
                            &mut stream,
                            200,
                            serde_json::json!({
                                "session": session_state_json(
                                    session_id,
                                    Some("You are a helper"),
                                    &[],
                                    request_json["adapter"]
                                        .get("adapter_id")
                                        .and_then(serde_json::Value::as_str)
                                        .map(ToString::to_string)
                                        .or_else(|| {
                                            attached_sessions_handle
                                                .lock()
                                                .expect("attached sessions lock")
                                                .get(session_id)
                                                .cloned()
                                        })
                                        .as_deref(),
                                    non_empty_transcript_json().as_str(),
                                ),
                                "model": "apple-foundation-model",
                                "output": "session hello from apple fm",
                                "usage": {
                                    "prompt_tokens": 6,
                                    "completion_tokens": 5,
                                    "total_tokens": 11
                                }
                            })
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("POST", path) if path.starts_with("/v1/sessions/") && path.ends_with("/reset") => {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/reset")
                            .trim_end_matches('/');
                        write_http_response(
                            &mut stream,
                            200,
                            session_state_json(
                                session_id,
                                Some("You are a helper"),
                                &[],
                                None,
                                empty_transcript_json().as_str(),
                            )
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("POST", path) if path.starts_with("/v1/sessions/") && path.ends_with("/adapter") => {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/adapter")
                            .trim_end_matches('/');
                        let request_json: serde_json::Value =
                            serde_json::from_str(body.as_str()).expect("attach adapter body");
                        let adapter_id = request_json["adapter"]["adapter_id"]
                            .as_str()
                            .expect("adapter id");
                        attached_sessions_handle
                            .lock()
                            .expect("attached sessions lock")
                            .insert(session_id.to_string(), adapter_id.to_string());
                        write_http_response(
                            &mut stream,
                            200,
                            session_state_json(
                                session_id,
                                Some("You are a helper"),
                                &[],
                                Some(adapter_id),
                                non_empty_transcript_json().as_str(),
                            )
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("DELETE", path) if path.starts_with("/v1/sessions/") && path.ends_with("/adapter") => {
                        let session_id = path
                            .trim_start_matches("/v1/sessions/")
                            .trim_end_matches("/adapter")
                            .trim_end_matches('/');
                        attached_sessions_handle
                            .lock()
                            .expect("attached sessions lock")
                            .remove(session_id);
                        write_http_response(
                            &mut stream,
                            200,
                            session_state_json(
                                session_id,
                                Some("You are a helper"),
                                &[],
                                None,
                                non_empty_transcript_json().as_str(),
                            )
                            .to_string()
                            .as_str(),
                        );
                    }
                    ("DELETE", path) if path.starts_with("/v1/sessions/") => {
                        write_http_response(&mut stream, 200, "{}");
                    }
                    other => panic!("unexpected mock apple fm request {other:?}"),
                }
            }
        });

        (format!("http://{}", address), saw_chat_completion, handle)
    }

    fn read_http_request(stream: &mut TcpStream) -> (String, String, String) {
        let mut buffer = Vec::<u8>::new();
        let mut chunk = [0u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut chunk).expect("read request bytes");
            assert!(read > 0, "request stream closed before headers completed");
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(position) = buffer.windows(4).position(|window| window == b"\r\n\r\n") {
                break position;
            }
        };

        let headers = String::from_utf8(buffer[..header_end].to_vec()).expect("request headers");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content-length"))
            })
            .unwrap_or(0);
        let mut body = buffer[(header_end + 4)..].to_vec();
        while body.len() < content_length {
            let read = stream.read(&mut chunk).expect("read request body");
            assert!(read > 0, "request stream closed before body completed");
            body.extend_from_slice(&chunk[..read]);
        }

        let request_line = headers.lines().next().expect("request line");
        let mut parts = request_line.split_whitespace();
        let method = parts.next().expect("request method").to_string();
        let path = parts.next().expect("request path").to_string();
        let body = String::from_utf8(body[..content_length].to_vec()).expect("request body utf8");
        (method, path, body)
    }

    fn write_http_response(stream: &mut TcpStream, status_code: u16, body: &str) {
        write_http_response_with_content_type(stream, status_code, "application/json", body);
    }

    fn write_http_response_with_content_type(
        stream: &mut TcpStream,
        status_code: u16,
        content_type: &str,
        body: &str,
    ) {
        let response = format!(
            "HTTP/1.1 {status_code} OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write mock response");
        stream.flush().expect("flush mock response");
    }

    fn session_create_response(
        session_id: &str,
        instructions: Option<&str>,
        tools: Vec<serde_json::Value>,
        adapter: Option<&serde_json::Value>,
        transcript_json: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "session": session_state_json(
                session_id,
                instructions,
                tools.as_slice(),
                adapter
                    .and_then(|value| value.get("adapter_id"))
                    .and_then(serde_json::Value::as_str),
                transcript_json,
            )
        })
    }

    fn session_state_json(
        session_id: &str,
        instructions: Option<&str>,
        tools: &[serde_json::Value],
        adapter_id: Option<&str>,
        transcript_json: &str,
    ) -> serde_json::Value {
        serde_json::json!({
            "id": session_id,
            "instructions": instructions,
            "model": {
                "id": "apple-foundation-model",
                "use_case": "general",
                "guardrails": "default"
            },
            "tools": tools,
            "adapter": adapter_id.map(adapter_selection_json),
            "is_responding": false,
            "transcript_json": transcript_json
        })
    }

    fn adapter_selection_json(adapter_id: &str) -> serde_json::Value {
        serde_json::json!({
            "adapter_id": adapter_id,
            "package_digest": format!("sha256:{adapter_id}"),
        })
    }

    fn adapter_entry_json(
        adapter_id: &str,
        attached_sessions: &HashMap<String, String>,
    ) -> serde_json::Value {
        let attached_session_ids = attached_sessions
            .iter()
            .filter_map(|(session_id, attached_adapter_id)| {
                (attached_adapter_id == adapter_id).then(|| session_id.clone())
            })
            .collect::<Vec<_>>();
        serde_json::json!({
            "adapter": adapter_selection_json(adapter_id),
            "base_model_signature": "apple-foundation-model/general/default",
            "package_format_version": "fmadapter.v1",
            "draft_model_present": false,
            "compatibility": {
                "compatible": true,
                "reason_code": null,
                "message": "mock bridge compatible"
            },
            "attached_session_ids": attached_session_ids,
        })
    }

    fn adapter_inventory_json(
        loaded_adapter_id: &str,
        attached_sessions: &HashMap<String, String>,
    ) -> Vec<serde_json::Value> {
        if loaded_adapter_id.trim().is_empty() {
            Vec::new()
        } else {
            vec![adapter_entry_json(loaded_adapter_id, attached_sessions)]
        }
    }

    fn tool_metadata_json(value: &serde_json::Value) -> Vec<serde_json::Value> {
        value
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool["name"],
                    "description": tool["description"],
                })
            })
            .collect()
    }

    fn empty_transcript_json() -> String {
        "{\"version\":1,\"type\":\"FoundationModels.Transcript\",\"transcript\":{\"entries\":[]}}"
            .to_string()
    }

    fn non_empty_transcript_json() -> String {
        "{\"version\":1,\"type\":\"FoundationModels.Transcript\",\"transcript\":{\"entries\":[{\"role\":\"assistant\",\"content\":\"hello world\"}]}}"
            .to_string()
    }

    fn wait_for_workbench_completion(
        worker: &mut AppleFmBridgeWorker,
        request_id: &str,
    ) -> (super::AppleFmWorkbenchCompleted, Vec<String>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Workbench(update) = update {
                    match *update {
                        AppleFmWorkbenchUpdate::Started(_) => {}
                        AppleFmWorkbenchUpdate::Event(event) if event.request_id == request_id => {
                            events.push(event.line);
                        }
                        AppleFmWorkbenchUpdate::Completed(completed)
                            if completed.request_id == request_id =>
                        {
                            return (completed, events);
                        }
                        AppleFmWorkbenchUpdate::Failed(failed)
                            if failed.request_id == request_id =>
                        {
                            panic!(
                                "expected workbench completion, got failure: {}",
                                failed.error
                            );
                        }
                        AppleFmWorkbenchUpdate::Event(_)
                        | AppleFmWorkbenchUpdate::Completed(_)
                        | AppleFmWorkbenchUpdate::Failed(_) => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        panic!("timed out waiting for workbench completion {request_id}");
    }

    fn wait_for_mission_control_summary_completion(
        worker: &mut AppleFmBridgeWorker,
        request_id: &str,
    ) -> (super::AppleFmMissionControlSummaryCompleted, Vec<String>) {
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut deltas = Vec::new();
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::MissionControlSummary(update) = update {
                    match *update {
                        AppleFmMissionControlSummaryUpdate::Started(_) => {}
                        AppleFmMissionControlSummaryUpdate::Delta(delta)
                            if delta.request_id == request_id =>
                        {
                            deltas.push(delta.delta);
                        }
                        AppleFmMissionControlSummaryUpdate::Completed(completed)
                            if completed.request_id == request_id =>
                        {
                            return (completed, deltas);
                        }
                        AppleFmMissionControlSummaryUpdate::Failed(failed)
                            if failed.request_id == request_id =>
                        {
                            panic!(
                                "expected mission control summary completion, got failure: {}",
                                failed.error
                            );
                        }
                        AppleFmMissionControlSummaryUpdate::Delta(_)
                        | AppleFmMissionControlSummaryUpdate::Completed(_)
                        | AppleFmMissionControlSummaryUpdate::Failed(_) => {}
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        panic!("timed out waiting for mission control summary completion {request_id}");
    }

    #[test]
    fn worker_refresh_and_generate_succeed_against_healthy_bridge() {
        let (base_url, saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url: base_url.clone(),
            auto_start: false,
        });

        worker
            .enqueue(AppleFmBridgeCommand::Refresh)
            .expect("queue apple fm refresh");
        worker
            .enqueue(AppleFmBridgeCommand::Generate(super::AppleFmGenerateJob {
                request_id: "req-apple-001".to_string(),
                prompt: "Write a short poem".to_string(),
                requested_model: Some("apple-foundation-model".to_string()),
            }))
            .expect("queue apple fm generation");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_ready_snapshot = false;
        let mut completed = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    AppleFmBridgeUpdate::Snapshot(snapshot) => {
                        if snapshot.reachable
                            && snapshot.model_available
                            && snapshot.ready_model.as_deref() == Some("apple-foundation-model")
                            && snapshot.system_model.use_case
                                == AppleFmSystemLanguageModelUseCase::General
                            && snapshot.system_model.guardrails
                                == AppleFmSystemLanguageModelGuardrails::Default
                        {
                            saw_ready_snapshot = true;
                        }
                    }
                    AppleFmBridgeUpdate::Completed(value) => {
                        completed = Some(value);
                        break;
                    }
                    AppleFmBridgeUpdate::Started(_)
                    | AppleFmBridgeUpdate::Failed(_)
                    | AppleFmBridgeUpdate::Workbench(_)
                    | AppleFmBridgeUpdate::MissionControlSummary(_) => {}
                }
            }
            if completed.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        let completed = completed.expect("healthy bridge should complete generation");
        assert!(
            saw_ready_snapshot,
            "expected ready snapshot before completion"
        );
        assert_eq!(completed.output, "hello from apple fm");
        assert_eq!(completed.provenance.backend, "apple_foundation_models");
        assert_eq!(completed.provenance.base_url, base_url);
        assert!(saw_chat_completion.load(Ordering::SeqCst));

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn worker_refresh_preserves_ready_snapshot_while_revalidating_healthy_bridge() {
        let (base_url, _saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });

        worker
            .enqueue(AppleFmBridgeCommand::Refresh)
            .expect("queue initial apple fm refresh");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_ready_snapshot = false;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Snapshot(snapshot) = update
                    && snapshot.is_ready()
                    && snapshot.ready_model.as_deref() == Some("apple-foundation-model")
                {
                    saw_ready_snapshot = true;
                    break;
                }
            }
            if saw_ready_snapshot {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_ready_snapshot,
            "expected initial ready Apple FM snapshot"
        );

        worker
            .enqueue(AppleFmBridgeCommand::Refresh)
            .expect("queue second apple fm refresh");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_revalidated_ready_snapshot = false;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Snapshot(snapshot) = update {
                    assert!(
                        snapshot.last_error.is_some() || snapshot.is_ready(),
                        "healthy revalidation should not publish a fake unavailable snapshot: {:?}",
                        snapshot
                    );
                    if snapshot.is_ready()
                        && snapshot.ready_model.as_deref() == Some("apple-foundation-model")
                    {
                        saw_revalidated_ready_snapshot = true;
                        break;
                    }
                }
            }
            if saw_revalidated_ready_snapshot {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_revalidated_ready_snapshot,
            "expected ready Apple FM snapshot after second refresh"
        );

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn ensure_bridge_running_reuses_existing_healthy_bridge() {
        let (base_url, _saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });

        worker
            .enqueue(AppleFmBridgeCommand::EnsureBridgeRunning)
            .expect("queue ensure-running command");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_ready_snapshot = false;
        let mut saw_local_spawn_attempt = false;
        let mut saw_pending_inventory_snapshot = false;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Snapshot(snapshot) = update {
                    if snapshot
                        .last_action
                        .as_deref()
                        .is_some_and(|action| action.contains("starting process"))
                    {
                        saw_local_spawn_attempt = true;
                    }
                    if snapshot.reachable
                        && snapshot.model_available
                        && snapshot.ready_model.is_none()
                    {
                        saw_pending_inventory_snapshot = true;
                    }
                    if snapshot.is_ready()
                        || snapshot.last_action.as_deref()
                            == Some("Apple FM bridge already responding on configured URL.")
                    {
                        saw_ready_snapshot = true;
                        break;
                    }
                }
            }
            if saw_ready_snapshot {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_ready_snapshot,
            "expected ensure-running to detect existing healthy bridge"
        );
        assert!(
            !saw_local_spawn_attempt,
            "ensure-running should not spawn a second bridge when one is already healthy"
        );
        assert!(
            !saw_pending_inventory_snapshot,
            "ensure-running should not publish a fake waiting-for-model-inventory snapshot when health is already ready"
        );

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn mission_control_summary_success_repairs_snapshot_readiness() {
        let (base_url, _saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });

        let request_id = "mission-control-summary-ready-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::MissionControlSummary(
                AppleFmMissionControlSummaryCommand {
                    request_id: request_id.clone(),
                    instructions: "Summarize the control state".to_string(),
                    prompt: "Latest logs go here".to_string(),
                    requested_model: Some("apple-foundation-model".to_string()),
                    options: Some(
                        AppleFmGenerationOptions::new(None, Some(0.2), Some(96))
                            .expect("summary options"),
                    ),
                },
            ))
            .expect("queue mission control summary command");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut saw_summary_complete = false;
        let mut saw_ready_snapshot = false;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                match update {
                    AppleFmBridgeUpdate::Snapshot(snapshot) => {
                        if snapshot.is_ready()
                            && snapshot.ready_model.as_deref() == Some("apple-foundation-model")
                        {
                            saw_ready_snapshot = true;
                        }
                    }
                    AppleFmBridgeUpdate::MissionControlSummary(update) => match *update {
                        AppleFmMissionControlSummaryUpdate::Completed(completed)
                            if completed.request_id == request_id =>
                        {
                            saw_summary_complete = true;
                        }
                        AppleFmMissionControlSummaryUpdate::Started(_)
                        | AppleFmMissionControlSummaryUpdate::Delta(_)
                        | AppleFmMissionControlSummaryUpdate::Completed(_)
                        | AppleFmMissionControlSummaryUpdate::Failed(_) => {}
                    },
                    AppleFmBridgeUpdate::Started(_)
                    | AppleFmBridgeUpdate::Completed(_)
                    | AppleFmBridgeUpdate::Failed(_)
                    | AppleFmBridgeUpdate::Workbench(_) => {}
                }
            }
            if saw_summary_complete && saw_ready_snapshot {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(
            saw_summary_complete,
            "expected mission control summary completion"
        );
        assert!(
            saw_ready_snapshot,
            "successful mission control summary should repair Apple FM readiness snapshot"
        );

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn worker_mission_control_summary_streams_ephemeral_session_output() {
        let (base_url, _saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });

        let request_id = "mission-control-summary-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::MissionControlSummary(
                AppleFmMissionControlSummaryCommand {
                    request_id: request_id.clone(),
                    instructions: "Summarize the control state".to_string(),
                    prompt: "Latest logs go here".to_string(),
                    requested_model: Some("apple-foundation-model".to_string()),
                    options: Some(
                        AppleFmGenerationOptions::new(None, Some(0.2), Some(96))
                            .expect("summary options"),
                    ),
                },
            ))
            .expect("queue mission control summary command");

        let (completed, deltas) =
            wait_for_mission_control_summary_completion(&mut worker, request_id.as_str());
        assert_eq!(completed.response_text, "hello world");
        assert!(!deltas.is_empty());
        assert_eq!(deltas.concat(), "hello world");

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn worker_workbench_covers_text_chat_session_stream_and_transcript_flows() {
        let (base_url, saw_chat_completion, server_handle) = spawn_mock_bridge();
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });

        let text_request_id = "wb-text-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: text_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RunText,
                instructions: None,
                prompt: Some("Say hello".to_string()),
                requested_model: Some("apple-foundation-model".to_string()),
                session_id: None,
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue text workbench command");
        let (text_completed, _) =
            wait_for_workbench_completion(&mut worker, text_request_id.as_str());
        assert_eq!(text_completed.response_text, "hello from apple fm");

        let chat_request_id = "wb-chat-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: chat_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RunChat,
                instructions: Some("Be brief".to_string()),
                prompt: Some("Say hello".to_string()),
                requested_model: Some("apple-foundation-model".to_string()),
                session_id: None,
                adapter_id: None,
                adapter_package_path: None,
                options: Some(
                    super::AppleFmGenerationOptions::new(
                        Some(AppleFmSamplingMode::greedy()),
                        Some(0.3),
                        Some(128),
                    )
                    .expect("chat options"),
                ),
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue chat workbench command");
        let (chat_completed, _) =
            wait_for_workbench_completion(&mut worker, chat_request_id.as_str());
        assert_eq!(chat_completed.response_text, "hello from apple fm");
        assert!(saw_chat_completion.load(Ordering::SeqCst));

        let create_request_id = "wb-create-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: create_request_id.clone(),
                operation: AppleFmWorkbenchOperation::CreateSession,
                instructions: Some("You are a helper".to_string()),
                prompt: None,
                requested_model: Some("apple-foundation-model".to_string()),
                session_id: None,
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::Demo,
            }))
            .expect("queue create-session workbench command");
        let (created, _) = wait_for_workbench_completion(&mut worker, create_request_id.as_str());
        let session_id = created.session_id.clone().expect("session id");
        let session_json = created.session_json.expect("session json");
        assert!(session_json.contains("lookup_secret_code"));
        assert!(session_json.contains("lookup_user_profile"));

        let inspect_request_id = "wb-inspect-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: inspect_request_id.clone(),
                operation: AppleFmWorkbenchOperation::InspectSession,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue inspect-session workbench command");
        let (inspected, _) =
            wait_for_workbench_completion(&mut worker, inspect_request_id.as_str());
        assert_eq!(inspected.session_id.as_deref(), Some(session_id.as_str()));

        let load_request_id = "wb-load-adapter-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: load_request_id.clone(),
                operation: AppleFmWorkbenchOperation::LoadAdapter,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: None,
                adapter_id: Some("fixture-chat-adapter".to_string()),
                adapter_package_path: Some("/tmp/mock-fixture-chat-adapter.fmadapter".to_string()),
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue load-adapter workbench command");
        let (loaded_adapter, _) =
            wait_for_workbench_completion(&mut worker, load_request_id.as_str());
        assert!(
            loaded_adapter
                .adapter_json
                .as_deref()
                .is_some_and(|json| json.contains("fixture-chat-adapter"))
        );

        let attach_request_id = "wb-attach-adapter-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: attach_request_id.clone(),
                operation: AppleFmWorkbenchOperation::AttachSessionAdapter,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: Some("fixture-chat-adapter".to_string()),
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue attach-adapter workbench command");
        let (attached_adapter, _) =
            wait_for_workbench_completion(&mut worker, attach_request_id.as_str());
        assert_eq!(
            attached_adapter
                .session_adapter
                .as_ref()
                .map(|adapter| adapter.adapter_id.as_str()),
            Some("fixture-chat-adapter")
        );

        let session_request_id = "wb-session-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: session_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RunSession,
                instructions: None,
                prompt: Some("hello".to_string()),
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: Some(
                    super::AppleFmGenerationOptions::new(
                        Some(
                            AppleFmSamplingMode::random(Some(32), None, Some(7))
                                .expect("random options"),
                        ),
                        Some(0.4),
                        Some(64),
                    )
                    .expect("session options"),
                ),
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue run-session workbench command");
        let (session_completed, _) =
            wait_for_workbench_completion(&mut worker, session_request_id.as_str());
        assert_eq!(
            session_completed.response_text,
            "session hello from apple fm"
        );

        let stream_request_id = "wb-stream-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: stream_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RunStream,
                instructions: None,
                prompt: Some("stream me".to_string()),
                requested_model: None,
                session_id: Some("sess-1".to_string()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue stream workbench command");
        let (stream_completed, stream_events) =
            wait_for_workbench_completion(&mut worker, stream_request_id.as_str());
        assert_eq!(stream_completed.response_text, "hello world");
        assert!(stream_events.iter().any(|line| line.contains("snapshot")));
        assert!(stream_events.iter().any(|line| line.contains("completed")));

        let structured_request_id = "wb-structured-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: structured_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RunStructured,
                instructions: None,
                prompt: Some("summarize this task".to_string()),
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: Some(
                    "{\n  \"type\": \"object\",\n  \"properties\": {\n    \"summary\": { \"type\": \"string\" },\n    \"confidence\": { \"type\": \"number\" }\n  },\n  \"required\": [\"summary\", \"confidence\"]\n}".to_string(),
                ),
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue structured workbench command");
        let (structured_completed, _) =
            wait_for_workbench_completion(&mut worker, structured_request_id.as_str());
        assert!(
            structured_completed
                .structured_json
                .as_deref()
                .is_some_and(|json| json.contains("Ship Apple FM"))
        );

        let export_request_id = "wb-export-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: export_request_id.clone(),
                operation: AppleFmWorkbenchOperation::ExportTranscript,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue export workbench command");
        let (exported, _) = wait_for_workbench_completion(&mut worker, export_request_id.as_str());
        let transcript_json = exported.transcript_json.expect("transcript json");
        assert!(transcript_json.contains("FoundationModels.Transcript"));

        let restore_request_id = "wb-restore-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: restore_request_id.clone(),
                operation: AppleFmWorkbenchOperation::RestoreTranscript,
                instructions: None,
                prompt: None,
                requested_model: Some("apple-foundation-model".to_string()),
                session_id: None,
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: Some(transcript_json),
                tool_mode: AppleFmWorkbenchToolMode::Demo,
            }))
            .expect("queue restore workbench command");
        let (restored, _) = wait_for_workbench_completion(&mut worker, restore_request_id.as_str());
        assert!(restored.session_id.is_some());
        assert!(
            restored
                .session_json
                .as_deref()
                .is_some_and(|json| json.contains("lookup_secret_code"))
        );

        let reset_request_id = "wb-reset-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: reset_request_id.clone(),
                operation: AppleFmWorkbenchOperation::ResetSession,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue reset workbench command");
        let (reset, _) = wait_for_workbench_completion(&mut worker, reset_request_id.as_str());
        assert_eq!(reset.session_id.as_deref(), Some(session_id.as_str()));

        let detach_request_id = "wb-detach-adapter-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: detach_request_id.clone(),
                operation: AppleFmWorkbenchOperation::DetachSessionAdapter,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id.clone()),
                adapter_id: None,
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue detach-adapter workbench command");
        let (detached_adapter, _) =
            wait_for_workbench_completion(&mut worker, detach_request_id.as_str());
        assert!(detached_adapter.session_adapter.is_none());

        let delete_request_id = "wb-delete-1".to_string();
        worker
            .enqueue(AppleFmBridgeCommand::Workbench(AppleFmWorkbenchCommand {
                request_id: delete_request_id.clone(),
                operation: AppleFmWorkbenchOperation::DeleteSession,
                instructions: None,
                prompt: None,
                requested_model: None,
                session_id: Some(session_id),
                adapter_id: Some("fixture-chat-adapter".to_string()),
                adapter_package_path: None,
                options: None,
                schema_json: None,
                transcript_json: None,
                tool_mode: AppleFmWorkbenchToolMode::None,
            }))
            .expect("queue delete workbench command");
        let (deleted, _) = wait_for_workbench_completion(&mut worker, delete_request_id.as_str());
        assert_eq!(deleted.summary, "deleted session sess-1");

        server_handle.join().expect("mock bridge thread");
    }

    #[test]
    fn worker_refresh_reports_unavailable_bridge() {
        let unused_listener = TcpListener::bind("127.0.0.1:0").expect("bind temp port");
        let base_url = format!(
            "http://{}",
            unused_listener.local_addr().expect("temp addr")
        );
        drop(unused_listener);

        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url,
            auto_start: false,
        });
        worker
            .enqueue(AppleFmBridgeCommand::Refresh)
            .expect("queue apple fm refresh");

        let deadline = Instant::now() + Duration::from_secs(3);
        let mut last_snapshot_error = None;
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Snapshot(snapshot) = update {
                    last_snapshot_error = snapshot.last_error.clone();
                    if !snapshot.reachable && snapshot.last_error.is_some() {
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        panic!("expected unavailable bridge snapshot, last_error={last_snapshot_error:?}");
    }

    #[test]
    fn worker_refresh_reports_misconfigured_base_url() {
        let mut worker = AppleFmBridgeWorker::spawn_with_config(AppleFmBridgeConfig {
            base_url: "://bad-url".to_string(),
            auto_start: false,
        });
        worker
            .enqueue(AppleFmBridgeCommand::Refresh)
            .expect("queue apple fm refresh");

        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            for update in worker.drain_updates() {
                if let AppleFmBridgeUpdate::Snapshot(snapshot) = update
                    && snapshot
                        .last_error
                        .as_deref()
                        .is_some_and(|error| error.contains("invalid Apple FM base URL"))
                {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        panic!("expected misconfigured base URL error snapshot");
    }
}
