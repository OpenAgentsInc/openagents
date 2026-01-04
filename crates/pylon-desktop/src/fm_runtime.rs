//! Async FM Bridge runtime
//!
//! Bridges async FM client with synchronous winit event loop using channels.

use fm_bridge::{FMClient, CompletionOptions, FinishReason};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

/// Events sent from FM runtime to UI
#[derive(Debug)]
pub enum FmEvent {
    /// Connection established
    Connected {
        model_available: bool,
        latency_ms: u32,
    },
    /// Connection failed
    ConnectionFailed(String),
    /// First token received (includes TTFT)
    FirstToken {
        text: String,
        ttft_ms: u64,
    },
    /// Subsequent token received
    Token {
        text: String,
    },
    /// Stream completed
    StreamComplete,
    /// Stream error
    StreamError(String),
}

/// Commands sent from UI to FM runtime
#[derive(Debug)]
pub enum FmCommand {
    /// Check connection to FM Bridge
    Connect,
    /// Start streaming completion
    Stream { prompt: String },
}

/// FM Runtime handle for communication with background thread
pub struct FmRuntime {
    cmd_tx: mpsc::Sender<FmCommand>,
    pub event_rx: mpsc::Receiver<FmEvent>,
}

impl FmRuntime {
    /// Create new FM runtime with background thread
    pub fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel::<FmCommand>(32);
        let (event_tx, event_rx) = mpsc::channel::<FmEvent>(256);

        // Spawn background thread with tokio runtime
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            rt.block_on(run_fm_loop(cmd_rx, event_tx));
        });

        Self { cmd_tx, event_rx }
    }

    /// Request connection check
    pub fn connect(&self) {
        let _ = self.cmd_tx.try_send(FmCommand::Connect);
    }

    /// Start streaming a prompt
    pub fn stream(&self, prompt: String) {
        let _ = self.cmd_tx.try_send(FmCommand::Stream { prompt });
    }
}

impl Default for FmRuntime {
    fn default() -> Self {
        Self::new()
    }
}

/// Main async loop running in background thread
async fn run_fm_loop(mut cmd_rx: mpsc::Receiver<FmCommand>, event_tx: mpsc::Sender<FmEvent>) {
    // Create FM client
    let client = match FMClient::new() {
        Ok(c) => c,
        Err(e) => {
            let _ = event_tx.send(FmEvent::ConnectionFailed(e.to_string())).await;
            return;
        }
    };

    // Process commands
    while let Some(cmd) = cmd_rx.recv().await {
        match cmd {
            FmCommand::Connect => {
                handle_connect(&client, &event_tx).await;
            }
            FmCommand::Stream { prompt } => {
                handle_stream(&client, &event_tx, prompt).await;
            }
        }
    }
}

/// Handle connection check
async fn handle_connect(client: &FMClient, event_tx: &mpsc::Sender<FmEvent>) {
    let start = Instant::now();

    match client.health().await {
        Ok(healthy) => {
            let latency_ms = start.elapsed().as_millis() as u32;
            let _ = event_tx
                .send(FmEvent::Connected {
                    model_available: healthy,
                    latency_ms,
                })
                .await;
        }
        Err(e) => {
            let _ = event_tx
                .send(FmEvent::ConnectionFailed(e.to_string()))
                .await;
        }
    }
}

/// Handle streaming completion
async fn handle_stream(client: &FMClient, event_tx: &mpsc::Sender<FmEvent>, prompt: String) {
    let start = Instant::now();
    let mut first_token = true;

    // Start streaming
    let stream_result = client.stream(&prompt, Some(CompletionOptions {
        max_tokens: Some(1024),
        temperature: Some(0.7),
        ..Default::default()
    })).await;

    let stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.send(FmEvent::StreamError(e.to_string())).await;
            return;
        }
    };

    // Pin the stream for iteration
    tokio::pin!(stream);

    // Process stream chunks
    while let Some(chunk_result) = stream.next().await {
        match chunk_result {
            Ok(chunk) => {
                // Check for completion
                if chunk.finish_reason == Some(FinishReason::Stop) {
                    let _ = event_tx.send(FmEvent::StreamComplete).await;
                    break;
                }

                // Send token
                if !chunk.text.is_empty() {
                    if first_token {
                        let ttft_ms = start.elapsed().as_millis() as u64;
                        let _ = event_tx
                            .send(FmEvent::FirstToken {
                                text: chunk.text,
                                ttft_ms,
                            })
                            .await;
                        first_token = false;
                    } else {
                        let _ = event_tx.send(FmEvent::Token { text: chunk.text }).await;
                    }
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                let _ = event_tx.send(FmEvent::StreamError(err_msg)).await;
                break;
            }
        }
    }

    // Ensure we send complete if not already sent
    if first_token {
        // No tokens received, still complete
        let _ = event_tx.send(FmEvent::StreamComplete).await;
    }
}
