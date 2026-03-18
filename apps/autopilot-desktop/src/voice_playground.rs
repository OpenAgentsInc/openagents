use std::io::{BufReader, Cursor};
use std::process::Command;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use reqwest::blocking::Client as HttpClient;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::Deserialize;
use serde_json::json;

const WORKER_POLL: Duration = Duration::from_millis(120);
const GOOGLE_STT_BASE_URL: &str = "https://us-speech.googleapis.com";
const GOOGLE_STT_LOCATION: &str = "us";
const GOOGLE_STT_MODEL: &str = "chirp_3";
const GOOGLE_STT_LANGUAGE_CODE: &str = "en-US";
const GOOGLE_TTS_BASE_URL: &str = "https://texttospeech.googleapis.com";
const GOOGLE_TTS_VOICE_NAME: &str = "en-US-Chirp3-HD-Charon";
const GOOGLE_TTS_LANGUAGE_CODE: &str = "en-US";
const GOOGLE_TTS_SAMPLE_RATE_HZ: u32 = 24_000;
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
    pub tts_base_url: String,
    pub tts_voice_name: String,
    pub tts_language_code: String,
    pub tts_sample_rate_hz: u32,
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
            tts_base_url: std::env::var("OPENAGENTS_GOOGLE_TTS_BASE_URL")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_TTS_BASE_URL.to_string()),
            tts_voice_name: std::env::var("OPENAGENTS_GOOGLE_TTS_VOICE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_TTS_VOICE_NAME.to_string()),
            tts_language_code: std::env::var("OPENAGENTS_GOOGLE_TTS_LANGUAGE_CODE")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| GOOGLE_TTS_LANGUAGE_CODE.to_string()),
            tts_sample_rate_hz: std::env::var("OPENAGENTS_GOOGLE_TTS_SAMPLE_RATE_HZ")
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .filter(|value| *value > 0)
                .unwrap_or(GOOGLE_TTS_SAMPLE_RATE_HZ),
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VoiceSynthesisState {
    #[default]
    Idle,
    Running,
    Ready,
    Error,
}

impl VoiceSynthesisState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Error => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum VoicePlaybackState {
    #[default]
    Idle,
    Playing,
    Completed,
    Error,
}

impl VoicePlaybackState {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Playing => "playing",
            Self::Completed => "completed",
            Self::Error => "failed",
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
    pub tts_voice_name: String,
    pub tts_language_code: String,
    pub recording_state: VoiceRecordingState,
    pub transcription_state: VoiceTranscriptionState,
    pub synthesis_state: VoiceSynthesisState,
    pub playback_state: VoicePlaybackState,
    pub input_device_name: Option<String>,
    pub pending_request_id: Option<String>,
    pub last_request_id: Option<String>,
    pub last_clip_duration_ms: Option<u64>,
    pub last_transcript: Option<String>,
    pub last_transcription_latency_ms: Option<u64>,
    pub last_synthesis_request_id: Option<String>,
    pub last_synthesis_latency_ms: Option<u64>,
    pub last_speech_text: Option<String>,
    pub last_tts_duration_ms: Option<u64>,
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
    SynthesisStarted {
        request_id: String,
        text: String,
    },
    SynthesisCompleted {
        request_id: String,
        text: String,
        voice_name: String,
        latency_ms: u64,
        duration_ms: Option<u64>,
    },
    SynthesisFailed {
        request_id: String,
        error: String,
    },
    PlaybackStarted {
        request_id: String,
    },
    PlaybackStopped {
        request_id: Option<String>,
        reason: &'static str,
    },
    PlaybackFailed {
        request_id: Option<String>,
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
    SynthesizeAndPlay {
        request_id: String,
        text: String,
    },
    ReplayLastSynthesis,
    StopPlayback,
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
                let backend = Box::new(GoogleCloudVoiceBackend::new(config.clone(), auth))
                    as Box<dyn VoiceBackend>;
                let capture = Box::new(CpalAudioCapture::default()) as Box<dyn AudioCapture>;
                let player = Box::new(RodioAudioPlayer::default()) as Box<dyn AudioPlayer>;
                let mut controller =
                    VoicePlaygroundController::new(config, backend, capture, player, environment);
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
                        Err(RecvTimeoutError::Timeout) => {
                            if let Some(update) = controller.handle_tick() {
                                let _ = update_tx.send(update);
                            }
                        }
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
        backend: Box<dyn VoiceBackend>,
        capture: Box<dyn AudioCapture + Send>,
        player: Box<dyn AudioPlayer + Send>,
        environment: GcloudEnvironmentProbe,
    ) -> Self {
        let (command_tx, command_rx) = mpsc::channel::<VoicePlaygroundWorkerCommand>();
        let (update_tx, update_rx) = mpsc::channel::<VoicePlaygroundUpdate>();
        let worker_thread = std::thread::Builder::new()
            .name("voice-playground".to_string())
            .spawn(move || {
                let mut controller =
                    VoicePlaygroundController::new(config, backend, capture, player, environment);
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
                        Err(RecvTimeoutError::Timeout) => {
                            if let Some(update) = controller.handle_tick() {
                                let _ = update_tx.send(update);
                            }
                        }
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
    backend: Box<dyn VoiceBackend>,
    capture: Box<dyn AudioCapture>,
    player: Box<dyn AudioPlayer>,
    environment: GcloudEnvironmentProbe,
    snapshot: VoicePlaygroundSnapshot,
    last_synthesized_audio: Option<SynthesizedAudio>,
    playback_request_id: Option<String>,
}

impl VoicePlaygroundController {
    fn new(
        config: VoicePlaygroundConfig,
        backend: Box<dyn VoiceBackend>,
        capture: Box<dyn AudioCapture>,
        player: Box<dyn AudioPlayer>,
        environment: GcloudEnvironmentProbe,
    ) -> Self {
        let mut snapshot = VoicePlaygroundSnapshot {
            backend_label: "google-cloud-speech-v2 + google-cloud-tts".to_string(),
            project_id: config.project_id.clone(),
            stt_location: config.stt_location.clone(),
            stt_model: config.stt_model.clone(),
            stt_language_code: config.stt_language_code.clone(),
            tts_voice_name: config.tts_voice_name.clone(),
            tts_language_code: config.tts_language_code.clone(),
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
            player,
            environment,
            snapshot,
            last_synthesized_audio: None,
            playback_request_id: None,
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
            VoicePlaygroundCommand::SynthesizeAndPlay { request_id, text } => {
                self.synthesize_and_play(request_id, text)
            }
            VoicePlaygroundCommand::ReplayLastSynthesis => self.replay_last_synthesis(),
            VoicePlaygroundCommand::StopPlayback => self.stop_playback(),
        }
    }

    fn handle_tick(&mut self) -> Option<VoicePlaygroundUpdate> {
        if self.snapshot.playback_state == VoicePlaybackState::Playing && !self.player.is_playing() {
            self.snapshot.playback_state = VoicePlaybackState::Completed;
            self.snapshot.last_action = Some("Voice playback completed".to_string());
            let request_id = self.playback_request_id.take();
            return Some(VoicePlaygroundUpdate::PlaybackStopped {
                request_id,
                reason: "completed",
            });
        }
        None
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

    fn synthesize_and_play(&mut self, request_id: String, text: String) -> Vec<VoicePlaygroundUpdate> {
        if text.trim().is_empty() {
            self.snapshot.synthesis_state = VoiceSynthesisState::Error;
            self.snapshot.playback_state = VoicePlaybackState::Error;
            self.snapshot.last_error =
                Some("Text is required before synthesizing speech".to_string());
            self.snapshot.last_action = Some("Voice synthesis blocked".to_string());
            return vec![VoicePlaygroundUpdate::SynthesisFailed {
                request_id,
                error: "Text is required before synthesizing speech".to_string(),
            }];
        }

        self.snapshot.synthesis_state = VoiceSynthesisState::Running;
        self.snapshot.playback_state = VoicePlaybackState::Idle;
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some(format!("Synthesizing speech [{}]", request_id));
        let mut updates = vec![VoicePlaygroundUpdate::SynthesisStarted {
            request_id: request_id.clone(),
            text: text.clone(),
        }];
        let started_at = Instant::now();
        match self.backend.synthesize(&SynthesisRequest {
            request_id: request_id.clone(),
            project_id: self.snapshot.project_id.clone(),
            text: text.clone(),
        }) {
            Ok(synthesized) => {
                let latency_ms =
                    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                let play_result = self.player.play_wav(synthesized.audio.wav_bytes.as_slice());
                self.snapshot.synthesis_state = VoiceSynthesisState::Ready;
                self.snapshot.last_synthesis_request_id = Some(request_id.clone());
                self.snapshot.last_synthesis_latency_ms = Some(latency_ms);
                self.snapshot.last_speech_text = Some(text.clone());
                self.snapshot.last_tts_duration_ms = synthesized.audio.duration_ms;
                self.snapshot.tts_voice_name = synthesized.voice_name.clone();
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some(format!(
                    "Synthesized speech [{}] in {} ms",
                    request_id, latency_ms
                ));
                self.last_synthesized_audio = Some(synthesized.audio.clone());
                updates.push(VoicePlaygroundUpdate::SynthesisCompleted {
                    request_id: request_id.clone(),
                    text,
                    voice_name: synthesized.voice_name,
                    latency_ms,
                    duration_ms: synthesized.audio.duration_ms,
                });
                match play_result {
                    Ok(()) => {
                        self.snapshot.playback_state = VoicePlaybackState::Playing;
                        self.playback_request_id = Some(request_id.clone());
                        updates.push(VoicePlaygroundUpdate::PlaybackStarted { request_id });
                    }
                    Err(error) => {
                        self.snapshot.playback_state = VoicePlaybackState::Error;
                        self.snapshot.last_error = Some(error.clone());
                        updates.push(VoicePlaygroundUpdate::PlaybackFailed {
                            request_id: Some(request_id),
                            error,
                        });
                    }
                }
            }
            Err(error) => {
                self.snapshot.synthesis_state = VoiceSynthesisState::Error;
                self.snapshot.playback_state = VoicePlaybackState::Error;
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action = Some("Voice synthesis failed".to_string());
                updates.push(VoicePlaygroundUpdate::SynthesisFailed { request_id, error });
            }
        }

        updates
    }

    fn replay_last_synthesis(&mut self) -> Vec<VoicePlaygroundUpdate> {
        let Some(audio) = self.last_synthesized_audio.as_ref() else {
            self.snapshot.playback_state = VoicePlaybackState::Error;
            self.snapshot.last_error = Some("No synthesized clip is available to replay".to_string());
            self.snapshot.last_action = Some("Voice replay blocked".to_string());
            return vec![VoicePlaygroundUpdate::PlaybackFailed {
                request_id: self.snapshot.last_synthesis_request_id.clone(),
                error: "No synthesized clip is available to replay".to_string(),
            }];
        };
        let request_id = self
            .snapshot
            .last_synthesis_request_id
            .clone()
            .unwrap_or_else(|| "voice-replay".to_string());
        match self.player.play_wav(audio.wav_bytes.as_slice()) {
            Ok(()) => {
                self.snapshot.playback_state = VoicePlaybackState::Playing;
                self.snapshot.last_error = None;
                self.snapshot.last_action = Some("Replaying synthesized voice clip".to_string());
                self.playback_request_id = Some(request_id.clone());
                vec![VoicePlaygroundUpdate::PlaybackStarted { request_id }]
            }
            Err(error) => {
                self.snapshot.playback_state = VoicePlaybackState::Error;
                self.snapshot.last_error = Some(error.clone());
                self.snapshot.last_action = Some("Voice replay failed".to_string());
                vec![VoicePlaygroundUpdate::PlaybackFailed {
                    request_id: Some(request_id),
                    error,
                }]
            }
        }
    }

    fn stop_playback(&mut self) -> Vec<VoicePlaygroundUpdate> {
        self.player.stop();
        self.snapshot.playback_state = VoicePlaybackState::Idle;
        self.snapshot.last_error = None;
        self.snapshot.last_action = Some("Stopped voice playback".to_string());
        vec![VoicePlaygroundUpdate::PlaybackStopped {
            request_id: self.playback_request_id.take(),
            reason: "stopped",
        }]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranscriptionResult {
    pub transcript: String,
    pub clip_duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesizedAudio {
    pub wav_bytes: Vec<u8>,
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SynthesisResult {
    pub voice_name: String,
    pub audio: SynthesizedAudio,
}

#[derive(Clone, Debug)]
pub struct TranscriptionRequest {
    pub request_id: String,
    pub clip: AudioClip,
    pub project_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SynthesisRequest {
    pub request_id: String,
    pub project_id: Option<String>,
    pub text: String,
}

pub trait VoiceBackend: Send {
    fn transcribe(&self, request: &TranscriptionRequest) -> Result<TranscriptionResult, String>;
    fn synthesize(&self, request: &SynthesisRequest) -> Result<SynthesisResult, String>;
}

struct GoogleCloudVoiceBackend {
    config: VoicePlaygroundConfig,
    auth: Arc<dyn AccessTokenProvider>,
    http: HttpClient,
}

impl GoogleCloudVoiceBackend {
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

    fn synthesize_url(&self) -> String {
        format!(
            "{}/v1/text:synthesize",
            self.config.tts_base_url.trim_end_matches('/')
        )
    }

    fn synthesis_payload(&self, text: &str) -> serde_json::Value {
        json!({
            "input": {
                "text": text,
            },
            "voice": {
                "languageCode": self.config.tts_language_code,
                "name": self.config.tts_voice_name,
            },
            "audioConfig": {
                "audioEncoding": "LINEAR16",
                "sampleRateHertz": self.config.tts_sample_rate_hz,
            },
        })
    }
}

impl VoiceBackend for GoogleCloudVoiceBackend {
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

    fn synthesize(&self, request: &SynthesisRequest) -> Result<SynthesisResult, String> {
        let project_id = request
            .project_id
            .clone()
            .or_else(|| self.config.project_id.clone())
            .ok_or_else(|| {
                "Google Cloud project is unset. Configure `OPENAGENTS_GOOGLE_CLOUD_PROJECT` or run `gcloud config set project ...`.".to_string()
            })?;
        let response = self
            .http
            .post(self.synthesize_url())
            .bearer_auth(self.auth.access_token()?)
            .header("x-goog-user-project", project_id.as_str())
            .json(&self.synthesis_payload(request.text.as_str()))
            .send()
            .map_err(|error| format!("Text-to-Speech request failed: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .unwrap_or_else(|_| "failed to read response body".to_string());
            return Err(format!("Text-to-Speech request failed ({status}): {body}"));
        }

        let payload: SynthesizeResponse = response
            .json()
            .map_err(|error| format!("Text-to-Speech response decode failed: {error}"))?;
        let audio_content = payload
            .audio_content
            .ok_or_else(|| "Text-to-Speech returned no audio".to_string())?;
        let wav_bytes = STANDARD
            .decode(audio_content.as_bytes())
            .map_err(|error| format!("Text-to-Speech audio decode failed: {error}"))?;
        Ok(SynthesisResult {
            voice_name: self.config.tts_voice_name.clone(),
            audio: SynthesizedAudio {
                duration_ms: wav_duration_ms(wav_bytes.as_slice()),
                wav_bytes,
            },
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

#[derive(Debug, Deserialize)]
struct SynthesizeResponse {
    #[serde(rename = "audioContent")]
    audio_content: Option<String>,
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

fn wav_duration_ms(bytes: &[u8]) -> Option<u64> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let channels = u16::from_le_bytes([*bytes.get(22)?, *bytes.get(23)?]);
    let sample_rate_hz = u32::from_le_bytes([
        *bytes.get(24)?,
        *bytes.get(25)?,
        *bytes.get(26)?,
        *bytes.get(27)?,
    ]);
    let bits_per_sample = u16::from_le_bytes([*bytes.get(34)?, *bytes.get(35)?]);
    let data_len = u32::from_le_bytes([
        *bytes.get(40)?,
        *bytes.get(41)?,
        *bytes.get(42)?,
        *bytes.get(43)?,
    ]);
    if channels == 0 || sample_rate_hz == 0 || bits_per_sample == 0 {
        return None;
    }
    let bytes_per_frame = u32::from(channels).saturating_mul(u32::from(bits_per_sample / 8));
    if bytes_per_frame == 0 {
        return None;
    }
    let frames = data_len / bytes_per_frame;
    Some(((frames as f64 / f64::from(sample_rate_hz)) * 1000.0).round() as u64)
}

trait AudioPlayer {
    fn play_wav(&mut self, wav_bytes: &[u8]) -> Result<(), String>;
    fn stop(&mut self);
    fn is_playing(&self) -> bool;
}

#[derive(Default)]
struct RodioAudioPlayer {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
}

impl RodioAudioPlayer {
    fn ensure_output(&mut self) -> Result<(), String> {
        if self.stream.is_some() && self.handle.is_some() {
            return Ok(());
        }
        let (stream, handle) = OutputStream::try_default()
            .map_err(|error| format!("Failed to open speaker output: {error}"))?;
        self.stream = Some(stream);
        self.handle = Some(handle);
        Ok(())
    }
}

impl AudioPlayer for RodioAudioPlayer {
    fn play_wav(&mut self, wav_bytes: &[u8]) -> Result<(), String> {
        self.ensure_output()?;
        self.stop();
        let handle = self
            .handle
            .as_ref()
            .ok_or_else(|| "Speaker output is unavailable".to_string())?;
        let sink =
            Sink::try_new(handle).map_err(|error| format!("Failed to create audio sink: {error}"))?;
        let cursor = Cursor::new(wav_bytes.to_vec());
        let decoder = Decoder::new(BufReader::new(cursor))
            .map_err(|error| format!("Failed to decode synthesized audio: {error}"))?;
        sink.append(decoder);
        sink.play();
        self.sink = Some(sink);
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
    }

    fn is_playing(&self) -> bool {
        self.sink.as_ref().is_some_and(|sink| !sink.empty())
    }
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
        synthesis: Mutex<Option<Result<SynthesisResult, String>>>,
    }

    impl MockBackend {
        fn with_transcript(result: Result<TranscriptionResult, String>) -> Self {
            Self {
                transcript: Mutex::new(Some(result)),
                synthesis: Mutex::new(Some(Ok(SynthesisResult {
                    voice_name: GOOGLE_TTS_VOICE_NAME.to_string(),
                    audio: SynthesizedAudio {
                        wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 24_000, 1),
                        duration_ms: Some(8),
                    },
                }))),
            }
        }

        fn with_synthesis(result: Result<SynthesisResult, String>) -> Self {
            Self {
                transcript: Mutex::new(Some(Ok(TranscriptionResult {
                    transcript: "unused".to_string(),
                    clip_duration_ms: 0,
                }))),
                synthesis: Mutex::new(Some(result)),
            }
        }
    }

    impl VoiceBackend for MockBackend {
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

        fn synthesize(&self, _request: &SynthesisRequest) -> Result<SynthesisResult, String> {
            self.synthesis
                .lock()
                .expect("mock synthesis lock")
                .take()
                .expect("mock synthesis result")
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

    #[derive(Clone, Default)]
    struct MockPlayerHandle {
        state: Arc<Mutex<MockPlayerState>>,
    }

    impl MockPlayerHandle {
        fn set_playing(&self, playing: bool) {
            self.state.lock().expect("mock player state").playing = playing;
        }

        fn plays(&self) -> usize {
            self.state.lock().expect("mock player state").plays
        }

        fn fail_on_play(&self, error: &str) {
            self.state.lock().expect("mock player state").fail_on_play = Some(error.to_string());
        }
    }

    #[derive(Default)]
    struct MockPlayerState {
        playing: bool,
        plays: usize,
        fail_on_play: Option<String>,
    }

    struct MockPlayer {
        state: Arc<Mutex<MockPlayerState>>,
    }

    impl Default for MockPlayer {
        fn default() -> Self {
            Self::with_handle().0
        }
    }

    impl MockPlayer {
        fn with_handle() -> (Self, MockPlayerHandle) {
            let handle = MockPlayerHandle::default();
            (
                Self {
                    state: handle.state.clone(),
                },
                handle,
            )
        }
    }

    impl AudioPlayer for MockPlayer {
        fn play_wav(&mut self, _wav_bytes: &[u8]) -> Result<(), String> {
            let mut state = self.state.lock().expect("mock player state");
            if let Some(error) = state.fail_on_play.take() {
                return Err(error);
            }
            state.playing = true;
            state.plays = state.plays.saturating_add(1);
            Ok(())
        }

        fn stop(&mut self) {
            self.state.lock().expect("mock player state").playing = false;
        }

        fn is_playing(&self) -> bool {
            self.state.lock().expect("mock player state").playing
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
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(820)) as Box<dyn AudioCapture>;
        let player = Box::new(MockPlayer::default()) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![
            (
                vec!["auth", "list", "--filter=status:ACTIVE", "--format=value(account)"],
                Ok("chris@openagents.com"),
            ),
            (vec!["config", "get-value", "project"], Ok("openagentsgemini")),
        ])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

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
        ))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(400)) as Box<dyn AudioCapture>;
        let player = Box::new(MockPlayer::default()) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

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
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(100)) as Box<dyn AudioCapture + Send>;
        let player = Box::new(MockPlayer::default()) as Box<dyn AudioPlayer + Send>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let worker = VoicePlaygroundWorker::with_components(config, backend, capture, player, probe);

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

    #[test]
    fn controller_synthesizes_and_tracks_playback_state() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_synthesis(Ok(SynthesisResult {
            voice_name: GOOGLE_TTS_VOICE_NAME.to_string(),
            audio: SynthesizedAudio {
                wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 24_000, 1),
                duration_ms: Some(12),
            },
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(120)) as Box<dyn AudioCapture>;
        let player = Box::new(MockPlayer::default()) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

        let updates = controller.handle_command(VoicePlaygroundCommand::SynthesizeAndPlay {
            request_id: "voice-tts-1".to_string(),
            text: "OpenAgents voice test".to_string(),
        });

        assert!(matches!(
            updates.first(),
            Some(VoicePlaygroundUpdate::SynthesisStarted { .. })
        ));
        assert!(matches!(
            updates.get(1),
            Some(VoicePlaygroundUpdate::SynthesisCompleted { .. })
        ));
        assert!(matches!(
            updates.get(2),
            Some(VoicePlaygroundUpdate::PlaybackStarted { .. })
        ));
        assert_eq!(controller.snapshot.synthesis_state, VoiceSynthesisState::Ready);
        assert_eq!(controller.snapshot.playback_state, VoicePlaybackState::Playing);
        assert_eq!(
            controller.snapshot.last_speech_text.as_deref(),
            Some("OpenAgents voice test")
        );
    }

    #[test]
    fn controller_replays_and_stops_last_synthesized_audio() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_synthesis(Ok(SynthesisResult {
            voice_name: GOOGLE_TTS_VOICE_NAME.to_string(),
            audio: SynthesizedAudio {
                wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 24_000, 1),
                duration_ms: Some(12),
            },
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(120)) as Box<dyn AudioCapture>;
        let (player, player_handle) = MockPlayer::with_handle();
        let player = Box::new(player) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

        let _ = controller.handle_command(VoicePlaygroundCommand::SynthesizeAndPlay {
            request_id: "voice-tts-1".to_string(),
            text: "OpenAgents voice test".to_string(),
        });
        assert_eq!(player_handle.plays(), 1);

        let stopped = controller.handle_command(VoicePlaygroundCommand::StopPlayback);
        assert!(matches!(
            stopped.as_slice(),
            [VoicePlaygroundUpdate::PlaybackStopped { reason: "stopped", .. }]
        ));
        assert_eq!(controller.snapshot.playback_state, VoicePlaybackState::Idle);

        let replayed = controller.handle_command(VoicePlaygroundCommand::ReplayLastSynthesis);
        assert!(matches!(
            replayed.as_slice(),
            [VoicePlaygroundUpdate::PlaybackStarted { .. }]
        ));
        assert_eq!(player_handle.plays(), 2);
        assert_eq!(controller.snapshot.playback_state, VoicePlaybackState::Playing);
    }

    #[test]
    fn controller_marks_playback_completed_when_player_finishes() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_synthesis(Ok(SynthesisResult {
            voice_name: GOOGLE_TTS_VOICE_NAME.to_string(),
            audio: SynthesizedAudio {
                wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 24_000, 1),
                duration_ms: Some(12),
            },
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(120)) as Box<dyn AudioCapture>;
        let (player, player_handle) = MockPlayer::with_handle();
        let player = Box::new(player) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

        let _ = controller.handle_command(VoicePlaygroundCommand::SynthesizeAndPlay {
            request_id: "voice-tts-1".to_string(),
            text: "OpenAgents voice test".to_string(),
        });
        player_handle.set_playing(false);

        let completed = controller.handle_tick();

        assert!(matches!(
            completed,
            Some(VoicePlaygroundUpdate::PlaybackStopped {
                reason: "completed",
                ..
            })
        ));
        assert_eq!(controller.snapshot.playback_state, VoicePlaybackState::Completed);
    }

    #[test]
    fn controller_reports_replay_failures_and_missing_clips() {
        let config = VoicePlaygroundConfig {
            project_id: Some("openagentsgemini".to_string()),
            ..VoicePlaygroundConfig::default()
        };
        let backend = Box::new(MockBackend::with_synthesis(Ok(SynthesisResult {
            voice_name: GOOGLE_TTS_VOICE_NAME.to_string(),
            audio: SynthesizedAudio {
                wav_bytes: encode_wav_i16(&[0_i16, 128_i16, -64_i16, 32_i16], 24_000, 1),
                duration_ms: Some(12),
            },
        }))) as Box<dyn VoiceBackend>;
        let capture = Box::new(MockCapture::ready(120)) as Box<dyn AudioCapture>;
        let (player, player_handle) = MockPlayer::with_handle();
        let player = Box::new(player) as Box<dyn AudioPlayer>;
        let probe = GcloudEnvironmentProbe::new(Arc::new(MockGcloudCli::from_pairs(vec![])));
        let mut controller = VoicePlaygroundController::new(config, backend, capture, player, probe);

        let missing_clip = controller.handle_command(VoicePlaygroundCommand::ReplayLastSynthesis);
        assert!(matches!(
            missing_clip.as_slice(),
            [VoicePlaygroundUpdate::PlaybackFailed { error, .. }]
            if error.contains("No synthesized clip")
        ));

        let _ = controller.handle_command(VoicePlaygroundCommand::SynthesizeAndPlay {
            request_id: "voice-tts-2".to_string(),
            text: "OpenAgents voice test".to_string(),
        });
        let _ = controller.handle_command(VoicePlaygroundCommand::StopPlayback);
        player_handle.fail_on_play("speaker output unavailable");

        let replay_failed = controller.handle_command(VoicePlaygroundCommand::ReplayLastSynthesis);

        assert!(matches!(
            replay_failed.as_slice(),
            [VoicePlaygroundUpdate::PlaybackFailed { error, .. }]
            if error.contains("speaker output unavailable")
        ));
        assert_eq!(controller.snapshot.playback_state, VoicePlaybackState::Error);
    }
}
