# ADR-0003: Replay Formats and Migration

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents captures agent execution traces for debugging, training data generation, transparency, and session replay. We need a canonical format specification and a migration path from the current implementation.

## Decision

**`REPLAY.jsonl` v1 is the canonical target format. `ReplayBundle` is the current implementation.**

### Schema Authority

The **canonical REPLAY.jsonl v1 schema** is defined in:
- [docs/execution/REPLAY.md](../execution/REPLAY.md)

This ADR states **what is canonical**, **migration expectations**, and **exporter requirements**. It does not duplicate the full schema.

### Format Summary (per REPLAY.md)

REPLAY.jsonl is a JSON Lines file where each line is a typed event with an `event` discriminator tag:

- `ReplayHeader` — Format version header (first line)
- `SessionStart` — Session metadata
- `PlanStart` — Plan generated
- `ToolCall` — Tool invocation with `params_hash`
- `ToolResult` — Tool result with `output_hash`, `step_utility`
- `StepComplete` — Step completion
- `Verification` — Verification run with `verification_delta`
- `SessionEnd` — Session completion

### Current Implementation: ReplayBundle

The `ReplayBundle` struct in `crates/autopilot-core/src/replay.rs` is the current internal runtime representation. It differs from the target REPLAY.jsonl format.

### Migration Path

1. **Phase 1 (Current):** `ReplayBundle` is used internally
2. **Phase 2:** Exporter writes `REPLAY.jsonl v1` format per REPLAY.md
3. **Phase 3:** Importers read both formats
4. **Phase 4:** `ReplayBundle` becomes internal-only (or deprecated)

### MVP Acceptance Criteria

Per [REPLAY.md](../execution/REPLAY.md), MVP is achieved when either:
- Native REPLAY.jsonl v1 emission is implemented, OR
- ReplayBundle emission + working exporter to REPLAY.jsonl v1 exists

This allows shipping with current implementation while maintaining upgrade path.

### Exporter Requirements

The `autopilot export` command must:
- Accept `ReplayBundle` as input
- Output `REPLAY.jsonl v1` format per REPLAY.md
- Validate all required fields are present
- Compute `replay_hash` for `RECEIPT.json`

## Scope

What this ADR covers:
- Which format is canonical (REPLAY.jsonl v1)
- Migration path from ReplayBundle
- Exporter expectations

What this ADR does NOT cover:
- Full schema definition (see REPLAY.md)
- ReplayBundle internal implementation details
- Viewer/importer implementation

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| File extension | Stable: `.jsonl` |
| Event tag | Stable: `"event"` field discriminates event type |
| Header | Stable: `ReplayHeader` with `replay_version: 1` as first line |
| Hashing | Stable: `params_hash`, `output_hash` use canonical JSON serialization |

Backward compatibility:
- New event types may be added; consumers should ignore unknown events
- New fields may be added to existing events; consumers should ignore unknown fields
- Existing required fields will not be removed

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
- Exporter complexity

**Neutral:**
- Exporters/importers handle format conversion

## Alternatives Considered

1. **Keep ReplayBundle as canonical** — Rust-specific, harder to consume externally.

2. **Protobuf format** — More efficient but requires schema distribution.

3. **SQLite database** — Good for queries, harder to share as files.

## References

- [docs/execution/REPLAY.md](../execution/REPLAY.md) — Canonical REPLAY.jsonl v1 specification
- [docs/execution/ARTIFACTS.md](../execution/ARTIFACTS.md) — Artifact overview
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle contract
- [GLOSSARY.md](../GLOSSARY.md) — `REPLAY.jsonl`, `ReplayBundle`
