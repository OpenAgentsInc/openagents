# ADR-0008: Session Storage Layout and Artifact Paths

## Status

**Proposed**

## Date

2026-01-13

## Context

Multiple docs and crates reference different storage roots for:
- sessions and replays,
- training datasets,
- policy bundles,
- metrics.

We need one canonical answer for:
- where sessions live,
- where Verified Patch Bundles live,
- what is stable vs configurable.

## Decision

**OpenAgents defines a canonical storage layout with a single session directory per session, containing the Verified Patch Bundle. Base paths are centralized and configurable, but the internal bundle layout is stable.**

### Canonical Layout (Normative)

A "session directory" contains the Verified Patch Bundle:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

(See [ADR-0002](./ADR-0002-verified-patch-bundle.md) for the bundle contract.)

The session directory path is derived from a single path resolver (e.g., `OpenAgentsPaths`) so crates do not invent their own roots.

### Default Roots (Proposed defaults)

- **User-global OpenAgents home**: `~/.openagents/`
- **Session storage**: `~/.openagents/sessions/{session_id}/`
- **Policy bundles**: `~/.openagents/policies/`
- **Training datasets**: `~/.openagents/datasets/`
- **Metrics/traces**: `~/.openagents/metrics/`
- **Config**: `~/.openagents/config/`

If repo-local storage is required for workflow reasons (e.g., demos, reproducible sharing), it must be produced via an explicit **export** command rather than silently writing into the repo.

### Configuration

The base path may be overridden by environment/config (e.g., `OPENAGENTS_HOME`), but the relative layout under that base must remain stable.

## Scope

What this ADR covers:
- Stable internal structure of a session directory
- Stable canonical base-path resolver responsibility
- Default directory layout for sessions/policies/datasets/metrics

What this ADR does NOT cover:
- Exact receipt/replay schemas (ARTIFACTS.md, REPLAY.md)
- Retention policies and cleanup (ops choice)
- Forge export formats (GitHub/GitAfter adapters)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Bundle filenames | Stable: `PR_SUMMARY.md`, `RECEIPT.json`, `REPLAY.jsonl` |
| Session directory contains bundle | Stable |
| Base path resolution | Centralized; crates must not hardcode paths |
| Export is explicit | Stable: do not write repo-local by default |

Backward compatibility:
- If current code writes to a different path, add a migration (symlink, exporter, or dual-write) and document it in the migration plan.

## Migration Plan

1. Inventory current writers:
   - session store (Adjutant/Autopilot)
   - replay writer/exporter
   - dataset collector
   - policy bundle storage
2. Implement/centralize `OpenAgentsPaths` (or equivalent) used by all writers.
3. Add compatibility:
   - Detect legacy locations and migrate or index them
4. Update documentation to reference this ADR for bundle location.

## Consequences

**Positive:**
- Tooling can find artifacts reliably
- Docs stop drifting on "where did my session go?"
- Makes "export bundle" a clean concept

**Negative:**
- Requires touching multiple crates to centralize path selection
- Requires a migration strategy for existing users

**Neutral:**
- Doesn't force a particular retention policy; ops can decide

## Alternatives Considered

1. **Repo-local `.autopilot/` only** — simple, but pollutes repos and is hard to manage across many repos.
2. **Mix of repo-local + home-dir** — workable but must be explicit and centrally resolved (this ADR chooses explicit export).
3. **Database-only storage** — not portable for PR attachments and sharing.

## References

- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — bundle contents
- [SYNTHESIS_EXECUTION.md](../../SYNTHESIS_EXECUTION.md) — current implementation status
- `crates/adjutant/src/dspy/sessions.rs` (session tracking)
- `crates/autopilot-core/src/replay.rs` (replay implementation)
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — canonical schemas
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — canonical schemas
