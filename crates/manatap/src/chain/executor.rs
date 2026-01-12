//! Chain executor for markdown summarization.

use super::signatures::{
    ContentSummarizerSignature, SummaryAggregatorSignature, TaskAnalysisSignature,
};
use super::{ChainEvent, ChainState};
use anyhow::{Context, Result};
use dsrs::predictors::Predict;
use dsrs::Predictor;
use glob::glob;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Result from task analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAnalysisResult {
    pub task_type: String,
    pub file_pattern: String,
    pub scope: String,
    pub output_action: String,
    pub confidence: f32,
}

/// Result from file discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiscoveryResult {
    pub paths: Vec<PathBuf>,
    pub count: usize,
}

/// Result from content reading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentReadResult {
    pub files: Vec<FileContent>,
    pub total_size: usize,
    pub failed_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
    pub path: PathBuf,
    pub content: String,
    pub size: usize,
}

/// Result from content summarization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSummaryResult {
    pub filename: String,
    pub summary: String,
    pub key_points: Vec<String>,
    pub topic: String,
}

/// Final aggregated result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedResult {
    pub final_summary: String,
    pub themes: Vec<String>,
    pub key_takeaways: Vec<String>,
    pub suggestions: Vec<String>,
}

/// Chain executor for markdown summarization.
pub struct MarkdownSummarizationChain {
    event_sender: mpsc::UnboundedSender<ChainEvent>,
    chain_state: Arc<Mutex<ChainState>>,
}

impl MarkdownSummarizationChain {
    /// Create a new chain executor.
    pub fn new(
        event_sender: mpsc::UnboundedSender<ChainEvent>,
        chain_state: Arc<Mutex<ChainState>>,
    ) -> Self {
        Self {
            event_sender,
            chain_state,
        }
    }

    /// Execute the full chain.
    pub async fn execute(&self, prompt: &str, repo_root: &Path) -> Result<AggregatedResult> {
        // Stage 1: Task Analysis (LLM)
        let task = self.run_task_analysis(prompt).await?;

        // Stage 2: File Discovery (tool-based)
        let files = self
            .run_file_discovery(&task.file_pattern, &task.scope, repo_root)
            .await?;

        // Stage 3: Content Reading (tool-based)
        let contents = self.run_content_reader(&files.paths).await?;

        // Stage 4: Content Summarization (LLM, per file)
        let mut summaries = Vec::new();
        for (i, file) in contents.files.iter().enumerate() {
            self.send_progress(&format!(
                "Summarizing {} ({}/{})...",
                file.path.file_name().unwrap_or_default().to_string_lossy(),
                i + 1,
                contents.files.len()
            ));
            let summary = self
                .run_content_summarizer(&file.content, &file.path.to_string_lossy(), i, contents.files.len())
                .await?;
            summaries.push(summary);
        }

        // Stage 5: Summary Aggregation (LLM)
        let result = self.run_summary_aggregator(&summaries, prompt).await?;

        Ok(result)
    }

    /// Stage 1: Analyze the user's task.
    async fn run_task_analysis(&self, prompt: &str) -> Result<TaskAnalysisResult> {
        let call_id = Uuid::new_v4();
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            state.start_tool_node("TaskAnalysis", call_id);
        }
        let _ = self.event_sender.send(ChainEvent::NodeStarted {
            call_id,
            signature_name: "TaskAnalysis".to_string(),
            inputs: HashMap::from([("prompt".to_string(), prompt.to_string())]),
        });

        let signature = TaskAnalysisSignature::new();
        let predictor = Predict::new(signature);

        let inputs = dsrs::Example::new(
            HashMap::from([("prompt".to_string(), json!(prompt))]),
            vec!["prompt".to_string()],
            vec![
                "task_type".to_string(),
                "file_pattern".to_string(),
                "scope".to_string(),
                "output_action".to_string(),
                "confidence".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let task_type = prediction
            .data
            .get("task_type")
            .and_then(|v| v.as_str())
            .unwrap_or("summarize")
            .to_string();

        let file_pattern = prediction
            .data
            .get("file_pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("*.md")
            .to_string();

        let scope = prediction
            .data
            .get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("root")
            .to_string();

        let output_action = prediction
            .data
            .get("output_action")
            .and_then(|v| v.as_str())
            .unwrap_or("summarize")
            .to_string();

        let confidence = prediction
            .data
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.8) as f32;

        // Complete the node
        let outputs = HashMap::from([
            ("task_type".to_string(), task_type.clone()),
            ("file_pattern".to_string(), file_pattern.clone()),
            ("scope".to_string(), scope.clone()),
            ("confidence".to_string(), format!("{:.2}", confidence)),
        ]);
        {
            let mut state = self.chain_state.lock().unwrap();
            state.complete_tool_node(
                call_id,
                HashMap::from([("prompt".to_string(), prompt.to_string())]),
                outputs.clone(),
                duration,
            );
        }
        let _ = self.event_sender.send(ChainEvent::NodeCompleted {
            call_id,
            outputs,
            tokens: prediction.lm_usage.total_tokens as u32,
            cost_msats: prediction.lm_usage.cost_msats,
            duration_ms: duration,
        });

        Ok(TaskAnalysisResult {
            task_type,
            file_pattern,
            scope,
            output_action,
            confidence,
        })
    }

    /// Stage 2: Discover files matching the pattern (tool-based).
    async fn run_file_discovery(
        &self,
        pattern: &str,
        scope: &str,
        repo_root: &Path,
    ) -> Result<FileDiscoveryResult> {
        let call_id = Uuid::new_v4();
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            state.start_tool_node("FileDiscovery", call_id);
        }

        self.send_progress(&format!("Discovering files matching {}...", pattern));

        // Build the glob pattern based on scope
        let glob_pattern = if scope == "root" {
            repo_root.join(pattern)
        } else if scope == "recursive" {
            repo_root.join("**").join(pattern)
        } else {
            repo_root.join(scope).join(pattern)
        };

        let paths: Vec<PathBuf> = glob(glob_pattern.to_str().unwrap_or("*.md"))
            .context("Invalid glob pattern")?
            .filter_map(Result::ok)
            .filter(|p| p.is_file())
            .collect();

        let count = paths.len();
        let duration = start.elapsed().as_millis() as u64;

        // Complete the node
        {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([
                ("pattern".to_string(), pattern.to_string()),
                ("scope".to_string(), scope.to_string()),
            ]);
            let outputs = HashMap::from([
                (
                    "paths".to_string(),
                    format!(
                        "[{}]",
                        paths
                            .iter()
                            .take(3)
                            .map(|p| format!("\"{}\"", p.file_name().unwrap_or_default().to_string_lossy()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ) + if paths.len() > 3 { "..." } else { "" },
                ),
                ("count".to_string(), count.to_string()),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(FileDiscoveryResult { paths, count })
    }

    /// Stage 3: Read file contents (tool-based).
    async fn run_content_reader(&self, paths: &[PathBuf]) -> Result<ContentReadResult> {
        let call_id = Uuid::new_v4();
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            state.start_tool_node("ContentReader", call_id);
        }

        self.send_progress(&format!("Reading {} files...", paths.len()));

        let mut files = Vec::new();
        let mut failed_paths = Vec::new();
        let mut total_size = 0;

        for path in paths {
            match fs::read_to_string(path) {
                Ok(content) => {
                    let size = content.len();
                    total_size += size;
                    files.push(FileContent {
                        path: path.clone(),
                        content,
                        size,
                    });
                }
                Err(_) => {
                    failed_paths.push(path.clone());
                }
            }
        }

        let duration = start.elapsed().as_millis() as u64;

        // Complete the node
        {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([(
                "paths".to_string(),
                format!("[{} files]", paths.len()),
            )]);
            let outputs = HashMap::from([
                ("total_size".to_string(), total_size.to_string()),
                (
                    "failed_paths".to_string(),
                    format!("[{} failed]", failed_paths.len()),
                ),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(ContentReadResult {
            files,
            total_size,
            failed_paths,
        })
    }

    /// Stage 4: Summarize a single file (LLM).
    async fn run_content_summarizer(
        &self,
        content: &str,
        filename: &str,
        file_index: usize,
        total_files: usize,
    ) -> Result<ContentSummaryResult> {
        let call_id = Uuid::new_v4();
        let start = Instant::now();

        // Only update UI for first file to avoid node flickering
        if file_index == 0 {
            let mut state = self.chain_state.lock().unwrap();
            state.start_tool_node("ContentSummarizer", call_id);
        }

        // Update progress
        {
            let mut state = self.chain_state.lock().unwrap();
            if let Some(node) = state.nodes.iter_mut().find(|n| n.name == "ContentSummarizer") {
                node.progress_message = Some(format!("Processing {} ({}/{})...",
                    PathBuf::from(filename).file_name().unwrap_or_default().to_string_lossy(),
                    file_index + 1,
                    total_files
                ));
            }
        }

        let signature = ContentSummarizerSignature::new();
        let predictor = Predict::new(signature);

        // Truncate content if too long
        let truncated_content = if content.len() > 8000 {
            format!("{}...[truncated]", &content[..8000])
        } else {
            content.to_string()
        };

        let inputs = dsrs::Example::new(
            HashMap::from([
                ("content".to_string(), json!(truncated_content)),
                ("filename".to_string(), json!(filename)),
                ("content_type".to_string(), json!("markdown")),
            ]),
            vec![
                "content".to_string(),
                "filename".to_string(),
                "content_type".to_string(),
            ],
            vec![
                "summary".to_string(),
                "key_points".to_string(),
                "topic".to_string(),
                "sections".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let summary = prediction
            .data
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("No summary generated")
            .to_string();

        let key_points: Vec<String> = prediction
            .data
            .get("key_points")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let topic = prediction
            .data
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("General")
            .to_string();

        // Complete node after last file
        if file_index == total_files - 1 {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([
                ("files".to_string(), format!("{} files", total_files)),
            ]);
            let outputs = HashMap::from([
                ("summaries".to_string(), format!("{} generated", total_files)),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(ContentSummaryResult {
            filename: filename.to_string(),
            summary,
            key_points,
            topic,
        })
    }

    /// Stage 5: Aggregate all summaries (LLM).
    async fn run_summary_aggregator(
        &self,
        summaries: &[ContentSummaryResult],
        original_request: &str,
    ) -> Result<AggregatedResult> {
        let call_id = Uuid::new_v4();
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            state.start_tool_node("SummaryAggregator", call_id);
        }
        let _ = self.event_sender.send(ChainEvent::NodeStarted {
            call_id,
            signature_name: "SummaryAggregator".to_string(),
            inputs: HashMap::from([
                ("summaries".to_string(), format!("{} summaries", summaries.len())),
                ("original_request".to_string(), original_request.to_string()),
            ]),
        });

        let signature = SummaryAggregatorSignature::new();
        let predictor = Predict::new(signature);

        let summaries_json = serde_json::to_string(summaries)?;

        let inputs = dsrs::Example::new(
            HashMap::from([
                ("summaries".to_string(), json!(summaries_json)),
                ("original_request".to_string(), json!(original_request)),
            ]),
            vec!["summaries".to_string(), "original_request".to_string()],
            vec![
                "final_summary".to_string(),
                "themes".to_string(),
                "key_takeaways".to_string(),
                "suggestions".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let final_summary = prediction
            .data
            .get("final_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("No summary generated")
            .to_string();

        let themes: Vec<String> = prediction
            .data
            .get("themes")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let key_takeaways: Vec<String> = prediction
            .data
            .get("key_takeaways")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let suggestions: Vec<String> = prediction
            .data
            .get("suggestions")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Complete the node - show full summary without truncation
        let outputs = HashMap::from([
            ("final_summary".to_string(), final_summary.clone()),
            ("themes".to_string(), format!("{} themes", themes.len())),
        ]);
        {
            let mut state = self.chain_state.lock().unwrap();
            state.complete_tool_node(
                call_id,
                HashMap::from([("summaries".to_string(), format!("{} files", summaries.len()))]),
                outputs.clone(),
                duration,
            );
        }
        let _ = self.event_sender.send(ChainEvent::NodeCompleted {
            call_id,
            outputs,
            tokens: prediction.lm_usage.total_tokens as u32,
            cost_msats: prediction.lm_usage.cost_msats,
            duration_ms: duration,
        });

        Ok(AggregatedResult {
            final_summary,
            themes,
            key_takeaways,
            suggestions,
        })
    }

    /// Send a progress event to the UI.
    fn send_progress(&self, message: &str) {
        let _ = self.event_sender.send(ChainEvent::Progress {
            message: message.to_string(),
        });
    }
}

/// Truncate a string for display.
fn truncate_for_display(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
