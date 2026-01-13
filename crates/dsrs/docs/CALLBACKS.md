# Callbacks System

> **Status:** Accurate (CANONICAL)
> **Last verified:** d44f9cd3f
> **Source of truth:** `crates/dsrs/src/callbacks.rs`
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

The callback system provides observability hooks for monitoring dsrs execution in real-time.

## DspyCallback Trait

```rust
pub trait DspyCallback: Send + Sync {
    /// Called when a module starts execution
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example);

    /// Called when a module completes (success or failure)
    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>);

    /// Called when an LM call starts
    fn on_lm_start(&self, call_id: Uuid, model: &str, prompt_tokens: usize);

    /// Called when an LM call completes
    fn on_lm_end(&self, call_id: Uuid, result: Result<(), &Error>, usage: &LmUsage);

    /// Called when optimizer generates a candidate
    fn on_optimizer_candidate(&self, candidate_id: &str, metrics: &HashMap<String, f32>);

    /// Called when a trace is complete
    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>);

    /// Generic event handler
    fn on_event(&self, event: CallbackEvent);
}
```

## Built-in Implementations

### NoopCallback

Does nothing. Default when no callback configured.

```rust
let callback = NoopCallback;
```

### LoggingCallback

Logs events to stdout (useful for debugging).

```rust
let callback = LoggingCallback::new();

// With custom prefix
let callback = LoggingCallback::with_prefix("[dsrs]");
```

Output:
```
[dsrs] module_start call_id=abc123 module=Predict
[dsrs] module_end call_id=abc123 success=true tokens=150
```

### CollectingCallback

Collects all events for later inspection.

```rust
let callback = CollectingCallback::new();

// After execution
let events = callback.events();
for event in events {
    println!("{:?}", event);
}

// Clear collected events
callback.clear();
```

### CompositeCallback

Combines multiple callbacks.

```rust
let callback = CompositeCallback::new(vec![
    Arc::new(LoggingCallback::new()),
    Arc::new(CollectingCallback::new()),
]);
```

## Configuration

### Global Callback

```rust
use dsrs::prelude::*;

// Set callback during configuration
configure_with_callback(
    LM::new("codex-3-sonnet"),
    ChatAdapter,
    LoggingCallback::new(),
);

// Or set callback separately
set_callback(LoggingCallback::new());

// Get current callback
let callback = get_callback();
```

## CallbackEvent

Generic event type for custom handling.

```rust
pub enum CallbackEvent {
    ModuleStart {
        call_id: Uuid,
        module_name: String,
        timestamp: u64,
    },
    ModuleEnd {
        call_id: Uuid,
        success: bool,
        duration_ms: u64,
        tokens: Option<u64>,
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
    OptimizerCandidate {
        candidate_id: String,
        metrics: HashMap<String, f32>,
    },
    TraceComplete {
        node_count: usize,
        total_tokens: u64,
        total_cost_msats: u64,
    },
    Custom {
        name: String,
        data: Value,
    },
}
```

## Custom Callbacks

Implement `DspyCallback` for custom behavior:

```rust
pub struct HudCallback {
    sender: mpsc::Sender<HudEvent>,
}

impl DspyCallback for HudCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, inputs: &Example) {
        let _ = self.sender.try_send(HudEvent::ModuleStarted {
            id: call_id.to_string(),
            name: module_name.to_string(),
        });
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        let _ = self.sender.try_send(HudEvent::ModuleCompleted {
            id: call_id.to_string(),
            success: result.is_ok(),
            tokens: result.ok().map(|p| p.lm_usage.total_tokens),
        });
    }

    // ... implement other methods
}
```

## Integration with Runtime HUD

For OpenAgents Runtime integration:

```rust
use dsrs::callbacks::DspyCallback;
use runtime::hud::HudService;

pub struct RuntimeHudCallback {
    hud: Arc<HudService>,
}

impl DspyCallback for RuntimeHudCallback {
    fn on_module_start(&self, call_id: Uuid, module_name: &str, _inputs: &Example) {
        self.hud.emit_span_start(
            call_id.to_string(),
            format!("dsrs.{}", module_name),
        );
    }

    fn on_module_end(&self, call_id: Uuid, result: Result<&Prediction, &Error>) {
        self.hud.emit_span_end(
            call_id.to_string(),
            result.is_ok(),
            result.ok().map(|p| serde_json::json!({
                "tokens": p.lm_usage.total_tokens,
                "cost_msats": p.lm_usage.cost_msats,
            })),
        );
    }

    fn on_trace_complete(&self, graph: &Graph, manifest: Option<&CompiledModuleManifest>) {
        // Emit trace summary
        self.hud.emit_trace_summary(serde_json::json!({
            "node_count": graph.nodes.len(),
            "compiled_id": manifest.and_then(|m| m.compiled_id.clone()),
        }));
    }
}
```

## Best Practices

1. **Keep callbacks fast** - They're called synchronously during execution
2. **Use channels for async work** - Don't block on I/O in callbacks
3. **Handle errors gracefully** - Callback failures shouldn't crash execution
4. **Use CompositeCallback** - Combine logging + metrics + HUD callbacks
5. **Clear CollectingCallback** - Prevent memory growth in long-running processes
