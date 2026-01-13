# ADR-0003: Replay Formats and Migration

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents captures agent execution traces for debugging, training data generation, transparency, and session replay. We need a canonical format specification.

## Decision

**`REPLAY.jsonl` v1 is the target canonical format. `ReplayBundle` is the current implementation.**

### REPLAY.jsonl v1 Specification

A JSON Lines file where each line is a typed event:

```jsonl
{"type":"ReplayHeader","version":"1.0.0","session_id":"...","policy_bundle_id":"...","started_at":"ISO8601"}
{"type":"SessionStart","task":"...","context":{}}
{"type":"ToolCall","id":"...","tool":"...","params":{}}
{"type":"ToolResult","id":"...","output":"...","step_utility":0.5,"latency_ms":100}
{"type":"Verification","tests_before":0,"tests_after":0,"delta":0}
{"type":"SessionEnd","ended_at":"ISO8601","outcome":"success|failure|timeout"}
```

### Event Types

| Event | Required Fields | Optional Fields |
|-------|-----------------|-----------------|
| `ReplayHeader` | version, session_id, started_at | policy_bundle_id |
| `SessionStart` | task | context, instructions |
| `ToolCall` | id, tool, params | |
| `ToolResult` | id, output | step_utility, latency_ms, side_effects |
| `Verification` | tests_before, tests_after, delta | ci_status |
| `SessionEnd` | ended_at, outcome | error_message |

### Current Implementation: ReplayBundle

The `ReplayBundle` struct in `crates/autopilot-core` is the current runtime representation:

```rust
pub struct ReplayBundle {
    pub header: ReplayHeader,
    pub events: Vec<ReplayEvent>,
}
```

### Migration Path

1. **Phase 1 (Current):** `ReplayBundle` is used internally
2. **Phase 2:** Exporter writes `REPLAY.jsonl` v1 format
3. **Phase 3:** Importers read both formats
4. **Phase 4:** `ReplayBundle` becomes internal-only

### Exporter Expectations

The `autopilot export` command must:
- Accept `ReplayBundle` as input
- Output `REPLAY.jsonl` v1 format
- Validate all required fields are present
- Compute `replay_hash` for `RECEIPT.json`

### Terminology

| Term | Status |
|------|--------|
| `REPLAY.jsonl` | Canonical file format |
| `ReplayBundle` | Internal Rust struct |
| `rlog` | Legacy alias (avoid in new code) |
| `trajectory` | High-level concept (session trace) |

## Consequences

**Positive:**
- Clear target format for tooling
- Backward compatibility during migration
- Language-agnostic (JSON Lines)

**Negative:**
- Two representations to maintain during migration
- Potential inconsistencies if migration incomplete

**Neutral:**
- Exporters/importers handle format conversion

## Alternatives Considered

1. **Keep ReplayBundle as canonical** — Rust-specific, harder to consume externally.

2. **Protobuf format** — More efficient but requires schema distribution.

3. **SQLite database** — Good for queries, harder to share as files.

## References

- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — Detailed replay specification
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle contract
- [GLOSSARY.md](../../GLOSSARY.md) — `REPLAY.jsonl`, `ReplayBundle`
