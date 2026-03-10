use crate::local_inference_runtime::{
    LocalInferenceExecutionMetrics, LocalInferenceExecutionProvenance,
};
use openagents_kernel_core::ids::sha256_prefixed_text;
use psionic_apple_fm::{AppleFmBridgeClient, DEFAULT_APPLE_FM_MODEL_ID};
use reqwest::Url;
use reqwest::blocking::Client as HttpClient;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const LANE_POLL: Duration = Duration::from_millis(120);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const BRIDGE_HEALTH_POLL: Duration = Duration::from_millis(100);
const BRIDGE_STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_APPLE_FM_BASE_URL: &str = "http://127.0.0.1:11435";
const ENV_APPLE_FM_BASE_URL: &str = "OPENAGENTS_APPLE_FM_BASE_URL";
const ENV_APPLE_FM_BRIDGE_BIN: &str = "OPENAGENTS_APPLE_FM_BRIDGE_BIN";

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
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
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

#[derive(Clone, Debug)]
pub enum AppleFmBridgeCommand {
    Refresh,
    EnsureBridgeRunning,
    StopBridge,
    Generate(AppleFmGenerateJob),
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

impl AppleFmLocalBridge {
    fn ensure_running(
        &mut self,
        config: &AppleFmBridgeConfig,
        client: &AppleFmBridgeClient,
        snapshot: &mut AppleFmBridgeSnapshot,
    ) -> Result<(), String> {
        if !cfg!(target_os = "macos") {
            self.status = AppleFmBridgeStatus::UnsupportedPlatform;
            snapshot.bridge_status = Some(self.status.label().to_string());
            snapshot.last_error =
                Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());
            snapshot.last_action = Some("Apple FM bridge unavailable on this platform".to_string());
            return Err(snapshot.last_error.clone().unwrap_or_default());
        }

        if let Some(child) = self.child.as_mut() {
            if let Ok(Some(status)) = child.try_wait() {
                self.child = None;
                self.status = AppleFmBridgeStatus::Failed;
                snapshot.last_error =
                    Some(format!("Apple FM bridge exited unexpectedly: {status}"));
            }
        }
        if self.child.is_none() {
            let binary = find_bridge_binary().ok_or_else(|| {
                "Apple FM bridge binary not found. Build swift/foundation-bridge first.".to_string()
            })?;
            let port = port_from_base_url(config.base_url.as_str()).unwrap_or(11435);
            let child = Command::new(binary)
                .arg(port.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| format!("failed to start Apple FM bridge: {error}"))?;
            self.child = Some(child);
            self.status = AppleFmBridgeStatus::Starting;
            snapshot.bridge_status = Some(self.status.label().to_string());
            snapshot.last_action = Some("Starting Apple FM bridge".to_string());
        }
        wait_for_bridge_health(client)?;
        self.status = AppleFmBridgeStatus::Running;
        snapshot.bridge_status = Some(self.status.label().to_string());
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
        self.snapshot.available_models.clear();
        self.snapshot.ready_model = None;
        self.snapshot.last_metrics = None;
        self.snapshot.reachable = false;
        self.snapshot.model_available = false;
        self.snapshot.refreshed_at = Some(Instant::now());

        if !cfg!(target_os = "macos") {
            self.snapshot.bridge_status =
                Some(AppleFmBridgeStatus::UnsupportedPlatform.label().to_string());
            self.snapshot.last_action = Some("Apple FM bridge unsupported".to_string());
            self.snapshot.last_error =
                Some("Apple Foundation Models requires macOS 26+ on Apple Silicon".to_string());
            self.publish_snapshot(update_tx);
            return;
        }

        let Some(client) = self.client.as_ref() else {
            self.snapshot.last_action = Some("Apple FM refresh failed".to_string());
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
                self.snapshot.reachable = true;
                self.snapshot.model_available = health.model_available;
                self.snapshot.availability_message = health.availability_message.clone();
                self.snapshot.bridge_status =
                    Some(AppleFmBridgeStatus::Running.label().to_string());
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some("Refreshed Apple FM bridge health".to_string());
                match client.model_ids() {
                    Ok(models) => {
                        self.snapshot.available_models = models.clone();
                        self.snapshot.ready_model = health.model_available.then(|| {
                            models
                                .first()
                                .cloned()
                                .unwrap_or_else(|| DEFAULT_APPLE_FM_MODEL_ID.to_string())
                        });
                    }
                    Err(error) => {
                        self.snapshot.last_error = Some(error.to_string());
                    }
                }
            }
            Err(error) => {
                self.snapshot.bridge_status = Some(self.bridge.status.label().to_string());
                self.snapshot.last_error = Some(error.to_string());
                self.snapshot.last_action = Some("Apple FM refresh failed".to_string());
            }
        }
        self.publish_snapshot(update_tx);
    }

    fn handle_ensure_bridge_running(&mut self, update_tx: &Sender<AppleFmBridgeUpdate>) {
        let Some(client) = self.client.as_ref() else {
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
            .ensure_running(&self.config, client, &mut self.snapshot)
        {
            Ok(()) => {
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some("Apple FM bridge running".to_string());
            }
            Err(error) => {
                self.snapshot.bridge_status = Some(self.bridge.status.label().to_string());
                self.snapshot.last_error = Some(error);
                self.snapshot.last_action = Some("Apple FM bridge start failed".to_string());
            }
        }
        self.handle_refresh(update_tx);
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
                .ensure_running(&self.config, client, &mut self.snapshot);
        }

        let model = job
            .requested_model
            .clone()
            .or_else(|| self.snapshot.ready_model.clone())
            .unwrap_or_else(|| DEFAULT_APPLE_FM_MODEL_ID.to_string());

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
                let _ = update_tx.send(AppleFmBridgeUpdate::Failed(AppleFmExecutionFailed {
                    request_id: job.request_id,
                    error: error.to_string(),
                }));
            }
        }
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

fn port_from_base_url(base_url: &str) -> Option<u16> {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.port_or_known_default())
}

fn find_bridge_binary() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_APPLE_FM_BRIDGE_BIN) {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Some(candidate);
        }
    }

    let candidates = [
        PathBuf::from("bin/foundation-bridge"),
        PathBuf::from("swift/foundation-bridge/.build/release/foundation-bridge"),
        PathBuf::from(
            "swift/foundation-bridge/.build/arm64-apple-macosx/release/foundation-bridge",
        ),
    ];
    for candidate in candidates {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.join("foundation-bridge")))
        .filter(|candidate| candidate.exists())
}

#[cfg(test)]
mod tests {
    use super::{
        AppleFmBridgeCommand, AppleFmBridgeConfig, AppleFmBridgeUpdate, AppleFmBridgeWorker,
    };
    use std::io::{ErrorKind, Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
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
                let (method, path, _) = read_http_request(&mut stream);
                let response_body = match (method.as_str(), path.as_str()) {
                    ("GET", "/health") => serde_json::json!({
                        "status": "ok",
                        "model_available": true,
                        "availability_message": null,
                    })
                    .to_string(),
                    ("GET", "/v1/models") => serde_json::json!({
                        "data": [{ "id": "apple-foundation-model" }],
                    })
                    .to_string(),
                    ("POST", "/v1/chat/completions") => {
                        saw_chat_completion_handle.store(true, Ordering::SeqCst);
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
                                "completion_tokens": 4
                            }
                        })
                        .to_string()
                    }
                    other => panic!("unexpected mock apple fm request {other:?}"),
                };
                write_http_response(&mut stream, 200, response_body.as_str());
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
        let response = format!(
            "HTTP/1.1 {status_code} OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        stream
            .write_all(response.as_bytes())
            .expect("write mock response");
        stream.flush().expect("flush mock response");
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
                        {
                            saw_ready_snapshot = true;
                        }
                    }
                    AppleFmBridgeUpdate::Completed(value) => {
                        completed = Some(value);
                        break;
                    }
                    AppleFmBridgeUpdate::Started(_) | AppleFmBridgeUpdate::Failed(_) => {}
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
