# ADR-0017: Telemetry and Trace Contract

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents emits telemetry and traces from multiple layers (dsrs callbacks, tool execution, replay events). Without a contract:
- different components log inconsistently,
- sensitive data leaks into traces,
- downstream consumers (HUD, replay, training) receive incomplete data,
- no clear distinction between required vs optional events.

We need canonical rules for what MUST be traced, what MAY be traced, and what MUST be redacted.

## Decision

**All execution layers MUST emit a minimum event set via the callback system. Sensitive fields MUST be redacted before external emission. Callbacks MUST be non-blocking.**

### Canonical owner

- Callback trait: `crates/dsrs/src/callbacks.rs` (DspyCallback)
- Replay events: [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md)
- Tool receipts: [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md)

### Required events (Normative)

These events MUST be emitted by compliant implementations:

| Event | Trigger | Required Fields |
|-------|---------|-----------------|
| `ModuleStart` | Module begins execution | `call_id`, `module_name`, `timestamp` |
| `ModuleEnd` | Module completes | `call_id`, `success`, `duration_ms` |
| `LmStart` | LLM call begins | `call_id`, `model`, `prompt_tokens` |
| `LmEnd` | LLM call completes | `call_id`, `success`, `usage` |
| `ToolCall` | Tool invoked | `id`, `tool`, `params_hash`, `step_id` |
| `ToolResult` | Tool returns | `id`, `output_hash`, `step_utility`, `latency_ms` |

### Optional events (Normative)

These events MAY be emitted:

| Event | Purpose |
|-------|---------|
| `OptimizerCandidate` | Optimizer generates candidate |
| `TraceComplete` | Full execution graph available |
| `Custom` | Application-specific events |

### Redaction rules (Normative)

Before external emission (logs, HUD, swarm), these fields MUST be redacted:

| Field | Redaction Rule |
|-------|----------------|
| `params` (full) | Hash only (`params_hash`), never raw params |
| `output` (full) | Hash only (`output_hash`), never raw output |
| File paths | Apply privacy policy redaction |
| API keys/tokens | Never emit; strip from all fields |
| User identifiers | Hash or omit unless explicitly allowed |

Internal callbacks (same process) MAY receive unredacted data.

### Callback behavior (Normative)

1. Callbacks MUST be non-blocking (use channels for async work).
2. Callback failures MUST NOT crash execution.
3. Callbacks MUST be `Send + Sync`.
4. Multiple callbacks are composed via `CompositeCallback`.

### Built-in callbacks

| Callback | Behavior |
|----------|----------|
| `NoopCallback` | Does nothing (default) |
| `LoggingCallback` | Logs to stdout |
| `CollectingCallback` | Collects events in memory |
| `CompositeCallback` | Fans out to multiple callbacks |

### LmUsage structure (Normative)

```rust
pub struct LmUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub cost_msats: Option<u64>,
    pub model: String,
    pub latency_ms: u64,
}
```

### Integration with REPLAY.jsonl

Callback events map to REPLAY.jsonl events:

| Callback Event | REPLAY Event |
|----------------|--------------|
| `ModuleStart` | (internal only) |
| `ModuleEnd` | (internal only) |
| `LmStart` | (internal only) |
| `LmEnd` | (internal only) |
| `ToolCall` | `ToolCall` |
| `ToolResult` | `ToolResult` |
| `TraceComplete` | `SessionEnd` |

REPLAY.jsonl is the external format; callbacks are the internal hook.

## Scope

What this ADR covers:
- Required vs optional telemetry events
- Redaction requirements for external emission
- Callback behavior constraints
- LmUsage structure

What this ADR does NOT cover:
- REPLAY.jsonl format (see REPLAY.md, ADR-0003)
- Metrics aggregation and dashboards
- Distributed tracing correlation

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Required events | Always emitted for compliant modules |
| Redaction | Sensitive fields never in external traces |
| Non-blocking | Callbacks never block execution |
| LmUsage fields | Stable structure |

Backward compatibility:
- New optional events may be added.
- New fields may be added to existing events.
- Removing required events requires superseding ADR.

## Consequences

**Positive:**
- Consistent observability across all execution
- Safe-by-default for sensitive data
- Enables HUD, replay, and training data extraction

**Negative:**
- Some overhead from callback invocation
- Redaction may lose debugging context

**Neutral:**
- Internal vs external distinction requires care

## Alternatives Considered

1. **No required events** — rejected (inconsistent observability).
2. **Full params/output in traces** — rejected (privacy risk).
3. **Blocking callbacks** — rejected (execution latency).

## References

- [crates/dsrs/docs/CALLBACKS.md](../../crates/dsrs/docs/CALLBACKS.md) — callback system documentation
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — replay format
- [ADR-0003](./ADR-0003-replay-formats.md) — replay formats
- [ADR-0016](./ADR-0016-privacy-defaults-swarm-dispatch.md) — privacy defaults
- `crates/dsrs/src/callbacks.rs` — implementation
