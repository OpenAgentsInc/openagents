//! Chain execution and visualization state management.

pub mod executor;
pub mod signatures;

pub use executor::MarkdownSummarizationChain;

use crate::components::{ChainNode, NodeState};
use anyhow::Error;
use dsrs::callbacks::DspyCallback;
use dsrs::data::{Example, Prediction};
use dsrs::trace::Graph;
use dsrs::{CallbackEvent, LmUsage};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Events sent from the chain executor to the UI.
#[derive(Debug, Clone)]
pub enum ChainEvent {
    /// A signature node has started execution.
    NodeStarted {
        call_id: Uuid,
        signature_name: String,
        inputs: HashMap<String, String>,
    },
    /// A signature node has completed successfully.
    NodeCompleted {
        call_id: Uuid,
        outputs: HashMap<String, String>,
        tokens: u32,
        cost_msats: u64,
        duration_ms: u64,
    },
    /// A signature node has failed.
    NodeFailed { call_id: Uuid, error: String },
    /// A token was received during streaming.
    TokenReceived { call_id: Uuid, token: String },
    /// Progress update for tool-based operations.
    Progress { message: String },
}

/// Manages the chain visualization state.
pub struct ChainState {
    /// The nodes in the chain.
    pub nodes: Vec<ChainNode>,
    /// Map from call_id to node index.
    pub call_id_to_node: HashMap<Uuid, usize>,
    /// The user prompt.
    pub prompt: String,
}

impl ChainState {
    /// Create a new chain state with the expected signatures.
    pub fn new(prompt: &str) -> Self {
        let nodes = vec![
            ChainNode::new("TaskAnalysis")
                .with_description("Parse user intent and extract task parameters"),
            ChainNode::new("FileDiscovery")
                .with_description("Find matching files using glob patterns"),
            ChainNode::new("ContentReader")
                .with_description("Read file contents from discovered paths"),
            ChainNode::new("ContentSummarizer")
                .with_description("Summarize each file's key points"),
            ChainNode::new("SummaryAggregator")
                .with_description("Combine summaries into final report"),
        ];

        Self {
            nodes,
            call_id_to_node: HashMap::new(),
            prompt: prompt.to_string(),
        }
    }

    /// Get the nodes for rendering.
    pub fn nodes(&self) -> &[ChainNode] {
        &self.nodes
    }

    /// Process a chain event and update state.
    pub fn handle_event(&mut self, event: ChainEvent) {
        match event {
            ChainEvent::NodeStarted {
                call_id,
                signature_name,
                inputs,
            } => {
                // Find the node by signature name
                if let Some(idx) = self
                    .nodes
                    .iter()
                    .position(|n| n.name == signature_name || n.name.contains(&signature_name))
                {
                    self.call_id_to_node.insert(call_id, idx);
                    let node = &mut self.nodes[idx];
                    node.state = NodeState::Running;
                    node.inputs = inputs
                        .into_iter()
                        .map(|(k, v)| (k, truncate_value(&v)))
                        .collect();
                    node.progress_message = Some("Processing...".to_string());
                }
            }
            ChainEvent::NodeCompleted {
                call_id,
                outputs,
                tokens,
                cost_msats,
                duration_ms,
            } => {
                if let Some(&idx) = self.call_id_to_node.get(&call_id) {
                    let node = &mut self.nodes[idx];
                    node.state = NodeState::Complete;
                    node.outputs = outputs
                        .into_iter()
                        .map(|(k, v)| (k, truncate_value(&v)))
                        .collect();
                    node.tokens = Some(tokens);
                    node.cost_msats = Some(cost_msats);
                    node.duration_ms = Some(duration_ms);
                    node.progress_message = None;
                }
            }
            ChainEvent::NodeFailed { call_id, error } => {
                if let Some(&idx) = self.call_id_to_node.get(&call_id) {
                    let node = &mut self.nodes[idx];
                    node.state = NodeState::Failed;
                    node.progress_message = Some(error);
                }
            }
            ChainEvent::TokenReceived { call_id, token: _ } => {
                // Could update a streaming display, for now just keep progress message
                if let Some(&idx) = self.call_id_to_node.get(&call_id) {
                    let node = &mut self.nodes[idx];
                    if node.state == NodeState::Running {
                        node.progress_message = Some("Generating...".to_string());
                    }
                }
            }
            ChainEvent::Progress { message } => {
                // Find the currently running node and update its progress
                if let Some(node) = self.nodes.iter_mut().find(|n| n.state == NodeState::Running) {
                    node.progress_message = Some(message);
                }
            }
        }
    }

    /// Start a tool-based node manually (for FileDiscovery, ContentReader).
    pub fn start_tool_node(&mut self, signature_name: &str, call_id: Uuid) {
        if let Some(idx) = self.nodes.iter().position(|n| n.name == signature_name) {
            self.call_id_to_node.insert(call_id, idx);
            let node = &mut self.nodes[idx];
            node.state = NodeState::Running;
            node.progress_message = Some("Processing...".to_string());
        }
    }

    /// Complete a tool-based node manually.
    pub fn complete_tool_node(
        &mut self,
        call_id: Uuid,
        inputs: HashMap<String, String>,
        outputs: HashMap<String, String>,
        duration_ms: u64,
    ) {
        if let Some(&idx) = self.call_id_to_node.get(&call_id) {
            let node = &mut self.nodes[idx];
            node.state = NodeState::Complete;
            node.inputs = inputs
                .into_iter()
                .map(|(k, v)| (k, truncate_value(&v)))
                .collect();
            node.outputs = outputs
                .into_iter()
                .map(|(k, v)| (k, truncate_value(&v)))
                .collect();
            node.tokens = Some(0); // Tool-based, no LLM tokens
            node.cost_msats = Some(0);
            node.duration_ms = Some(duration_ms);
            node.progress_message = None;
        }
    }

    /// Add nodes for a curiosity loop iteration.
    /// Returns (curiosity_id, search_id, answer_id) for tracking.
    pub fn add_curiosity_iteration(&mut self, iteration: usize) -> (Uuid, Uuid, Uuid) {
        let curiosity_id = Uuid::new_v4();
        let search_id = Uuid::new_v4();
        let answer_id = Uuid::new_v4();

        // Create the three nodes for this iteration
        let curiosity_node = ChainNode::new(&format!("Curiosity #{}", iteration + 1))
            .with_description("Generate a question about the code");
        let search_node = ChainNode::new(&format!("CodeSearch #{}", iteration + 1))
            .with_description("Search codebase for relevant code");
        let answer_node = ChainNode::new(&format!("Answer #{}", iteration + 1))
            .with_description("Answer the question from code");

        // Add curiosity node
        let curiosity_idx = self.nodes.len();
        self.nodes.push(curiosity_node);
        self.call_id_to_node.insert(curiosity_id, curiosity_idx);

        // Add search node
        let search_idx = self.nodes.len();
        self.nodes.push(search_node);
        self.call_id_to_node.insert(search_id, search_idx);

        // Add answer node
        let answer_idx = self.nodes.len();
        self.nodes.push(answer_node);
        self.call_id_to_node.insert(answer_id, answer_idx);

        (curiosity_id, search_id, answer_id)
    }
}

/// Keep full values for display (no truncation).
fn truncate_value(s: &str) -> String {
    s.to_string()
}

/// Callback that sends events to the UI.
pub struct VisualizerCallback {
    sender: mpsc::UnboundedSender<ChainEvent>,
    start_times: Mutex<HashMap<Uuid, Instant>>,
}

impl VisualizerCallback {
    /// Create a new visualizer callback.
    pub fn new(sender: mpsc::UnboundedSender<ChainEvent>) -> Self {
        Self {
            sender,
            start_times: Mutex::new(HashMap::new()),
        }
    }

    /// Send a progress event.
    pub fn send_progress(&self, message: &str) {
        let _ = self.sender.send(ChainEvent::Progress {
            message: message.to_string(),
        });
    }
}

impl DspyCallback for VisualizerCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        self.start_times
            .lock()
            .unwrap()
            .insert(call_id, Instant::now());

        let inputs_map: HashMap<String, String> = inputs
            .data
            .iter()
            .map(|(k, v)| {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                (k.clone(), s)
            })
            .collect();

        let _ = self.sender.send(ChainEvent::NodeStarted {
            call_id,
            signature_name: module_name.to_string(),
            inputs: inputs_map,
        });
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        let duration = self
            .start_times
            .lock()
            .unwrap()
            .remove(&call_id)
            .map(|t| t.elapsed().as_millis() as u64)
            .unwrap_or(0);

        match result {
            Ok(prediction) => {
                let outputs_map: HashMap<String, String> = prediction
                    .data
                    .iter()
                    .map(|(k, v)| {
                        let s = match v {
                            serde_json::Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        (k.clone(), s)
                    })
                    .collect();

                let _ = self.sender.send(ChainEvent::NodeCompleted {
                    call_id,
                    outputs: outputs_map,
                    tokens: prediction.lm_usage.total_tokens as u32,
                    cost_msats: prediction.lm_usage.cost_msats,
                    duration_ms: duration,
                });
            }
            Err(e) => {
                let _ = self.sender.send(ChainEvent::NodeFailed {
                    call_id,
                    error: e.to_string(),
                });
            }
        }
    }

    fn on_lm_start(&self, _call_id: Uuid, _model: &str, _prompt_tokens: usize) {}

    fn on_lm_end(&self, _call_id: Uuid, _result: Result<(), &Error>, _usage: &LmUsage) {}

    fn on_lm_stream_start(&self, _call_id: Uuid, _model: &str) {}

    fn on_lm_token(&self, call_id: Uuid, token: &str) {
        let _ = self.sender.send(ChainEvent::TokenReceived {
            call_id,
            token: token.to_string(),
        });
    }

    fn on_lm_stream_end(&self, _call_id: Uuid) {}

    fn on_optimizer_candidate(&self, _candidate_id: &str, _metrics: &HashMap<String, f32>) {}

    fn on_trace_complete(&self, _graph: &Graph, _manifest: Option<&dsrs::CompiledModuleManifest>) {}

    fn on_event(&self, event: CallbackEvent) {
        if let CallbackEvent::Progress { message, .. } = event {
            let _ = self.sender.send(ChainEvent::Progress { message });
        }
    }
}
