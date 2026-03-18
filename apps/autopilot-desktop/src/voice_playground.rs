use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use reqwest::blocking::Client as HttpClient;
use serde::Deserialize;
use serde_json::json;

const WORKER_POLL: Duration = Duration::from_millis(120);
const GOOGLE_STT_BASE_URL: &str = "https://us-speech.googleapis.com";
const GOOGLE_STT_LOCATION: &str = "us";
const GOOGLE_STT_MODEL: &str = "chirp_3";
const GOOGLE_STT_LANGUAGE_CODE: &str = "en-US";
const GCLOUD_PROJECT_ENV_KEYS: [&str; 3] = [
    "OPENAGENTS_GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_PROJECT",
    "GCLOUD_PROJECT",
];

#[derive(Clone, Debug)]
pub struct VoicePlaygroundConfig {
    pub project_id: Option<String>,
    pub stt_base_url: String,
    pub stt_location: String,
    pub stt_model: String,
    pub stt_language_code: String,
}

impl Default for VoicePlaygroundConfig {
    fn default() -> Self {
        Self {
            project_id: env_project_id(),
            stt_base_url: std::env::var("OPENAGENTS_GOOGLE_STT_BASE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_STT_BASE_URL.to_string()),
            stt_location: std::env::var("OPENAGENTS_GOOGLE_STT_LOCATION")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_STT_LOCATION.to_string()),
            stt_model: std::env::var("OPENAGENTS_GOOGLE_STT_MODEL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_STT_MODEL.to_string()),
            stt_language_code: std::env::var("OPENAGENTS_GOOGLE_STT_LANGUAGE_CODE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_STT_LANGUAGE_CODE.to_string()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VoiceRecordingState {
    #[default]
    Idle,
    Recording,
}

impl VoiceRecordingState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VoiceTranscriptionState {
    #[default]
    Idle,
    Running,
    Ready,
    Error,
}

impl VoiceTranscriptionState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VoicePlaygroundSnapshot {
    pub backend_label: String,
    pub active_account: Option<String>,
    pub project_id: Option<String>,
    pub stt_location: String,
    pub stt_model: String,
    pub stt_language_code: String,
    pub recording_state: VoiceRecordingState,
    pub transcription_state: VoiceTranscriptionState,
    pub input_device_name: Option<String>,
    pub pending_request_id: Option<String>,
    pub last_request_id: Option<String>,
    pub last_clip_duration_ms: Option<u64>,
    pub last_transcript: Option<String>,
    pub last_transcription_latency_ms: Option<u64>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoicePlaygroundUpdate {
    Snapshot(VoicePlaygroundSnapshot),
    RecordingStarted {
        input_device_name: Option<String>,
    },
    RecordingCancelled,
    RecordingStopped {
        clip_duration_ms: u64,
    },
    RecordingFailed {
        error: String,
    },
    TranscriptionStarted {
        request_id: String,
    },
    TranscriptionCompleted {
        request_id: String,
        transcript: String,
        clip_duration_ms: u64,
        latency_ms: u64,
    },
    TranscriptionFailed {
        request_id: String,
        error: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoicePlaygroundCommand {
    Refresh,
    StartRecording,
    CancelRecording,
    StopRecordingAndTranscribe {
        request_id: String,
    },
}

enum VoicePlaygroundWorkerCommand {
    Run(VoicePlaygroundCommand),
    Shutdown,
}

pub struct VoicePlaygroundWorker {
    command_tx: Sender<VoicePlaygroundWorkerCommand>,
    update_rx: Receiver<VoicePlaygroundUpdate>,
    worker_thread: Option<JoinHandle<()>>,
}

impl VoicePlaygroundWorker {
    pub fn new() -> Self {
        let config = VoicePlaygroundConfig::default();
        let (command_tx, command_rx) = mpsc::channel::<VoicePlaygroundWorkerCommand>();
        let (update_tx, update_rx) = mpsc::channel::<VoicePlaygroundUpdate>();
        let worker_thread = std::thread::Builder::new()
            .name("voice-playground".to_string())
            .spawn(move || {
                let gcloud = Arc::new(SystemGcloudCli);
                let auth = Arc::new(GcloudAccessTokenProvider::new(gcloud.clone()));
                let environment = GcloudEnvironmentProbe::new(gcloud);
                let backend = Box::new(GoogleCloudSttBackend::new(config.clone(), auth))
                    as Box<dyn VoiceTranscriptionBackend>;
                let capture = Box::new(CpalAudioCapture::default()) as Box<dyn AudioCapture>;
                let mut controller =
                    VoicePlaygroundController::new(config, backend, capture, environment);
                let _ = update_tx.send(VoicePlaygroundUpdate::Snapshot(
                    controller.snapshot().clone(),
                ));
                loop {
                    match command_rx.recv_timeout(WORKER_POLL) {
                        Ok(VoicePlaygroundWorkerCommand::Run(command)) => {
                            for update in controller.handle_command(command) {
                                let _ = update_tx.send(update);
                            }
                        }
                        Ok(VoicePlaygroundWorkerCommand::Shutdown) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .ok();
        Self {
            command_tx,
            update_rx,
            worker_thread,
        }
    }

    #[cfg(test)]
    fn with_components(
        config: VoicePlaygroundConfig,
        backend: Box<dyn VoiceTranscriptionBackend>,
        capture: Box<dyn AudioCapture + Send>,
        environment: GcloudEnvironmentProbe,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<VoicePlaygroundWorkerCommand>();
        let (update_tx, update_rx) = mpsc::channel::<VoicePlaygroundUpdate>();
        let worker_thread = std::thread::Builder::new()
            .name("voice-playground".to_string())
            .spawn(move || {
                let mut controller =
                    VoicePlaygroundController::new(config, backend, capture, environment);
                let _ = update_tx.send(VoicePlaygroundUpdate::Snapshot(
                    controller.snapshot().clone(),
                ));
                loop {
                    match command_rx.recv_timeout(WORKER_POLL) {
                        Ok(VoicePlaygroundWorkerCommand::Run(command)) => {
                            for update in controller.handle_command(command) {
                                let _ = update_tx.send(update);
                            }
                        }
                        Ok(VoicePlaygroundWorkerCommand::Shutdown) => break,
                        Err(RecvTimeoutError::Timeout) => {}
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .ok();
        Self {
            command_tx,
            update_rx,
            worker_thread,
        }
    }

    pub fn enqueue(&self, command: VoicePlaygroundCommand) -> Result<(), String> {
        self.command_tx
            .send(VoicePlaygroundWorkerCommand::Run(command))
            .map_err(|error| format!("voice playground worker unavailable: {error}"))
    }

    pub fn drain_updates(&self) -> Vec<VoicePlaygroundUpdate> {
        self.update_rx.try_iter().collect()
    }
}

impl Drop for VoicePlaygroundWorker {
    fn drop(&mut self) {
        let _ = self.command_tx.send(VoicePlaygroundWorkerCommand::Shutdown);
        if let Some(worker_thread) = self.worker_thread.take() {
            let _ = worker_thread.join();
        }
    }
}

struct VoicePlaygroundController {
    config: VoicePlaygroundConfig,
    backend: Box<dyn VoiceTranscriptionBackend>,
    capture: Box<dyn AudioCapture>,
    environment: GcloudEnvironmentProbe,
    snapshot: VoicePlaygroundSnapshot,
}

impl VoicePlaygroundController {
    fn new(
        config: VoicePlaygroundConfig,
        backend: Box<dyn VoiceTranscriptionBackend>,
        capture: Box<dyn AudioCapture>,
        environment: GcloudEnvironmentProbe,
    ) -> Self {
        let mut snapshot = VoicePlaygroundSnapshot {
            backend_label: "google-cloud-speech-v2".to_string(),
            project_id: config.project_id.clone(),
            stt_location: config.stt_location.clone(),
            stt_model: config.stt_model.clone(),
            stt_language_code: config.stt_language_code.clone(),
            last_action: Some("Voice playground ready".to_string()),
            ..VoicePlaygroundSnapshot::default()
        };
        let (active_account, project_id) = environment.describe(config.project_id.clone());
        snapshot.active_account = active_account;
        snapshot.project_id = project_id;
        Self {
            config,
            backend,
            capture,
            environment,
            snapshot,
        }
    }

    fn snapshot(&self) -> &VoicePlaygroundSnapshot {
        &self.snapshot
    }

    fn handle_command(&mut self, command: VoicePlaygroundCommand) -> Vec<VoicePlaygroundUpdate> {
        match command {
            VoicePlaygroundCommand::Refresh => self.refresh(),
            VoicePlaygroundCommand::StartRecording => self.start_recording(),
            VoicePlaygroundCommand::CancelRecording => self.cancel_recording(),
            VoicePlaygroundCommand::StopRecordingAndTranscribe { request_id } => {
                self.stop_and_transcribe(request_id)
            }
        }
    }

    fn refresh(&mut self) -> Vec<VoicePlaygroundUpdate> {
        let (active_account, project_id) = self.environment.describe(self.config.project_id.clone());
        self.snapshot.active_account = active_account;
        self.snapshot.project_id = project_id;
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some("Refreshed Google Cloud voice environment".to_string());
        vec![VoicePlaygroundUpdate::Snapshot(self.snapshot.clone())]
    }

    fn start_recording(&mut self) -> Vec<VoicePlaygroundUpdate> {
        match self.capture.start_recording() {
            Ok(recording) => {
                self.snapshot.recording_state = VoiceRecordingState::Recording;
                self.snapshot.transcription_state = VoiceTranscriptionState::Idle;
                self.snapshot.last_error = None;
                self.snapshot.input_device_name = recording.input_device_name.clone();
                self.snapshot.last_action = Some(format!(
                    "Recording microphone clip from {}",
                    recording
                        .input_device_name
                        .as_deref()
                        .unwrap_or("default input")
                ));
                vec![VoicePlaygroundUpdate::RecordingStarted {
                    input_device_name: recording.input_device_name,
                }]
            }
            Err(error) => {
                self.snapshot.recording_state = VoiceRecordingState::Idle;
                self.snapshot.transcription_state = VoiceTranscriptionState::Error;
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action = Some("Voice recording failed to start".to_string());
                vec![VoicePlaygroundUpdate::RecordingFailed { error }]
            }
        }
    }

    fn cancel_recording(&mut self) -> Vec<VoicePlaygroundUpdate> {
        self.capture.cancel_recording();
        self.snapshot.recording_state = VoiceRecordingState::Idle;
        self.snapshot.transcription_state = VoiceTranscriptionState::Idle;
        self.snapshot.pending_request_id = None;
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some("Cancelled voice recording".to_string());
        vec![VoicePlaygroundUpdate::RecordingCancelled]
    }

    fn stop_and_transcribe(&mut self, request_id: String) -> Vec<VoicePlaygroundUpdate> {
        let clip = match self.capture.stop_recording() {
            Ok(clip) => clip,
            Err(error) => {
                self.snapshot.recording_state = VoiceRecordingState::Idle;
                self.snapshot.transcription_state = VoiceTranscriptionState::Error;
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action = Some("Voice recording failed".to_string());
                return vec![VoicePlaygroundUpdate::RecordingFailed { error }];
            }
        };

        self.snapshot.recording_state = VoiceRecordingState::Idle;
        self.snapshot.transcription_state = VoiceTranscriptionState::Running;
        self.snapshot.pending_request_id = Some(request_id.clone());
        self.snapshot.last_request_id = Some(request_id.clone());
        self.snapshot.last_clip_duration_ms = Some(clip.duration_ms);
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some(format!("Transcribing microphone clip [{}]", request_id));
        let mut updates = vec![
            VoicePlaygroundUpdate::RecordingStopped {
                clip_duration_ms: clip.duration_ms,
            },
            VoicePlaygroundUpdate::TranscriptionStarted {
                request_id: request_id.clone(),
            },
        ];

        let started_at = Instant::now();
        match self.backend.transcribe(&TranscriptionRequest {
            request_id: request_id.clone(),
            clip,
            project_id: self.snapshot.project_id.clone(),
        }) {
            Ok(transcript) => {
                let latency_ms =
                    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                self.snapshot.pending_request_id = None;
                self.snapshot.transcription_state = VoiceTranscriptionState::Ready;
                self.snapshot.last_transcript = Some(transcript.transcript.clone());
                self.snapshot.last_transcription_latency_ms = Some(latency_ms);
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some(format!(
                    "Transcribed voice clip [{}] in {} ms",
                    request_id, latency_ms
                ));
                updates.push(VoicePlaygroundUpdate::TranscriptionCompleted {
                    request_id,
                    transcript: transcript.transcript,
                    clip_duration_ms: transcript.clip_duration_ms,
                    latency_ms,
                });
            }
            Err(error) => {
                self.snapshot.pending_request_id = None;
                self.snapshot.transcription_state = VoiceTranscriptionState::Error;
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action = Some("Voice transcription failed".to_string());
                updates.push(VoicePlaygroundUpdate::TranscriptionFailed { request_id, error });
            }
        }

        updates
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptionResult {
    pub transcript: String,
    pub clip_duration_ms: u64,
}

#[derive(Clone, Debug)]
pub struct TranscriptionRequest {
    pub request_id: String,
    pub clip: AudioClip,
    pub project_id: Option<String>,
}

pub trait VoiceTranscriptionBackend: Send {
    fn transcribe(&self, request: &TranscriptionRequest) -> Result<TranscriptionResult, String>;
}

struct GoogleCloudSttBackend {
    config: VoicePlaygroundConfig,
    auth: Arc<dyn AccessTokenProvider>,
    http: HttpClient,
}

impl GoogleCloudSttBackend {
    fn new(config: VoicePlaygroundConfig, auth: Arc<dyn AccessTokenProvider>) -> Self {
        Self {
            config,
            auth,
            http: HttpClient::new(),
        }
    }

    fn recognize_url(&self, project_id: &str) -> String {
        format!(
            "{}/v2/projects/{}/locations/{}/recognizers/_:recognize",
            self.config.stt_base_url.trim_end_matches('/'),
            project_id,
            self.config.stt_location,
        )
    }

    fn request_payload(&self, clip: &AudioClip) -> serde_json::Value {
        json!({
            "config": {
                "autoDecodingConfig": {},
                "languageCodes": [self.config.stt_language_code],
                "model": self.config.stt_model,
            },
            "content": URL_SAFE_NO_PAD.encode(clip.wav_bytes.as_slice()),
        })
    }
}

impl VoiceTranscriptionBackend for GoogleCloudSttBackend {
    fn transcribe(&self, request: &TranscriptionRequest) -> Result<TranscriptionResult, String> {
        let project_id = request
            .project_id
            .clone()
            .or_else(|| self.config.project_id.clone())
            .ok_or_else(|| {
                "Google Cloud project is unset. Configure `OPENAGENTS_GOOGLE_CLOUD_PROJECT` or run `gcloud config set project ...`.".to_string()
            })?;
        let response = self
            .http
            .post(self.recognize_url(project_id.as_str()))
            .bearer_auth(self.auth.access_token()?)
            .header("x-goog-user-project", project_id.as_str())
            .json(&self.request_payload(&request.clip))
            .send()
            .map_err(|error| format!("Speech-to-Text request failed: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .unwrap_or_else(|_| "failed to read response body".to_string());
            return Err(format!("Speech-to-Text request failed ({status}): {body}"));
        }

        let payload: RecognizeResponse = response
            .json()
            .map_err(|error| format!("Speech-to-Text response decode failed: {error}"))?;
        let transcript = payload
            .results
            .iter()
            .filter_map(|result| result.alternatives.first())
            .map(|alternative| alternative.transcript.trim())
            .filter(|transcript| !transcript.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if transcript.is_empty() {
            return Err("Speech-to-Text returned no transcript".to_string());
        }
        Ok(TranscriptionResult {
            transcript,
            clip_duration_ms: request.clip.duration_ms,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RecognizeResponse {
    #[serde(default)]
    results: Vec<RecognizeResult>,
}

#[derive(Debug, Deserialize)]
struct RecognizeResult {
    #[serde(default)]
    alternatives: Vec<RecognizeAlternative>,
}

#[derive(Debug, Deserialize)]
struct RecognizeAlternative {
    transcript: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordingHandle {
    pub input_device_name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AudioClip {
    pub wav_bytes: Vec<u8>,
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub duration_ms: u64,
}

trait AudioCapture {
    fn start_recording(&mut self) -> Result<RecordingHandle, String>;
    fn stop_recording(&mut self) -> Result<AudioClip, String>;
    fn cancel_recording(&mut self);
}

#[derive(Default)]
struct CpalAudioCapture {
    active: Option<ActiveRecording>,
}

struct ActiveRecording {
    stream: cpal::Stream,
    samples: Arc<std::sync::Mutex<Vec<i16>>>,
    sample_rate_hz: u32,
    channels: u16,
    input_device_name: Option<String>,
}

impl AudioCapture for CpalAudioCapture {
    fn start_recording(&mut self) -> Result<RecordingHandle, String> {
        if self.active.is_some() {
            return Err("A microphone recording is already active".to_string());
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No default microphone input device is available".to_string())?;
        let input_device_name = device.name().ok();
        let default_config = device
            .default_input_config()
            .map_err(|error| format!("Failed to inspect microphone config: {error}"))?;
        let stream_config: cpal::StreamConfig = default_config.clone().into();
        let channels = stream_config.channels;
        let sample_rate_hz = stream_config.sample_rate.0;
        let samples = Arc::new(std::sync::Mutex::new(Vec::<i16>::new()));
        let sink = samples.clone();
        let err_fn = move |error| {
            tracing::warn!("voice playground microphone capture error: {}", error);
        };

        let stream = match default_config.sample_format() {
            cpal::SampleFormat::I16 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[i16], _| extend_i16_samples(&sink, data),
                    err_fn,
                    None,
                )
                .map_err(|error| format!("Failed to start i16 microphone capture: {error}"))?,
            cpal::SampleFormat::U16 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[u16], _| extend_u16_samples(&sink, data),
                    err_fn,
                    None,
                )
                .map_err(|error| format!("Failed to start u16 microphone capture: {error}"))?,
            cpal::SampleFormat::F32 => device
                .build_input_stream(
                    &stream_config,
                    move |data: &[f32], _| extend_f32_samples(&sink, data),
                    err_fn,
                    None,
                )
                .map_err(|error| format!("Failed to start f32 microphone capture: {error}"))?,
            other => {
                return Err(format!("Unsupported microphone sample format: {other:?}"));
            }
        };
        stream
            .play()
            .map_err(|error| format!("Failed to activate microphone capture: {error}"))?;

        self.active = Some(ActiveRecording {
            stream,
            samples,
            sample_rate_hz,
            channels,
            input_device_name: input_device_name.clone(),
        });
        Ok(RecordingHandle { input_device_name })
    }

    fn stop_recording(&mut self) -> Result<AudioClip, String> {
        let Some(active) = self.active.take() else {
            return Err("No active microphone recording is available".to_string());
        };
        let samples = active
            .samples
            .lock()
            .map_err(|_| "Voice recording buffer lock is poisoned".to_string())?
            .clone();
        drop(active.stream);
        audio_clip_from_i16_samples(samples, active.sample_rate_hz, active.channels)
    }

    fn cancel_recording(&mut self) {
        self.active.take();
    }
}

fn extend_i16_samples(target: &Arc<std::sync::Mutex<Vec<i16>>>, samples: &[i16]) {
    if let Ok(mut buffer) = target.lock() {
        buffer.extend_from_slice(samples);
    }
}

fn extend_u16_samples(target: &Arc<std::sync::Mutex<Vec<i16>>>, samples: &[u16]) {
    if let Ok(mut buffer) = target.lock() {
        buffer.extend(
            samples
                .iter()
                .map(|sample| (*sample as i32).saturating_sub(i16::MAX as i32 + 1) as i16),
        );
    }
}

fn extend_f32_samples(target: &Arc<std::sync::Mutex<Vec<i16>>>, samples: &[f32]) {
    if let Ok(mut buffer) = target.lock() {
        buffer.extend(samples.iter().map(|sample| {
            (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
        }));
    }
}

fn audio_clip_from_i16_samples(
    samples: Vec<i16>,
    sample_rate_hz: u32,
    channels: u16,
) -> Result<AudioClip, String> {
    if sample_rate_hz == 0 || channels == 0 {
        return Err("Microphone configuration is invalid".to_string());
    }
    if samples.is_empty() {
        return Err("Recorded clip is empty".to_string());
    }

    let frames = samples.len() / usize::from(channels);
    let duration_ms = ((frames as f64 / f64::from(sample_rate_hz)) * 1000.0).round() as u64;
    Ok(AudioClip {
        wav_bytes: encode_wav_i16(&samples, sample_rate_hz, channels),
        sample_rate_hz,
        channels,
        duration_ms,
    })
}

fn encode_wav_i16(samples: &[i16], sample_rate_hz: u32, channels: u16) -> Vec<u8> {
    let data_len = samples.len().saturating_mul(std::mem::size_of::<i16>());
    let data_len_u32 = u32::try_from(data_len).unwrap_or(u32::MAX);
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36_u32.saturating_add(data_len_u32)).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate_hz.to_le_bytes());
    let byte_rate = sample_rate_hz
        .saturating_mul(u32::from(channels))
        .saturating_mul(2);
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels.saturating_mul(2);
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len_u32.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

trait AccessTokenProvider: Send + Sync {
    fn access_token(&self) -> Result<String, String>;
}

struct GcloudAccessTokenProvider {
    cli: Arc<dyn GcloudCli>,
}

impl GcloudAccessTokenProvider {
    fn new(cli: Arc<dyn GcloudCli>) -> Self {
        Self { cli }
    }
}

impl AccessTokenProvider for GcloudAccessTokenProvider {
    fn access_token(&self) -> Result<String, String> {
        self.cli
            .run(&["auth", "print-access-token"])
            .map(|value| value.to_string())
    }
}

#[derive(Clone)]
struct GcloudEnvironmentProbe {
    cli: Arc<dyn GcloudCli>,
}

impl GcloudEnvironmentProbe {
    fn new(cli: Arc<dyn GcloudCli>) -> Self {
        Self { cli }
    }

    fn describe(&self, config_project_id: Option<String>) -> (Option<String>, Option<String>) {
        let active_account = self
            .cli
            .run(&["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"])
            .ok()
            .map(|value| value.to_string())
            .filter(|value| !value.trim().is_empty());
        let project_id = config_project_id.or_else(|| {
            self.cli
                .run(&["config", "get-value", "project"])
                .ok()
                .map(|value| value.to_string())
                .filter(|value| {
                    let trimmed = value.trim();
                    !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("(unset)")
                })
        });
        (active_account, project_id)
    }
}

trait GcloudCli: Send + Sync {
    fn run(&self, args: &[&str]) -> Result<String, String>;
}

struct SystemGcloudCli;

impl GcloudCli for SystemGcloudCli {
    fn run(&self, args: &[&str]) -> Result<String, String> {
        let output = Command::new("gcloud")
            .args(args)
            .output()
            .map_err(|error| format!("failed to spawn gcloud {:?}: {}", args, error))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let detail = if stderr.is_empty() { stdout } else { stderr };
            return Err(format!("gcloud {:?} failed: {}", args, detail));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

fn env_project_id() -> Option<String> {
    GCLOUD_PROJECT_ENV_KEYS.iter().find_map(|key| {
        std::env::var(key)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, VecDeque};
    use std::sync::Mutex;

    #[derive(Default)]
    struct MockBackend {
        transcript: Mutex<Option<Result<TranscriptionResult, String>>>,
    }

    impl MockBackend {
        fn with_transcript(result: Result<TranscriptionResult, String>) -> Self {
            Self {
                transcript: Mutex::new(Some(result)),
            }
        }
    }

    impl VoiceTranscriptionBackend for MockBackend {
        fn transcribe(&self, request: &TranscriptionRequest) -> Result<TranscriptionResult, String> {
            let result = self
                .transcript
                .lock()
                .expect("mock backend lock")
                .take()
                .expect("mock backend result");
            match result {
                Ok(mut transcript) => {
                    transcript.clip_duration_ms = request.clip.duration_ms;
                    Ok(transcript)
                }
                Err(error) => Err(error),
            }
        }
    }

    struct MockCapture {
        start_result: Option<Result<RecordingHandle, String>>,
        stop_result: Option<Result<AudioClip, String>>,
        cancelled: bool,
    }

    impl MockCapture {
        fn ready(clip_duration_ms: u64) -> Self {
            Self {
                start_result: Some(Ok(RecordingHandle {
                    input_device_name: Some("Mock Mic".to_string()),
                })),
                stop_result: Some(Ok(AudioClip {
                    wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 16_000, 1),
                    sample_rate_hz: 16_000,
                    channels: 1,
                    duration_ms: clip_duration_ms,
                })),
                cancelled: false,
            }
        }
    }

    impl AudioCapture for MockCapture {
        fn start_recording(&mut self) -> Result<RecordingHandle, String> {
            self.start_result
                .take()
                .unwrap_or_else(|| Err("missing start result".to_string()))
        }

        fn stop_recording(&mut self) -> Result<AudioClip, String> {
            self.stop_result
                .take()
                .unwrap_or_else(|| Err("missing stop result".to_string()))
        }

        fn cancel_recording(&mut self) {
            self.cancelled = true;
        }
    }

    struct MockGcloudCli {
        responses: HashMap<Vec<String>, Result<String, String>>,
    }

    impl MockGcloudCli {
        fn from_pairs(pairs: Vec<(Vec<&str>, Result<&str, &str>)>) -> Self {
            let responses = pairs
                .into_iter()
                .map(|(key, value)| {
                    (
                        key.into_iter().map(str::to_string).collect::<Vec<_>>(),
                        value.map(str::to_string).map_err(str::to_string),
                    )
                })
                .collect();
            Self { responses }
        }
    }

    impl GcloudCli for MockGcloudCli {
        fn run(&self, args: &[&str]) -> Result<String, String> {
            self.responses
                .get(&args.iter().map(|value| value.to_string()).collect::<Vec<_>>())
                .cloned()
                .unwrap_or_else(|| Err(format!("missing mock response for {:?}", args)))
        }
    }

    #[test]
    fn environment_probe_prefers_configured_project_and_reports_active_account() {
        let cli = Arc::new(MockGcloudCli::from_pairs(vec![
            (
                vec!["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"],
                Ok("chris@openagents.com"),
            ),
            (vec!["config", "get-value", "project"], Ok("openagentsgemini")),
        ]));
        let probe = GcloudEnvironmentProbe::new(cli);

        let (account, project) = probe.describe(Some("forced-project".to_string()));

        assert_eq!(account.as_deref(), Some("chris@openagents.com"));
        assert_eq!(project.as_deref(), Some("forced-project"));
    }

    #[test]
    fn access_token_provider_returns_active_gcloud_token() {
        let cli = Arc::new(MockGcloudCli::from_pairs(vec![(
            vec!["auth", "print-access-token"],
            Ok("ya29.mock-token"),
        )]));
        let provider = GcloudAccessTokenProvider::new(cli);

        let token = provider.access_token().expect("token");

        assert_eq!(token, "ya29.mock-token");
    }

    #[test]
    fn controller_start_stop_transcribes_recorded_clip() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_transcript(Ok(TranscriptionResult {
            transcript: "Open Agents voice smoke test.".to_string(),
            clip_duration_ms: 0,
        }))) as Box<dyn VoiceTranscriptionBackend>;
        let capture = Box::new(MockCapture::ready(820)) as Box<dyn AudioCapture>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![
            (
                vec!["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"],
                Ok("chris@openagents.com"),
            ),
            (vec!["config", "get-value", "project"], Ok("openagentsgemini")),
        ])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, probe);

        let started = controller.handle_command(VoicePlaygroundCommand::StartRecording);
        let updates = controller.handle_command(
            VoicePlaygroundCommand::StopRecordingAndTranscribe {
                request_id: "voice-req-1".to_string(),
            },
        );

        assert_eq!(
            started,
            vec![VoicePlaygroundUpdate::RecordingStarted {
                input_device_name: Some("Mock Mic".to_string()),
            }]
        );
        assert_eq!(
            updates.first(),
            Some(&VoicePlaygroundUpdate::RecordingStopped {
                clip_duration_ms: 820
            })
        );
        assert_eq!(
            updates.get(1),
            Some(&VoicePlaygroundUpdate::TranscriptionStarted {
                request_id: "voice-req-1".to_string()
            })
        );
        match updates.get(2).expect("transcription completion") {
            VoicePlaygroundUpdate::TranscriptionCompleted {
                request_id,
                transcript,
                clip_duration_ms,
                latency_ms,
            } => {
                assert_eq!(request_id, "voice-req-1");
                assert_eq!(transcript, "Open Agents voice smoke test.");
                assert_eq!(*clip_duration_ms, 820);
                assert!(*latency_ms <= 5_000);
            }
            other => panic!("unexpected update: {:?}", other),
        }
        assert_eq!(controller.snapshot.recording_state, VoiceRecordingState::Idle);
        assert_eq!(
            controller.snapshot.transcription_state,
            VoiceTranscriptionState::Ready
        );
        assert_eq!(
            controller.snapshot.last_transcript.as_deref(),
            Some("Open Agents voice smoke test.")
        );
        assert!(
            !controller
                .snapshot
                .last_error
                .as_deref()
                .is_some_and(|value| !value.is_empty())
        );
    }

    #[test]
    fn controller_cancel_resets_recording_state() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_transcript(Err(
            "backend should not be called".to_string(),
        ))) as Box<dyn VoiceTranscriptionBackend>;
        let capture = Box::new(MockCapture::ready(400)) as Box<dyn AudioCapture>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, probe);

        let _ = controller.handle_command(VoicePlaygroundCommand::StartRecording);
        let cancelled = controller.handle_command(VoicePlaygroundCommand::CancelRecording);

        assert_eq!(cancelled, vec![VoicePlaygroundUpdate::RecordingCancelled]);
        assert_eq!(controller.snapshot.recording_state, VoiceRecordingState::Idle);
        assert_eq!(
            controller.snapshot.transcription_state,
            VoiceTranscriptionState::Idle
        );
    }

    #[test]
    fn encode_wav_i16_produces_riff_header() {
        let bytes = encode_wav_i16(&[1_i16, -1_i16, 2_i16, -2_i16], 16_000, 1);

        assert!(bytes.starts_with(b"RIFF"));
        assert_eq!(&bytes[8..12], b"WAVE");
        assert!(bytes.len() > 44);
    }

    #[test]
    fn worker_drains_initial_snapshot() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_transcript(Ok(TranscriptionResult {
            transcript: "mock".to_string(),
            clip_duration_ms: 0,
        }))) as Box<dyn VoiceTranscriptionBackend>;
        let capture = Box::new(MockCapture::ready(100)) as Box<dyn AudioCapture + Send>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let worker = VoicePlaygroundWorker::with_components(config, backend, capture, probe);

        let mut updates = VecDeque::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && updates.is_empty() {
            updates.extend(worker.drain_updates());
            std::thread::sleep(Duration::from_millis(20));
        }

        assert!(matches!(
            updates.pop_front(),
            Some(VoicePlaygroundUpdate::Snapshot(_))
        ));
    }
}
