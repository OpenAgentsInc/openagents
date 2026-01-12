//! Callback system for observing DSPy module execution.
//!
//! The callback system enables:
//! - Streaming events to the HUD
//! - Publishing traces to Nostr
//! - Logging and debugging
//! - Cost tracking and monitoring
//!
//! Callbacks are invoked at key points during module execution:
//! - Module start/end
//! - LM inference start/end
//! - Optimizer candidate evaluation
//! - Trace completion

use crate::core::lm::LmUsage;
use crate::data::{Example, Prediction};
use crate::manifest::CompiledModuleManifest;
use crate::trace::Graph;
use anyhow::Error;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Trait for receiving callbacks during DSPy execution.
///
/// Implement this trait to observe module execution, stream events,
/// or publish traces.
///
/// # Thread Safety
///
/// Callbacks must be `Send + Sync` as they may be called from
/// multiple async tasks concurrently.
///
/// # Example
///
/// ```ignore
/// use dsrs::callbacks::{DspyCallback, CallbackEvent};
///
/// struct LoggingCallback;
///
/// impl DspyCallback for LoggingCallback {
///     fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
///         println!("[{}] Module {} started", call_id, module_name);
///     }
///     // ... other methods
/// }
/// ```
pub trait DspyCallback: Send + Sync {
    /// Called when a module begins execution.
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        let _ = (call_id, module_name, inputs);
    }

    /// Called when a module completes execution.
    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        let _ = (call_id, result);
    }

    /// Called when an LM inference begins.
    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize) {
        let _ = (call_id, model, prompt_tokens);
    }

    /// Called when an LM inference completes.
    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage) {
        let _ = (call_id, result, usage);
    }

    /// Called when LM streaming begins.
    fn on_lm_stream_start(&self, call_id: Uuid, model: &str) {
        let _ = (call_id, model);
    }

    /// Called for each streamed token from the LM.
    fn on_lm_token(&self, call_id: Uuid, token: &str) {
        let _ = (call_id, token);
    }

    /// Called when LM streaming ends.
    fn on_lm_stream_end(&self, call_id: Uuid) {
        let _ = call_id;
    }

    /// Called when an optimizer evaluates a candidate.
    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>) {
        let _ = (candidate_id, metrics);
    }

    /// Called when a full trace is complete.
    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        let _ = (graph, manifest);
    }

    /// Called for general events (extensible).
    fn on_event(&self, event: CallbackEvent) {
        let _ = event;
    }
}

/// General callback event for extensibility.
#[derive(Debug, Clone)]
pub enum CallbackEvent {
    /// Custom event with name and data.
    Custom {
        name: String,
        data: serde_json::Value,
    },
    /// Progress update.
    Progress {
        current: usize,
        total: usize,
        message: String,
    },
    /// Warning message.
    Warning(String),
    /// Error message (non-fatal).
    Error(String),
}

/// A no-op callback that does nothing.
///
/// This is the default when no callback is configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopCallback;

impl DspyCallback for NoopCallback {}

/// A callback that logs events to stdout.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingCallback {
    /// Whether to include detailed information.
    pub verbose: bool,
}

impl LoggingCallback {
    /// Create a new logging callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable verbose logging.
    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }
}

impl DspyCallback for LoggingCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        if self.verbose {
            println!(
                "[DSRS] [{:.8}] Module '{}' started with {} inputs",
                call_id,
                module_name,
                inputs.data.len()
            );
        } else {
            println!("[DSRS] Module '{}' started", module_name);
        }
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        match result {
            Ok(pred) => {
                if self.verbose {
                    println!(
                        "[DSRS] [{:.8}] Module completed with {} outputs",
                        call_id,
                        pred.data.len()
                    );
                } else {
                    println!("[DSRS] Module completed");
                }
            }
            Err(e) => {
                println!("[DSRS] [{:.8}] Module failed: {}", call_id, e);
            }
        }
    }

    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize) {
        if self.verbose {
            println!(
                "[DSRS] [{:.8}] LM '{}' inference started ({} prompt tokens)",
                call_id, model, prompt_tokens
            );
        }
    }

    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage) {
        match result {
            Ok(()) => {
                if self.verbose {
                    println!(
                        "[DSRS] [{:.8}] LM inference completed: {} total tokens, {} msats",
                        call_id, usage.total_tokens, usage.cost_msats
                    );
                }
            }
            Err(e) => {
                println!("[DSRS] [{:.8}] LM inference failed: {}", call_id, e);
            }
        }
    }

    fn on_lm_stream_start(&self, call_id: Uuid, model: &str) {
        if self.verbose {
            println!("[DSRS] [{:.8}] LM '{}' streaming started", call_id, model);
        }
    }

    fn on_lm_token(&self, call_id: Uuid, token: &str) {
        if self.verbose {
            // Only log token length to avoid spamming stdout
            println!("[DSRS] [{:.8}] Token: {} chars", call_id, token.len());
        }
    }

    fn on_lm_stream_end(&self, call_id: Uuid) {
        if self.verbose {
            println!("[DSRS] [{:.8}] LM streaming ended", call_id);
        }
    }

    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>) {
        println!(
            "[DSRS] Optimizer candidate '{}': {:?}",
            candidate_id, metrics
        );
    }

    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        println!(
            "[DSRS] Trace complete: {} nodes, manifest: {}",
            graph.nodes.len(),
            manifest.is_some()
        );
    }

    fn on_event(&self, event: CallbackEvent) {
        match event {
            CallbackEvent::Custom { name, data } => {
                println!("[DSRS] Event '{}': {}", name, data);
            }
            CallbackEvent::Progress {
                current,
                total,
                message,
            } => {
                println!("[DSRS] Progress: {}/{} - {}", current, total, message);
            }
            CallbackEvent::Warning(msg) => {
                println!("[DSRS] Warning: {}", msg);
            }
            CallbackEvent::Error(msg) => {
                println!("[DSRS] Error: {}", msg);
            }
        }
    }
}

/// A callback that chains multiple callbacks together.
///
/// Events are dispatched to all inner callbacks in order.
#[derive(Default)]
pub struct CompositeCallback {
    callbacks: Vec<Arc<dyn DspyCallback>>,
}

impl CompositeCallback {
    /// Create a new composite callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a callback to the chain.
    pub fn add(mut self, callback: impl DspyCallback + 'static) -> Self {
        self.callbacks.push(Arc::new(callback));
        self
    }

    /// Add an already-arc'd callback.
    pub fn add_arc(mut self, callback: Arc<dyn DspyCallback>) -> Self {
        self.callbacks.push(callback);
        self
    }
}

impl DspyCallback for CompositeCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        for cb in &self.callbacks {
            cb.on_module_start(call_id, module_name, inputs);
        }
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        for cb in &self.callbacks {
            cb.on_module_end(call_id, result);
        }
    }

    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize) {
        for cb in &self.callbacks {
            cb.on_lm_start(call_id, model, prompt_tokens);
        }
    }

    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage) {
        for cb in &self.callbacks {
            cb.on_lm_end(call_id, result, usage);
        }
    }

    fn on_lm_stream_start(&self, call_id: Uuid, model: &str) {
        for cb in &self.callbacks {
            cb.on_lm_stream_start(call_id, model);
        }
    }

    fn on_lm_token(&self, call_id: Uuid, token: &str) {
        for cb in &self.callbacks {
            cb.on_lm_token(call_id, token);
        }
    }

    fn on_lm_stream_end(&self, call_id: Uuid) {
        for cb in &self.callbacks {
            cb.on_lm_stream_end(call_id);
        }
    }

    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>) {
        for cb in &self.callbacks {
            cb.on_optimizer_candidate(candidate_id, metrics);
        }
    }

    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        for cb in &self.callbacks {
            cb.on_trace_complete(graph, manifest);
        }
    }

    fn on_event(&self, event: CallbackEvent) {
        for cb in &self.callbacks {
            cb.on_event(event.clone());
        }
    }
}

/// A callback that collects events for later inspection.
///
/// Useful for testing and debugging.
#[derive(Default)]
pub struct CollectingCallback {
    events: std::sync::Mutex<Vec<CollectedEvent>>,
}

/// An event collected by CollectingCallback.
#[derive(Debug, Clone)]
pub enum CollectedEvent {
    ModuleStart {
        call_id: Uuid,
        module_name: String,
    },
    ModuleEnd {
        call_id: Uuid,
        success: bool,
    },
    LmStart {
        call_id: Uuid,
        model: String,
        prompt_tokens: usize,
    },
    LmEnd {
        call_id: Uuid,
        success: bool,
        usage: LmUsage,
    },
    LmStreamStart {
        call_id: Uuid,
        model: String,
    },
    LmToken {
        call_id: Uuid,
        token: String,
    },
    LmStreamEnd {
        call_id: Uuid,
    },
    OptimizerCandidate {
        candidate_id: String,
        metrics: HashMap<String, f32>,
    },
    TraceComplete {
        node_count: usize,
        has_manifest: bool,
    },
    Custom(CallbackEvent),
}

impl CollectingCallback {
    /// Create a new collecting callback.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all collected events.
    pub fn events(&self) -> Vec<CollectedEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Clear collected events.
    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get count of events.
    pub fn len(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.events.lock().unwrap().is_empty()
    }
}

impl DspyCallback for CollectingCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, _inputs: &Example) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::ModuleStart {
                call_id,
                module_name: module_name.to_string(),
            });
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        self.events.lock().unwrap().push(CollectedEvent::ModuleEnd {
            call_id,
            success: result.is_ok(),
        });
    }

    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize) {
        self.events.lock().unwrap().push(CollectedEvent::LmStart {
            call_id,
            model: model.to_string(),
            prompt_tokens,
        });
    }

    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage) {
        self.events.lock().unwrap().push(CollectedEvent::LmEnd {
            call_id,
            success: result.is_ok(),
            usage: usage.clone(),
        });
    }

    fn on_lm_stream_start(&self, call_id: Uuid, model: &str) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::LmStreamStart {
                call_id,
                model: model.to_string(),
            });
    }

    fn on_lm_token(&self, call_id: Uuid, token: &str) {
        self.events.lock().unwrap().push(CollectedEvent::LmToken {
            call_id,
            token: token.to_string(),
        });
    }

    fn on_lm_stream_end(&self, call_id: Uuid) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::LmStreamEnd { call_id });
    }

    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::OptimizerCandidate {
                candidate_id: candidate_id.to_string(),
                metrics: metrics.clone(),
            });
    }

    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::TraceComplete {
                node_count: graph.nodes.len(),
                has_manifest: manifest.is_some(),
            });
    }

    fn on_event(&self, event: CallbackEvent) {
        self.events
            .lock()
            .unwrap()
            .push(CollectedEvent::Custom(event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noop_callback() {
        let cb = NoopCallback;
        let example = Example::new(HashMap::new(), vec![], vec![]);
        cb.on_module_start(Uuid::new_v4(), "test", &example);
        // Should not panic
    }

    #[test]
    fn test_logging_callback() {
        let cb = LoggingCallback::new().verbose();
        let example = Example::new(HashMap::new(), vec![], vec![]);
        cb.on_module_start(Uuid::new_v4(), "test", &example);
        // Should print to stdout
    }

    #[test]
    fn test_composite_callback() {
        let collecting = Arc::new(CollectingCallback::new());
        let composite = CompositeCallback::new()
            .add_arc(collecting.clone())
            .add(NoopCallback);

        let example = Example::new(HashMap::new(), vec![], vec![]);
        let call_id = Uuid::new_v4();
        composite.on_module_start(call_id, "test", &example);

        assert_eq!(collecting.len(), 1);
    }

    #[test]
    fn test_collecting_callback() {
        let cb = CollectingCallback::new();
        let example = Example::new(HashMap::new(), vec![], vec![]);
        let call_id = Uuid::new_v4();

        cb.on_module_start(call_id, "module1", &example);
        cb.on_module_end(
            call_id,
            Ok(&Prediction::new(HashMap::new(), LmUsage::default())),
        );

        let events = cb.events();
        assert_eq!(events.len(), 2);

        match &events[0] {
            CollectedEvent::ModuleStart { module_name, .. } => {
                assert_eq!(module_name, "module1");
            }
            _ => panic!("Expected ModuleStart"),
        }

        match &events[1] {
            CollectedEvent::ModuleEnd { success, .. } => {
                assert!(*success);
            }
            _ => panic!("Expected ModuleEnd"),
        }
    }

    #[test]
    fn test_callback_event_variants() {
        let cb = CollectingCallback::new();

        cb.on_event(CallbackEvent::Progress {
            current: 5,
            total: 10,
            message: "Processing".to_string(),
        });

        cb.on_event(CallbackEvent::Warning("Test warning".to_string()));

        assert_eq!(cb.len(), 2);
    }
}
