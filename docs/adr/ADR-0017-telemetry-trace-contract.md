# ADR-0017: Telemetry and Trace Contract

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents emits telemetry from two distinct systems:
- **Callbacks** (`DspyCallback` trait): Module/LM execution observability
- **Replay events** (`ReplayEvent` enum): Tool execution and session recording

Without a contract:
- the two systems are conflated,
- sensitive data leaks into published traces,
- no clear distinction between local storage vs external publication.

We need canonical rules for what is traced where, and what must be redacted before publication.

## Decision

**Telemetry has three layers with different privacy requirements. Callbacks and replay events are separate systems with different purposes.**

### Implementation Status

| Component | Status |
|-----------|--------|
| DspyCallback trait | Implemented (`crates/dsrs/src/callbacks.rs`) |
| REPLAY.jsonl spec | Spec only (current impl uses `ReplayBundle`) |
| Layer C export | Not yet implemented |

### Canonical owner

- Callback trait: `crates/dsrs/src/callbacks.rs`
- Callback docs: [crates/dsrs/docs/CALLBACKS.md](../../crates/dsrs/docs/CALLBACKS.md)
- Replay format: [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md)

### Trace layers (Normative)

| Layer | Scope | Privacy | Params/Output |
|-------|-------|---------|---------------|
| **A: Internal callbacks** | Same process | None required | Full data allowed |
| **B: Local REPLAY.jsonl** | Stored under `${OPENAGENTS_HOME}` | User-controlled | Full `params` + hashes |
| **C: Published/external** | Swarm, public logs | Strict | Hashes only, redacted |

**Layer A (Internal callbacks):**
- `DspyCallback` trait methods receive full data
- Used for HUD streaming, debugging, cost tracking
- Never leaves the process boundary

**Layer B (Local REPLAY.jsonl):**
- Stored at `${OPENAGENTS_HOME}/sessions/{session_id}/REPLAY.jsonl`
- Includes full `params` field (for replay/debugging) AND `params_hash`
- User-controlled; not published without explicit action

**Layer C (Published/external):**
- Produced by export/publish pipeline
- MUST apply privacy policy redaction (see ADR-0016)
- `params` and `output` fields replaced with hashes only

### Callbacks vs Replay events (Normative)

These are **separate systems** with different event types:

**DspyCallback events** (internal observability):

| Event | Trigger | Fields |
|-------|---------|--------|
| `on_module_start` | Module begins | `call_id`, `module_name`, `inputs` |
| `on_module_end` | Module completes | `call_id`, `result` |
| `on_lm_start` | LLM call begins | `call_id`, `model`, `prompt_tokens` |
| `on_lm_end` | LLM call completes | `call_id`, `result`, `usage` |
| `on_optimizer_candidate` | Optimizer evaluates | `candidate_id`, `metrics` |
| `on_trace_complete` | Execution graph done | `graph`, `manifest` |

**ReplayEvent types** (session recording):

| Event | Purpose | Defined in |
|-------|---------|------------|
| `SessionStart` | Session metadata | REPLAY.md |
| `ToolCall` | Tool invocation | REPLAY.md |
| `ToolResult` | Tool result | REPLAY.md |
| `Verification` | Test/build results | REPLAY.md |
| `SessionEnd` | Session summary | REPLAY.md |

Callbacks do NOT emit ToolCall/ToolResult. Those are replay events emitted by the execution runtime.

### Callback behavior (Normative)

Per `crates/dsrs/src/callbacks.rs`:

1. Callbacks MUST be `Send + Sync`.
2. Callbacks SHOULD be non-blocking (use channels for async work).
3. Callback failures MUST NOT crash execution.
4. Multiple callbacks compose via `CompositeCallback`.

### Built-in callbacks

| Callback | Behavior |
|----------|----------|
| `NoopCallback` | Does nothing (default) |
| `LoggingCallback` | Logs to stdout |
| `CollectingCallback` | Collects events in memory |
| `CompositeCallback` | Fans out to multiple callbacks |

### LmUsage structure

Defined in `crates/dsrs/src/core/lm.rs` (canonical). This ADR does not redefine fields; see CALLBACKS.md for current shape.

### Layer C redaction rules (Normative)

When producing Layer C (external) output:

| Field | Redaction Rule |
|-------|----------------|
| `params` | Remove entirely; keep only `params_hash` |
| `output` | Remove entirely; keep only `output_hash` |
| File paths | Apply active privacy policy (ADR-0016) |
| API keys/tokens | Never emit; strip from all fields |

## Scope

What this ADR covers:
- Three-layer trace model (A/B/C)
- Distinction between callbacks and replay events
- Redaction requirements for Layer C

What this ADR does NOT cover:
- REPLAY.jsonl format details (see REPLAY.md)
- Callback event field definitions (see CALLBACKS.md)
- Privacy policy presets (see ADR-0016)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Layer separation | A/B/C have different privacy rules |
| Callback trait | `DspyCallback` in `crates/dsrs/src/callbacks.rs` |
| Layer B includes params | Local REPLAY.jsonl has full params |
| Layer C excludes params | Published traces have hashes only |

Backward compatibility:
- New callback events may be added with default no-op implementations.
- New replay event types may be added.
- Removing callback methods requires superseding ADR.

## Consequences

**Positive:**
- Clear separation of internal vs local vs external
- Local debugging retains full params
- Published traces are privacy-safe

**Negative:**
- Layer C requires export pipeline (not yet implemented)
- Two separate event systems to understand

**Neutral:**
- Layer B files may be large (full params stored)

## Alternatives Considered

1. **Single trace format for all layers** — rejected (privacy risk for external).
2. **Callbacks emit ToolCall/ToolResult** — rejected (conflates two systems).
3. **Never store full params** — rejected (breaks local debugging/replay).

## References

- [crates/dsrs/docs/CALLBACKS.md](../../crates/dsrs/docs/CALLBACKS.md) — callback trait docs
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — replay event format
- [ADR-0003](./ADR-0003-replay-formats.md) — replay format migration
- [ADR-0016](./ADR-0016-privacy-defaults-swarm-dispatch.md) — privacy policy for Layer C
- `crates/dsrs/src/callbacks.rs` — callback implementation
