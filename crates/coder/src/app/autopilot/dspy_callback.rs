//! DSPy callback that sends events to the Coder UI.
//!
//! This module provides a callback implementation that bridges dsrs execution
//! events to the Coder UI, enabling real-time visibility into:
//! - Module execution lifecycle (start/end)
//! - LLM inference calls (model, tokens, cost)
//! - Optimizer progress (candidates, metrics)
//! - General events (progress, warnings, errors)

use anyhow::Error;
use dsrs::callbacks::{CallbackEvent, DspyCallback};
use dsrs::core::lm::LmUsage;
use dsrs::data::{Example, Prediction};
use dsrs::manifest::CompiledModuleManifest;
use dsrs::trace::Graph;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Callback that sends dsrs events to the UI via a channel.
///
/// Events are sent as formatted strings that get displayed in the chat stream.
/// The handler in `handler.rs` parses these for special markers like `<<DSPY_STAGE:...:DSPY_STAGE>>`.
pub struct UiDspyCallback {
    tx: mpsc::UnboundedSender<String>,
}

impl UiDspyCallback {
    /// Create a new UI callback that sends events to the given channel.
    pub fn new(tx: mpsc::UnboundedSender<String>) -> Self {
        Self { tx }
    }

    /// Send a message to the UI channel, ignoring errors if the channel is closed.
    fn send(&self, msg: String) {
        let _ = self.tx.send(msg);
    }
}

impl DspyCallback for UiDspyCallback {
    fn on_module_start(&self, _call_id: Uuid, module_name: &str, _inputs: &Example) {
        self.send(format!("\n**[{}]** starting...\n", module_name));
    }

    fn on_module_end(&self, _call_id: Uuid, result: Result<&Prediction, &Error>) {
        match result {
            Ok(_) => self.send("**[complete]**\n".to_string()),
            Err(e) => self.send(format!("**[failed]** {}\n", e)),
        }
    }

    fn on_lm_start(&self, _call_id: Uuid, model: &str, prompt_tokens: usize) {
        self.send(format!(
            "LLM call: `{}` ({} prompt tokens)...\n",
            model, prompt_tokens
        ));
    }

    fn on_lm_end(&self, _call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage) {
        match result {
            Ok(()) => {
                let cost_info = if usage.cost_msats > 0 {
                    format!(" ({} msats)", usage.cost_msats)
                } else {
                    String::new()
                };
                self.send(format!(
                    "LLM done: {} in → {} out{}\n",
                    usage.prompt_tokens, usage.completion_tokens, cost_info
                ));
            }
            Err(e) => {
                self.send(format!("LLM failed: {}\n", e));
            }
        }
    }

    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>) {
        // Format metrics as a concise summary
        let metrics_str: Vec<String> = metrics
            .iter()
            .map(|(k, v)| format!("{}={:.2}", k, v))
            .collect();
        self.send(format!(
            "Optimizer candidate `{}`: {}\n",
            candidate_id,
            metrics_str.join(", ")
        ));
    }

    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        let manifest_info = manifest
            .and_then(|m| m.compiled_id.as_ref())
            .map(|id| format!(", manifest: {}", &id[..8.min(id.len())]))
            .unwrap_or_default();
        self.send(format!(
            "Trace complete: {} nodes{}\n",
            graph.nodes.len(),
            manifest_info
        ));
    }

    fn on_event(&self, event: CallbackEvent) {
        match event {
            CallbackEvent::Progress {
                current,
                total,
                message,
            } => {
                self.send(format!("Progress: {}/{} - {}\n", current, total, message));
            }
            CallbackEvent::Warning(msg) => {
                self.send(format!("⚠️ Warning: {}\n", msg));
            }
            CallbackEvent::Error(msg) => {
                self.send(format!("❌ Error: {}\n", msg));
            }
            CallbackEvent::Custom { name, data } => {
                // Only log custom events if they're meaningful
                if !data.is_null() {
                    self.send(format!("Event `{}`: {}\n", name, data));
                }
            }
        }
    }
}
