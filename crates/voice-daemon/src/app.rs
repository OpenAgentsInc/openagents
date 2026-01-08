//! Main application logic for voice-daemon

use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use voice::{VoiceEvent, VoiceSession};

use crate::hotkey::HotkeyListener;
use crate::text_insert::insert_text;

/// Run the daemon in foreground mode
pub fn run_foreground() -> Result<(), String> {
    tracing::info!("Voice daemon starting...");

    // Initialize voice session
    let (tx, rx) = mpsc::channel();
    let mut voice = VoiceSession::new()
        .map_err(|e| format!("Failed to initialize voice: {}", e))?;

    voice.on_event(move |event| {
        let _ = tx.send(event);
    });

    tracing::info!("Voice session initialized, waiting for model to load...");

    // Wait for model to be ready
    while voice.is_loading() {
        if let Ok(event) = rx.try_recv() {
            match event {
                VoiceEvent::ModelReady => {
                    tracing::info!("Voice model ready!");
                    break;
                }
                VoiceEvent::ModelError(e) => {
                    return Err(format!("Failed to load voice model: {}", e));
                }
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Set up recording state
    let recording = Arc::new(AtomicBool::new(false));

    // Set up hotkey listener
    let mut hotkey = HotkeyListener::new()
        .map_err(|e| format!("Failed to initialize hotkey listener: {}", e))?;

    // Channel for hotkey events
    let (hotkey_tx, hotkey_rx) = mpsc::channel::<bool>();

    // Start hotkey listener in background
    let hotkey_tx_press = hotkey_tx.clone();
    let hotkey_tx_release = hotkey_tx;

    hotkey.listen_right_command(
        move || {
            let _ = hotkey_tx_press.send(true); // pressed
        },
        move || {
            let _ = hotkey_tx_release.send(false); // released
        },
    ).map_err(|e| format!("Failed to start hotkey listener: {}", e))?;

    tracing::info!("Voice daemon ready! Hold Right Command to record.");

    // Main event loop
    loop {
        // Check for hotkey events
        if let Ok(pressed) = hotkey_rx.try_recv() {
            if pressed {
                // Start recording
                if !recording.load(Ordering::SeqCst) {
                    tracing::info!("Starting recording...");
                    if let Err(e) = voice.start_recording() {
                        tracing::error!("Failed to start recording: {}", e);
                    } else {
                        recording.store(true, Ordering::SeqCst);
                    }
                }
            } else {
                // Stop recording
                if recording.load(Ordering::SeqCst) {
                    tracing::info!("Stopping recording...");
                    if let Err(e) = voice.stop_recording() {
                        tracing::error!("Failed to stop recording: {}", e);
                    }
                    recording.store(false, Ordering::SeqCst);
                }
            }
        }

        // Check for voice events
        if let Ok(event) = rx.try_recv() {
            match event {
                VoiceEvent::TranscriptionStarted => {
                    tracing::info!("Transcribing...");
                }
                VoiceEvent::TranscriptionComplete { text } => {
                    // Filter out Whisper artifacts like [BLANK_AUDIO], (silence), etc.
                    let cleaned = text.trim();
                    let is_artifact = cleaned.is_empty()
                        || cleaned.starts_with('[')
                        || cleaned.starts_with('(')
                        || cleaned.to_lowercase().contains("blank")
                        || cleaned.to_lowercase().contains("silence");

                    if !is_artifact {
                        tracing::info!("Transcribed: {}", cleaned);
                        if let Err(e) = insert_text(cleaned) {
                            tracing::error!("Failed to insert text: {}", e);
                        }
                    } else {
                        tracing::debug!("Filtered out artifact: {}", text);
                    }
                }
                VoiceEvent::TranscriptionError(e) => {
                    tracing::error!("Transcription error: {}", e);
                }
                VoiceEvent::RecordingDiscarded { reason } => {
                    tracing::info!("Recording discarded: {}", reason);
                }
                _ => {}
            }
        }

        // Small sleep to prevent busy loop
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
