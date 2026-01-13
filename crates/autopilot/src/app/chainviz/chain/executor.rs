//! Chain executor for markdown summarization.

use super::signatures::{
    ContentSummarizerSignature, CuriosityGeneratorSignature, QuestionAnswererSignature,
    SummaryAggregatorSignature, TaskAnalysisSignature,
};
use super::{ChainEvent, ChainEventSender, ChainState};
use adjutant::dspy::{tool_step_utility_predict, LabeledToolCall, TrainingCollector};
use anyhow::{Context, Result};
use chrono::Utc;
use dsrs::predictors::Predict;
use dsrs::Predictor;
use glob::glob;
use hex::encode as hex_encode;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;
use walkdir::WalkDir;

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

/// Receipt emitted for a tool-based step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolReceipt {
    pub tool_name: String,
    pub inputs_hash: String,
    pub outputs_hash: String,
    pub latency_ms: u64,
    pub cost_tokens: u64,
    pub side_effects: Vec<String>,
}

/// Utility evaluation for a tool step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStepUtilityResult {
    pub step_utility: f32,
    pub should_continue: bool,
    pub next_action_hint: String,
    pub confidence: f32,
}

#[derive(Debug, Clone)]
struct ToolStepUtilityEvaluation {
    result: ToolStepUtilityResult,
    tokens: u64,
    cost_msats: u64,
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

/// Result from curiosity generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuriosityResult {
    pub question: String,
    pub search_patterns: Vec<String>,
    pub reasoning: String,
}

/// A match found during code search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeMatch {
    pub file_path: PathBuf,
    pub line_number: usize,
    pub matched_line: String,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
}

/// Result from code searching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSearchResult {
    pub matches: Vec<CodeMatch>,
    pub patterns_used: Vec<String>,
    pub files_searched: usize,
    pub receipt: ToolReceipt,
    pub utility: ToolStepUtilityResult,
    pub utility_tokens: u64,
    pub utility_cost_msats: u64,
}

/// Result from answering a question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerResult {
    pub answer: String,
    pub insights: Vec<String>,
    pub follow_up_topics: Vec<String>,
}

/// Final result including curiosity loop insights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalResult {
    pub aggregated: AggregatedResult,
    pub curiosity_insights: Vec<CuriosityInsight>,
}

/// A single curiosity loop iteration result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuriosityInsight {
    pub iteration: usize,
    pub question: String,
    pub answer: String,
    pub insights: Vec<String>,
}

/// Chain executor for markdown summarization.
pub struct MarkdownSummarizationChain {
    event_sender: ChainEventSender,
    chain_state: Arc<Mutex<ChainState>>,
    tool_call_history: Arc<Mutex<HashSet<String>>>,
}

impl MarkdownSummarizationChain {
    /// Create a new chain executor.
    pub fn new(
        event_sender: ChainEventSender,
        chain_state: Arc<Mutex<ChainState>>,
    ) -> Self {
        Self {
            event_sender,
            chain_state,
            tool_call_history: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Execute the full chain.
    pub async fn execute(&self, prompt: &str, repo_root: &Path) -> Result<FinalResult> {
        self.execute_with_curiosity(prompt, repo_root, 5).await
    }

    /// Execute the full chain with a configurable curiosity loop count.
    pub async fn execute_with_curiosity(
        &self,
        prompt: &str,
        repo_root: &Path,
        max_curiosity_iterations: usize,
    ) -> Result<FinalResult> {
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
        let aggregated = self.run_summary_aggregator(&summaries, prompt).await?;

        // Stage 6+: Curiosity Loop
        let curiosity_insights = self
            .run_curiosity_loop(&aggregated, repo_root, max_curiosity_iterations)
            .await?;

        Ok(FinalResult {
            aggregated,
            curiosity_insights,
        })
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
        self.event_sender.send(ChainEvent::NodeStarted {
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
        self.event_sender.send(ChainEvent::NodeCompleted {
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

        let inputs_value = json!({
            "pattern": pattern,
            "scope": scope,
            "glob_pattern": glob_pattern.to_string_lossy(),
        });
        let mut path_strings: Vec<String> =
            paths.iter().map(|p| p.display().to_string()).collect();
        path_strings.sort();
        let outputs_value = json!({
            "paths": path_strings,
            "count": count,
        });

        let receipt = build_tool_receipt(
            "FileDiscovery",
            &inputs_value,
            &outputs_value,
            duration,
            0,
            vec!["read_fs".to_string(), "scan_fs".to_string()],
        );
        tracing::info!(
            tool_name = receipt.tool_name,
            inputs_hash = receipt.inputs_hash,
            outputs_hash = receipt.outputs_hash,
            latency_ms = receipt.latency_ms,
            cost_tokens = receipt.cost_tokens,
            side_effects = ?receipt.side_effects,
            "tool receipt"
        );

        let utility = match self
            .evaluate_tool_step(
                "FileDiscovery",
                "Discover repository files matching the requested pattern",
                &summarize_json(&inputs_value, 600),
                &summarize_json(&outputs_value, 600),
                &receipt,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    tool_name = "FileDiscovery",
                    error = %error,
                    "tool step utility fallback"
                );
                fallback_tool_step_utility()
            }
        };
        self.record_tool_call_label(
            "FileDiscovery",
            &inputs_value,
            &outputs_value,
            &receipt,
            &utility,
        );

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
                ("receipt".to_string(), format_receipt_summary(&receipt)),
                (
                    "step_utility".to_string(),
                    format!("{:.2}", utility.result.step_utility),
                ),
                (
                    "should_continue".to_string(),
                    utility.result.should_continue.to_string(),
                ),
                (
                    "next_action_hint".to_string(),
                    utility.result.next_action_hint.clone(),
                ),
                (
                    "utility_confidence".to_string(),
                    format!("{:.2}", utility.result.confidence),
                ),
                ("utility_tokens".to_string(), utility.tokens.to_string()),
                (
                    "utility_cost_msats".to_string(),
                    utility.cost_msats.to_string(),
                ),
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
        let mut file_entries: Vec<(String, usize, String)> = Vec::new();
        let mut total_size = 0;

        for path in paths {
            match fs::read_to_string(path) {
                Ok(content) => {
                    let size = content.len();
                    total_size += size;
                    let path_string = path.display().to_string();
                    let content_hash = sha256_hex(content.as_bytes());
                    file_entries.push((path_string, size, content_hash));
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

        let mut input_paths: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
        input_paths.sort();
        let inputs_value = json!({
            "paths": input_paths,
        });

        file_entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut failed_path_strings: Vec<String> =
            failed_paths.iter().map(|p| p.display().to_string()).collect();
        failed_path_strings.sort();
        let outputs_value = json!({
            "files": file_entries
                .iter()
                .map(|(path, size, content_hash)| json!({
                    "path": path,
                    "size": size,
                    "content_hash": content_hash,
                }))
                .collect::<Vec<_>>(),
            "total_size": total_size,
            "failed_paths": failed_path_strings,
        });

        let receipt = build_tool_receipt(
            "ContentReader",
            &inputs_value,
            &outputs_value,
            duration,
            0,
            vec!["read_fs".to_string()],
        );
        tracing::info!(
            tool_name = receipt.tool_name,
            inputs_hash = receipt.inputs_hash,
            outputs_hash = receipt.outputs_hash,
            latency_ms = receipt.latency_ms,
            cost_tokens = receipt.cost_tokens,
            side_effects = ?receipt.side_effects,
            "tool receipt"
        );

        let utility = match self
            .evaluate_tool_step(
                "ContentReader",
                "Read file contents for downstream analysis",
                &summarize_json(&inputs_value, 600),
                &summarize_json(&outputs_value, 600),
                &receipt,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    tool_name = "ContentReader",
                    error = %error,
                    "tool step utility fallback"
                );
                fallback_tool_step_utility()
            }
        };
        self.record_tool_call_label(
            "ContentReader",
            &inputs_value,
            &outputs_value,
            &receipt,
            &utility,
        );

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
                ("receipt".to_string(), format_receipt_summary(&receipt)),
                (
                    "step_utility".to_string(),
                    format!("{:.2}", utility.result.step_utility),
                ),
                (
                    "should_continue".to_string(),
                    utility.result.should_continue.to_string(),
                ),
                (
                    "next_action_hint".to_string(),
                    utility.result.next_action_hint.clone(),
                ),
                (
                    "utility_confidence".to_string(),
                    format!("{:.2}", utility.result.confidence),
                ),
                ("utility_tokens".to_string(), utility.tokens.to_string()),
                (
                    "utility_cost_msats".to_string(),
                    utility.cost_msats.to_string(),
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
        self.event_sender.send(ChainEvent::NodeStarted {
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
        self.event_sender.send(ChainEvent::NodeCompleted {
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
        self.event_sender.send(ChainEvent::Progress {
            message: message.to_string(),
        });
    }

    fn record_tool_call_label(
        &self,
        signature: &str,
        inputs: &serde_json::Value,
        outputs: &serde_json::Value,
        receipt: &ToolReceipt,
        utility: &ToolStepUtilityEvaluation,
    ) {
        let call_hash = format!(
            "{}:{}:{}",
            signature, receipt.inputs_hash, receipt.outputs_hash
        );
        let was_repeated = if let Ok(mut history) = self.tool_call_history.lock() {
            let repeated = history.contains(&call_hash);
            history.insert(call_hash);
            repeated
        } else {
            false
        };

        let labeled = LabeledToolCall {
            signature: signature.to_string(),
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            step_utility: utility.result.step_utility,
            verification_delta: 0,
            cost_tokens: utility.tokens as u32,
            cost_tool_calls: 1,
            was_repeated,
            recorded_at: Utc::now(),
        };

        match TrainingCollector::new(false) {
            Ok(collector) => {
                if let Err(err) = collector.record_tool_call(labeled) {
                    tracing::warn!("Failed to record tool call label: {}", err);
                }
            }
            Err(err) => {
                tracing::warn!("Failed to init training collector: {}", err);
            }
        }
    }

    /// Evaluate utility for a tool-based step.
    async fn evaluate_tool_step(
        &self,
        tool_name: &str,
        step_goal: &str,
        inputs_summary: &str,
        outputs_summary: &str,
        receipt: &ToolReceipt,
    ) -> Result<ToolStepUtilityEvaluation> {
        self.send_progress(&format!("Scoring {} utility...", tool_name));
        let start = Instant::now();

        let predictor = tool_step_utility_predict();

        let receipt_json = serde_json::to_string(receipt)?;
        let inputs = dsrs::Example::new(
            HashMap::from([
                ("tool_name".to_string(), json!(tool_name)),
                ("step_goal".to_string(), json!(step_goal)),
                ("inputs_summary".to_string(), json!(inputs_summary)),
                ("outputs_summary".to_string(), json!(outputs_summary)),
                ("receipt".to_string(), json!(receipt_json)),
            ]),
            vec![
                "tool_name".to_string(),
                "step_goal".to_string(),
                "inputs_summary".to_string(),
                "outputs_summary".to_string(),
                "receipt".to_string(),
            ],
            vec![
                "step_utility".to_string(),
                "should_continue".to_string(),
                "next_action_hint".to_string(),
                "confidence".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let step_utility = parse_f32_value(prediction.data.get("step_utility"), 0.5)
            .clamp(0.0, 1.0);
        let should_continue =
            parse_bool_value(prediction.data.get("should_continue")).unwrap_or(true);
        let next_action_hint = parse_string_value(
            prediction.data.get("next_action_hint"),
            "Continue",
        );
        let confidence = parse_f32_value(prediction.data.get("confidence"), 0.5).clamp(0.0, 1.0);

        tracing::info!(
            tool_name,
            step_utility,
            should_continue,
            confidence,
            utility_latency_ms = duration,
            utility_tokens = prediction.lm_usage.total_tokens,
            utility_cost_msats = prediction.lm_usage.cost_msats,
            "tool step utility"
        );

        Ok(ToolStepUtilityEvaluation {
            result: ToolStepUtilityResult {
                step_utility,
                should_continue,
                next_action_hint,
                confidence,
            },
            tokens: prediction.lm_usage.total_tokens,
            cost_msats: prediction.lm_usage.cost_msats,
        })
    }

    /// Run the curiosity loop.
    async fn run_curiosity_loop(
        &self,
        aggregated: &AggregatedResult,
        repo_root: &Path,
        max_iterations: usize,
    ) -> Result<Vec<CuriosityInsight>> {
        let mut insights = Vec::new();
        let mut previous_questions: Vec<String> = Vec::new();

        for iteration in 0..max_iterations {
            eprintln!(
                "[chainviz] Curiosity loop iteration {}/{}",
                iteration + 1,
                max_iterations
            );

            // Add nodes for this iteration
            let (curiosity_id, search_id, answer_id) = {
                let mut state = self.chain_state.lock().unwrap();
                state.add_curiosity_iteration(iteration)
            };

            // Generate a question
            let curiosity = self
                .run_curiosity_generator(
                    &aggregated.final_summary,
                    &previous_questions,
                    iteration,
                    curiosity_id,
                )
                .await?;

            previous_questions.push(curiosity.question.clone());

            // Log the question
            eprintln!("\n[chainviz] ========== CURIOSITY Q{} ==========", iteration + 1);
            eprintln!("[chainviz] QUESTION: {}", curiosity.question);
            eprintln!("[chainviz] REASONING: {}", curiosity.reasoning);
            eprintln!("[chainviz] SEARCH PATTERNS: {:?}", curiosity.search_patterns);

            // Search the codebase
            let search = self
                .run_code_searcher(
                    &curiosity.question,
                    &curiosity.search_patterns,
                    repo_root,
                    search_id,
                )
                .await?;

            eprintln!("[chainviz] Found {} matches in {} files", search.matches.len(), search.files_searched);

            // Answer the question
            let answer = self
                .run_question_answerer(
                    &curiosity.question,
                    &search,
                    &aggregated.final_summary,
                    answer_id,
                )
                .await?;

            // Log the answer
            eprintln!("\n[chainviz] ANSWER: {}", answer.answer);
            if !answer.insights.is_empty() {
                eprintln!("[chainviz] INSIGHTS: {:?}", answer.insights);
            }
            eprintln!("[chainviz] =====================================\n");

            insights.push(CuriosityInsight {
                iteration,
                question: curiosity.question,
                answer: answer.answer,
                insights: answer.insights,
            });

            let counterfactual_continue = iteration + 1 < max_iterations;
            let should_continue = if search.utility.confidence >= 0.5 {
                search.utility.should_continue
            } else {
                counterfactual_continue
            };
            tracing::info!(
                iteration = iteration + 1,
                step_utility = search.utility.step_utility,
                utility_confidence = search.utility.confidence,
                utility_should_continue = search.utility.should_continue,
                counterfactual_continue,
                next_action_hint = search.utility.next_action_hint,
                "curiosity loop continuation decision"
            );

            if !should_continue {
                break;
            }
        }

        Ok(insights)
    }

    /// Generate a curiosity question (LLM).
    async fn run_curiosity_generator(
        &self,
        summary: &str,
        previous_questions: &[String],
        iteration: usize,
        call_id: Uuid,
    ) -> Result<CuriosityResult> {
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            if let Some(&idx) = state.call_id_to_node.get(&call_id) {
                state.nodes[idx].state = crate::app::chainviz::components::NodeState::Running;
                state.nodes[idx].progress_message = Some("Generating question...".to_string());
            }
        }

        let signature = CuriosityGeneratorSignature::new();
        let predictor = Predict::new(signature);

        let previous_json = serde_json::to_string(previous_questions)?;

        let inputs = dsrs::Example::new(
            HashMap::from([
                ("summary".to_string(), json!(summary)),
                ("previous_questions".to_string(), json!(previous_json)),
                ("iteration".to_string(), json!(iteration)),
            ]),
            vec![
                "summary".to_string(),
                "previous_questions".to_string(),
                "iteration".to_string(),
            ],
            vec![
                "question".to_string(),
                "search_patterns".to_string(),
                "reasoning".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let question = prediction
            .data
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("What patterns are used in this codebase?")
            .to_string();

        let search_patterns: Vec<String> = prediction
            .data
            .get("search_patterns")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_else(|| vec!["fn ".to_string(), "struct ".to_string()]);

        let reasoning = prediction
            .data
            .get("reasoning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Complete the node
        {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([("iteration".to_string(), format!("{}", iteration + 1))]);
            let outputs = HashMap::from([
                ("question".to_string(), question.clone()),
                (
                    "patterns".to_string(),
                    format!("{} patterns", search_patterns.len()),
                ),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(CuriosityResult {
            question,
            search_patterns,
            reasoning,
        })
    }

    /// Search the codebase for patterns (tool-based).
    async fn run_code_searcher(
        &self,
        question: &str,
        patterns: &[String],
        repo_root: &Path,
        call_id: Uuid,
    ) -> Result<CodeSearchResult> {
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            if let Some(&idx) = state.call_id_to_node.get(&call_id) {
                state.nodes[idx].state = crate::app::chainviz::components::NodeState::Running;
                state.nodes[idx].progress_message = Some("Searching codebase...".to_string());
            }
        }

        let mut all_matches = Vec::new();
        let mut files_searched = 0;

        // Walk the directory tree
        for entry in WalkDir::new(repo_root)
            .into_iter()
            .filter_entry(|e| !is_hidden(e) && !is_build_dir(e))
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();

            // Skip binary files and very large files
            if !is_text_file(path) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(path) {
                files_searched += 1;

                // Search for each pattern
                for pattern in patterns {
                    // Try regex first, fall back to literal search
                    let matches = if let Ok(re) = Regex::new(pattern) {
                        find_regex_matches(&content, &re, path)
                    } else {
                        find_literal_matches(&content, pattern, path)
                    };

                    all_matches.extend(matches);
                }
            }
        }

        // Limit matches to prevent overwhelming the LLM
        all_matches.truncate(20);

        let duration = start.elapsed().as_millis() as u64;

        let receipt_inputs = json!({
            "patterns": patterns,
            "repo_root": repo_root.display().to_string(),
        });
        let receipt_outputs = json!({
            "matches": all_matches
                .iter()
                .map(|m| {
                    json!({
                        "path": m.file_path.display().to_string(),
                        "line_number": m.line_number,
                        "matched_line": m.matched_line,
                        "context_before": m.context_before,
                        "context_after": m.context_after,
                    })
                })
                .collect::<Vec<_>>(),
            "files_searched": files_searched,
        });

        let receipt = build_tool_receipt(
            "CodeSearch",
            &receipt_inputs,
            &receipt_outputs,
            duration,
            0,
            vec![
                "read_fs".to_string(),
                "scan_fs".to_string(),
                "regex_search".to_string(),
            ],
        );
        tracing::info!(
            tool_name = receipt.tool_name,
            inputs_hash = receipt.inputs_hash,
            outputs_hash = receipt.outputs_hash,
            latency_ms = receipt.latency_ms,
            cost_tokens = receipt.cost_tokens,
            side_effects = ?receipt.side_effects,
            "tool receipt"
        );

        let utility_inputs = json!({
            "question": question,
            "patterns": patterns,
            "repo_root": repo_root.display().to_string(),
        });
        let utility = match self
            .evaluate_tool_step(
                "CodeSearch",
                "Search the codebase for evidence to answer the question",
                &summarize_json(&utility_inputs, 600),
                &summarize_json(&receipt_outputs, 600),
                &receipt,
            )
            .await
        {
            Ok(result) => result,
            Err(error) => {
                tracing::warn!(
                    tool_name = "CodeSearch",
                    error = %error,
                    "tool step utility fallback"
                );
                fallback_tool_step_utility()
            }
        };
        self.record_tool_call_label(
            "CodeSearch",
            &receipt_inputs,
            &receipt_outputs,
            &receipt,
            &utility,
        );

        // Complete the node
        {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([(
                "patterns".to_string(),
                format!("{} patterns", patterns.len()),
            )]);
            let outputs = HashMap::from([
                ("matches".to_string(), format!("{} found", all_matches.len())),
                ("files".to_string(), format!("{} searched", files_searched)),
                ("receipt".to_string(), format_receipt_summary(&receipt)),
                (
                    "step_utility".to_string(),
                    format!("{:.2}", utility.result.step_utility),
                ),
                (
                    "should_continue".to_string(),
                    utility.result.should_continue.to_string(),
                ),
                (
                    "next_action_hint".to_string(),
                    utility.result.next_action_hint.clone(),
                ),
                (
                    "utility_confidence".to_string(),
                    format!("{:.2}", utility.result.confidence),
                ),
                ("utility_tokens".to_string(), utility.tokens.to_string()),
                (
                    "utility_cost_msats".to_string(),
                    utility.cost_msats.to_string(),
                ),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(CodeSearchResult {
            matches: all_matches,
            patterns_used: patterns.to_vec(),
            files_searched,
            receipt,
            utility: utility.result,
            utility_tokens: utility.tokens,
            utility_cost_msats: utility.cost_msats,
        })
    }

    /// Answer a question based on code snippets (LLM).
    async fn run_question_answerer(
        &self,
        question: &str,
        search_result: &CodeSearchResult,
        context: &str,
        call_id: Uuid,
    ) -> Result<AnswerResult> {
        let start = Instant::now();

        // Start the node
        {
            let mut state = self.chain_state.lock().unwrap();
            if let Some(&idx) = state.call_id_to_node.get(&call_id) {
                state.nodes[idx].state = crate::app::chainviz::components::NodeState::Running;
                state.nodes[idx].progress_message = Some("Analyzing code...".to_string());
            }
        }

        let signature = QuestionAnswererSignature::new();
        let predictor = Predict::new(signature);

        // Format code snippets for the LLM
        let snippets = format_code_snippets(&search_result.matches);

        let inputs = dsrs::Example::new(
            HashMap::from([
                ("question".to_string(), json!(question)),
                ("code_snippets".to_string(), json!(snippets)),
                ("context".to_string(), json!(context)),
            ]),
            vec![
                "question".to_string(),
                "code_snippets".to_string(),
                "context".to_string(),
            ],
            vec![
                "answer".to_string(),
                "insights".to_string(),
                "follow_up_topics".to_string(),
            ],
        );

        let prediction = predictor.forward(inputs).await?;
        let duration = start.elapsed().as_millis() as u64;

        let answer = prediction
            .data
            .get("answer")
            .and_then(|v| v.as_str())
            .unwrap_or("Unable to determine answer from the code snippets.")
            .to_string();

        let insights: Vec<String> = prediction
            .data
            .get("insights")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let follow_up_topics: Vec<String> = prediction
            .data
            .get("follow_up_topics")
            .and_then(|v| {
                if let Some(s) = v.as_str() {
                    serde_json::from_str(s).ok()
                } else if let Some(arr) = v.as_array() {
                    Some(
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect(),
                    )
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // Complete the node
        {
            let mut state = self.chain_state.lock().unwrap();
            let inputs = HashMap::from([("question".to_string(), question.to_string())]);
            let outputs = HashMap::from([
                ("answer".to_string(), answer.clone()),
                ("insights".to_string(), format!("{} insights", insights.len())),
            ]);
            state.complete_tool_node(call_id, inputs, outputs, duration);
        }

        Ok(AnswerResult {
            answer,
            insights,
            follow_up_topics,
        })
    }
}

/// Check if a directory entry is hidden.
fn is_hidden(entry: &walkdir::DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Check if a directory is a build directory to skip.
fn is_build_dir(entry: &walkdir::DirEntry) -> bool {
    let name = entry.file_name().to_str().unwrap_or("");
    matches!(name, "target" | "node_modules" | "dist" | "build" | ".git")
}

/// Check if a file is likely a text file.
fn is_text_file(path: &Path) -> bool {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    matches!(
        ext,
        "rs" | "py"
            | "js"
            | "ts"
            | "jsx"
            | "tsx"
            | "md"
            | "txt"
            | "json"
            | "yaml"
            | "yml"
            | "toml"
            | "html"
            | "css"
            | "sql"
            | "sh"
            | "go"
            | "java"
            | "c"
            | "cpp"
            | "h"
            | "hpp"
    )
}

/// Find matches using a regex pattern.
fn find_regex_matches(content: &str, re: &Regex, path: &Path) -> Vec<CodeMatch> {
    let lines: Vec<&str> = content.lines().collect();
    let mut matches = Vec::new();

    for (line_num, line) in lines.iter().enumerate() {
        if re.is_match(line) {
            let context_before: Vec<String> = lines
                .iter()
                .skip(line_num.saturating_sub(2))
                .take(2.min(line_num))
                .map(|s| s.to_string())
                .collect();

            let context_after: Vec<String> = lines
                .iter()
                .skip(line_num + 1)
                .take(2)
                .map(|s| s.to_string())
                .collect();

            matches.push(CodeMatch {
                file_path: path.to_path_buf(),
                line_number: line_num + 1,
                matched_line: line.to_string(),
                context_before,
                context_after,
            });
        }
    }

    matches
}

/// Find matches using literal string search.
fn find_literal_matches(content: &str, pattern: &str, path: &Path) -> Vec<CodeMatch> {
    let lines: Vec<&str> = content.lines().collect();
    let mut matches = Vec::new();

    for (line_num, line) in lines.iter().enumerate() {
        if line.contains(pattern) {
            let context_before: Vec<String> = lines
                .iter()
                .skip(line_num.saturating_sub(2))
                .take(2.min(line_num))
                .map(|s| s.to_string())
                .collect();

            let context_after: Vec<String> = lines
                .iter()
                .skip(line_num + 1)
                .take(2)
                .map(|s| s.to_string())
                .collect();

            matches.push(CodeMatch {
                file_path: path.to_path_buf(),
                line_number: line_num + 1,
                matched_line: line.to_string(),
                context_before,
                context_after,
            });
        }
    }

    matches
}

/// Format code matches for LLM consumption.
fn format_code_snippets(matches: &[CodeMatch]) -> String {
    let mut output = String::new();

    for m in matches {
        output.push_str(&format!(
            "\n--- {}:{} ---\n",
            m.file_path.display(),
            m.line_number
        ));

        for line in &m.context_before {
            output.push_str(&format!("  {}\n", line));
        }
        output.push_str(&format!("> {}\n", m.matched_line));
        for line in &m.context_after {
            output.push_str(&format!("  {}\n", line));
        }
    }

    output
}

fn fallback_tool_step_utility() -> ToolStepUtilityEvaluation {
    ToolStepUtilityEvaluation {
        result: ToolStepUtilityResult {
            step_utility: 0.5,
            should_continue: true,
            next_action_hint: "Continue".to_string(),
            confidence: 0.2,
        },
        tokens: 0,
        cost_msats: 0,
    }
}

fn build_tool_receipt(
    tool_name: &str,
    inputs: &serde_json::Value,
    outputs: &serde_json::Value,
    latency_ms: u64,
    cost_tokens: u64,
    side_effects: Vec<String>,
) -> ToolReceipt {
    ToolReceipt {
        tool_name: tool_name.to_string(),
        inputs_hash: hash_json(inputs),
        outputs_hash: hash_json(outputs),
        latency_ms,
        cost_tokens,
        side_effects,
    }
}

fn format_receipt_summary(receipt: &ToolReceipt) -> String {
    format!(
        "inputs={}, outputs={}, latency={}ms, cost_tokens={}, effects=[{}]",
        receipt.inputs_hash,
        receipt.outputs_hash,
        receipt.latency_ms,
        receipt.cost_tokens,
        receipt.side_effects.join(", ")
    )
}

fn hash_json(value: &serde_json::Value) -> String {
    let encoded = value.to_string();
    sha256_hex(encoded.as_bytes())
}

fn sha256_hex(data: impl AsRef<[u8]>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_ref());
    hex_encode(hasher.finalize())
}

fn summarize_json(value: &serde_json::Value, max_len: usize) -> String {
    let text = value.to_string();
    if text.len() <= max_len {
        return text;
    }
    let truncated: String = text.chars().take(max_len).collect();
    format!("{}...", truncated)
}

fn parse_f32_value(value: Option<&serde_json::Value>, default: f32) -> f32 {
    value
        .and_then(|val| {
            if let Some(n) = val.as_f64() {
                Some(n as f32)
            } else if let Some(s) = val.as_str() {
                s.parse::<f32>().ok()
            } else {
                None
            }
        })
        .unwrap_or(default)
}

fn parse_bool_value(value: Option<&serde_json::Value>) -> Option<bool> {
    match value {
        Some(serde_json::Value::Bool(val)) => Some(*val),
        Some(serde_json::Value::String(val)) => match val.trim().to_lowercase().as_str() {
            "true" | "yes" | "y" => Some(true),
            "false" | "no" | "n" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn parse_string_value(value: Option<&serde_json::Value>, default: &str) -> String {
    match value {
        Some(val) => val
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| val.to_string()),
        None => default.to_string(),
    }
}
