use crate::app_state::{PaneLoadState, RenderState};
use crate::voice_playground::{VoicePlaygroundSnapshot, VoicePlaygroundUpdate};

const TRANSCRIPT_PREVIEW_LIMIT: usize = 1_600;

pub(super) fn apply_voice_update(state: &mut RenderState, update: &VoicePlaygroundUpdate) -> bool {
    match update {
        VoicePlaygroundUpdate::Snapshot(snapshot) => apply_snapshot(state, snapshot),
        VoicePlaygroundUpdate::RecordingStarted { input_device_name } => {
            state.voice_playground.load_state = PaneLoadState::Ready;
            state.voice_playground.last_error = None;
            state.voice_playground.recording_state =
                crate::voice_playground::VoiceRecordingState::Recording;
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Idle;
            state.voice_playground.input_device_name = input_device_name.clone();
            state.voice_playground.last_action = Some(format!(
                "Recording microphone clip from {}",
                input_device_name.as_deref().unwrap_or("default input")
            ));
            true
        }
        VoicePlaygroundUpdate::RecordingCancelled => {
            state.voice_playground.load_state = PaneLoadState::Ready;
            state.voice_playground.last_error = None;
            state.voice_playground.recording_state =
                crate::voice_playground::VoiceRecordingState::Idle;
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Idle;
            state.voice_playground.pending_request_id = None;
            state.voice_playground.last_action = Some("Cancelled voice recording".to_string());
            true
        }
        VoicePlaygroundUpdate::RecordingStopped { clip_duration_ms } => {
            state.voice_playground.load_state = PaneLoadState::Loading;
            state.voice_playground.recording_state =
                crate::voice_playground::VoiceRecordingState::Idle;
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Running;
            state.voice_playground.clip_duration_ms = Some(*clip_duration_ms);
            state.voice_playground.last_error = None;
            state.voice_playground.last_action =
                Some(format!("Captured {} ms voice clip", clip_duration_ms));
            true
        }
        VoicePlaygroundUpdate::RecordingFailed { error } => {
            state.voice_playground.load_state = PaneLoadState::Error;
            state.voice_playground.recording_state =
                crate::voice_playground::VoiceRecordingState::Idle;
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Error;
            state.voice_playground.last_action = Some("Voice recording failed".to_string());
            state.voice_playground.last_error = Some(error.clone());
            true
        }
        VoicePlaygroundUpdate::TranscriptionStarted { request_id } => {
            state.voice_playground.load_state = PaneLoadState::Loading;
            state.voice_playground.pending_request_id = Some(request_id.clone());
            state.voice_playground.last_request_id = Some(request_id.clone());
            state.voice_playground.last_error = None;
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Running;
            state.voice_playground.last_action =
                Some(format!("Transcribing microphone clip [{}]", request_id));
            true
        }
        VoicePlaygroundUpdate::TranscriptionCompleted {
            request_id,
            transcript,
            clip_duration_ms,
            latency_ms,
        } => {
            state.voice_playground.load_state = PaneLoadState::Ready;
            state.voice_playground.pending_request_id = None;
            state.voice_playground.last_request_id = Some(request_id.clone());
            state.voice_playground.clip_duration_ms = Some(*clip_duration_ms);
            state.voice_playground.last_transcription_latency_ms = Some(*latency_ms);
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Ready;
            state.voice_playground.last_error = None;
            state.voice_playground.last_action = Some(format!(
                "Transcribed voice clip [{}] in {} ms",
                request_id, latency_ms
            ));
            state.voice_playground.transcript_chars = transcript.chars().count();
            state.voice_playground.transcript_preview =
                truncate_preview(transcript.as_str(), TRANSCRIPT_PREVIEW_LIMIT);
            true
        }
        VoicePlaygroundUpdate::TranscriptionFailed { request_id, error } => {
            state.voice_playground.load_state = PaneLoadState::Error;
            state.voice_playground.pending_request_id = None;
            state.voice_playground.last_request_id = Some(request_id.clone());
            state.voice_playground.transcription_state =
                crate::voice_playground::VoiceTranscriptionState::Error;
            state.voice_playground.last_action =
                Some(format!("Voice transcription failed [{}]", request_id));
            state.voice_playground.last_error = Some(error.clone());
            true
        }
    }
}

fn apply_snapshot(state: &mut RenderState, snapshot: &VoicePlaygroundSnapshot) -> bool {
    state.voice_playground.backend_label = snapshot.backend_label.clone();
    state.voice_playground.active_account = snapshot.active_account.clone();
    state.voice_playground.project_id = snapshot.project_id.clone();
    state.voice_playground.stt_location = snapshot.stt_location.clone();
    state.voice_playground.stt_model = snapshot.stt_model.clone();
    state.voice_playground.stt_language_code = snapshot.stt_language_code.clone();
    state.voice_playground.input_device_name = snapshot.input_device_name.clone();
    state.voice_playground.last_request_id = snapshot.last_request_id.clone();
    state.voice_playground.pending_request_id = snapshot.pending_request_id.clone();
    state.voice_playground.clip_duration_ms = snapshot.last_clip_duration_ms;
    state.voice_playground.last_transcription_latency_ms = snapshot.last_transcription_latency_ms;
    state.voice_playground.last_action = snapshot.last_action.clone();
    state.voice_playground.last_error = snapshot.last_error.clone();
    state.voice_playground.recording_state = snapshot.recording_state;
    state.voice_playground.transcription_state = snapshot.transcription_state;
    if let Some(transcript) = snapshot.last_transcript.as_ref() {
        state.voice_playground.transcript_chars = transcript.chars().count();
        state.voice_playground.transcript_preview =
            truncate_preview(transcript.as_str(), TRANSCRIPT_PREVIEW_LIMIT);
    }
    state.voice_playground.load_state = if snapshot.last_error.is_some() {
        PaneLoadState::Error
    } else {
        PaneLoadState::Ready
    };
    true
}

fn truncate_preview(raw: &str, limit: usize) -> String {
    let mut preview = String::new();
    for ch in raw.chars().take(limit) {
        preview.push(ch);
    }
    preview.trim().to_string()
}
