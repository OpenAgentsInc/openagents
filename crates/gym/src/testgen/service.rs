//! TestGen Service - Background test generation with UI updates
//!
//! Bridges the testgen crate's async generation to GPUI's UI update model.

use fm_bridge::FMClient;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use testgen::{
    EnvironmentInfo, GeneratedTest, IterationConfig, ReflectionEntry, TestCategory,
    TestGenContext, TestGenEmitter, TestGenerator,
};
use tokio::sync::mpsc;

// ============================================================================
// Event Types
// ============================================================================

/// Events emitted during test generation
#[derive(Debug, Clone)]
pub enum TestGenEvent {
    /// Progress update
    Progress {
        phase: String,
        category: Option<TestCategory>,
        round: u32,
        status: String,
    },
    /// A new test was generated
    TestGenerated(GeneratedTest),
    /// Reflection entry
    Reflection(ReflectionEntry),
    /// Generation complete
    Complete {
        total_tests: u32,
        total_rounds: u32,
        duration_ms: u64,
    },
    /// Error occurred
    Error(String),
}

// ============================================================================
// Channel-based Emitter
// ============================================================================

/// Emitter that sends events through a channel
pub struct ChannelEmitter {
    sender: mpsc::UnboundedSender<TestGenEvent>,
}

impl ChannelEmitter {
    pub fn new(sender: mpsc::UnboundedSender<TestGenEvent>) -> Self {
        Self { sender }
    }
}

impl TestGenEmitter for ChannelEmitter {
    fn on_progress(&self, phase: &str, category: Option<TestCategory>, round: u32, status: &str) {
        let _ = self.sender.send(TestGenEvent::Progress {
            phase: phase.to_string(),
            category,
            round,
            status: status.to_string(),
        });
    }

    fn on_test(&self, test: &GeneratedTest) {
        let _ = self.sender.send(TestGenEvent::TestGenerated(test.clone()));
    }

    fn on_reflection(&self, entry: &ReflectionEntry) {
        let _ = self.sender.send(TestGenEvent::Reflection(entry.clone()));
    }

    fn on_complete(&self, total_tests: u32, total_rounds: u32, duration_ms: u64) {
        let _ = self.sender.send(TestGenEvent::Complete {
            total_tests,
            total_rounds,
            duration_ms,
        });
    }

    fn on_error(&self, error: &str) {
        let _ = self.sender.send(TestGenEvent::Error(error.to_string()));
    }
}

// ============================================================================
// Generation Request
// ============================================================================

/// Request to start test generation
#[derive(Debug, Clone)]
pub struct GenerationRequest {
    pub task_id: String,
    pub task_description: String,
    pub context: TestGenContext,
}

// ============================================================================
// TestGen Service
// ============================================================================

/// Service that manages background test generation
pub struct TestGenService {
    /// Channel to receive events from background task
    event_receiver: Arc<Mutex<Option<mpsc::UnboundedReceiver<TestGenEvent>>>>,
    /// Whether generation is in progress
    is_generating: Arc<Mutex<bool>>,
}

impl TestGenService {
    pub fn new() -> Self {
        Self {
            event_receiver: Arc::new(Mutex::new(None)),
            is_generating: Arc::new(Mutex::new(false)),
        }
    }

    /// Check if generation is in progress
    pub fn is_generating(&self) -> bool {
        *self.is_generating.lock()
    }

    /// Start test generation for a task
    /// Returns a receiver for events
    pub fn start_generation(
        &self,
        request: GenerationRequest,
    ) -> mpsc::UnboundedReceiver<TestGenEvent> {
        let (sender, receiver) = mpsc::unbounded_channel();

        // Mark as generating
        *self.is_generating.lock() = true;

        // Clone Arc for the spawned task
        let is_generating = Arc::clone(&self.is_generating);

        // Spawn background task
        std::thread::spawn(move || {
            // Create tokio runtime for async operations
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async {
                let result = run_generation(request, sender.clone()).await;

                if let Err(e) = result {
                    let _ = sender.send(TestGenEvent::Error(e.to_string()));
                }

                // Mark as not generating
                *is_generating.lock() = false;
            });
        });

        receiver
    }

    /// Poll for events (non-blocking)
    pub fn poll_events(&self) -> Vec<TestGenEvent> {
        let mut events = Vec::new();
        if let Some(ref mut receiver) = *self.event_receiver.lock() {
            while let Ok(event) = receiver.try_recv() {
                events.push(event);
            }
        }
        events
    }
}

impl Default for TestGenService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Generation Logic
// ============================================================================

async fn run_generation(
    request: GenerationRequest,
    sender: mpsc::UnboundedSender<TestGenEvent>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create FM client
    let client = FMClient::new();

    // Create generator with default config
    let config = IterationConfig {
        max_total_rounds: 8,
        target_total_tests: 20,
        ..Default::default()
    };
    let generator = TestGenerator::with_config(client, config);

    // Create emitter
    let emitter = ChannelEmitter::new(sender);

    // Create minimal environment info
    let environment = EnvironmentInfo::default();

    // Run generation
    let result = generator
        .generate_iteratively(
            &request.task_description,
            &request.task_id,
            &environment,
            request.context,
            &emitter,
        )
        .await?;

    // Save results to disk
    if let Err(e) = save_generation_result(&request.task_id, &result.tests).await {
        eprintln!("Warning: Failed to save generation results: {}", e);
    }

    Ok(())
}

// ============================================================================
// Persistence
// ============================================================================

/// Saved generation record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedGeneration {
    pub task_id: String,
    pub timestamp: String,
    pub total_tests: usize,
    pub tests: Vec<GeneratedTest>,
}

/// Get the data directory for TestGen results
pub fn get_data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "openagents", "commander")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
        .join("testgen")
}

/// Save generation results to disk
async fn save_generation_result(
    task_id: &str,
    tests: &[GeneratedTest],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data_dir = get_data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let filename = format!("{}_{}.json", task_id, timestamp);
    let filepath = data_dir.join(&filename);

    let record = SavedGeneration {
        task_id: task_id.to_string(),
        timestamp: timestamp.clone(),
        total_tests: tests.len(),
        tests: tests.to_vec(),
    };

    let json = serde_json::to_string_pretty(&record)?;
    std::fs::write(&filepath, json)?;

    // Also update "latest" symlink/file
    let latest_path = data_dir.join(format!("{}_latest.json", task_id));
    std::fs::write(&latest_path, serde_json::to_string_pretty(&record)?)?;

    Ok(())
}

/// Load the most recent generation for a task
pub fn load_latest_generation(task_id: &str) -> Option<SavedGeneration> {
    let data_dir = get_data_dir();
    let latest_path = data_dir.join(format!("{}_latest.json", task_id));

    if latest_path.exists() {
        std::fs::read_to_string(&latest_path)
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
    } else {
        None
    }
}

/// List all saved generations for a task
pub fn list_generations(task_id: &str) -> Vec<SavedGeneration> {
    let data_dir = get_data_dir();

    if !data_dir.exists() {
        return Vec::new();
    }

    let mut generations = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&data_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Match files like "regex-log_20241210_143000.json"
                if name.starts_with(task_id) && name.ends_with(".json") && !name.contains("latest")
                {
                    if let Ok(json) = std::fs::read_to_string(&path) {
                        if let Ok(saved) = serde_json::from_str::<SavedGeneration>(&json) {
                            generations.push(saved);
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp descending
    generations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    generations
}
