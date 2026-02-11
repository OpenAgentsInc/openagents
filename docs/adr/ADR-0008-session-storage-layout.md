# ADR-0008: Session Storage Layout and Artifact Paths

## Status

**Accepted**

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

**OpenAgents defines a canonical storage layout with `OPENAGENTS_HOME` as the single root. Base paths are centralized and configurable, but the internal layout is stable.**

### OPENAGENTS_HOME (Normative)

| Variable | Default | Description |
|----------|---------|-------------|
| `OPENAGENTS_HOME` | `~/.openagents` | Root directory for all OpenAgents data |

All paths below are relative to `${OPENAGENTS_HOME}`.

### Canonical Layout (Normative)

```
${OPENAGENTS_HOME}/
├── sessions/{session_id}/     # Verified Patch Bundles (one per session)
│   ├── PR_SUMMARY.md
│   ├── RECEIPT.json
│   └── REPLAY.jsonl
├── policies/                  # Policy bundles (layout per OPTIMIZERS.md)
├── datasets/                  # Training datasets
├── metrics/                   # Metrics and traces
└── config/                    # Configuration
```

A "session directory" contains the Verified Patch Bundle (see [ADR-0002](./ADR-0002-verified-patch-bundle.md)).

**Policy bundle internal layout** (subdirectory structure, manifest format) is defined by [OPTIMIZERS.md](../../crates/dsrs/docs/OPTIMIZERS.md). This ADR only defines the root discovery (`${OPENAGENTS_HOME}/policies/`).

The session directory path is derived from a single path resolver (e.g., `OpenAgentsPaths`) so crates do not invent their own roots.

### Repo-Local Storage

Repo-local storage (e.g., `.autopilot/`) is **not written by default**. If repo-local artifacts are needed for workflow reasons (demos, reproducible sharing, PR attachments), they must be produced via an explicit **export** command:

```bash
autopilot session export {session_id} --output ./.autopilot/
```

This keeps repos clean and makes the export action explicit and auditable.

### Legacy Locations

If code currently writes to legacy locations (e.g., `.autopilot/sessions/`), it must:
1. Migrate to `${OPENAGENTS_HOME}/sessions/` as the primary write location
2. Optionally support reading from legacy locations for backward compatibility
3. Document the migration in release notes

## Scope

What this ADR covers:
- `OPENAGENTS_HOME` environment variable and default
- Stable internal structure of a session directory
- Stable canonical base-path resolver responsibility
- Default directory layout for sessions/policies/datasets/metrics
- Repo-local storage policy (export-only)

What this ADR does NOT cover:
- Exact receipt/replay schemas (ARTIFACTS.md, REPLAY.md)
- Retention policies and cleanup (ops choice)
- Forge export formats (GitHub/GitAfter adapters)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| `OPENAGENTS_HOME` | Stable env var name; default `~/.openagents` |
| Bundle filenames | Stable: `PR_SUMMARY.md`, `RECEIPT.json`, `REPLAY.jsonl` |
| Session directory contains bundle | Stable |
| Base path resolution | Centralized; crates must not hardcode paths |
| Repo-local write | Never by default; export-only |

Backward compatibility:
- If current code writes to a different path, add a migration (symlink, exporter, or dual-write) and document it in the migration plan.
- Legacy `.autopilot/` locations may be read for migration but not written.

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
- [SYNTHESIS_EXECUTION.md](../SYNTHESIS_EXECUTION.md) — current implementation status
- `crates/adjutant/src/dspy/sessions.rs` (session tracking)
- `crates/autopilot-core/src/replay.rs` (replay implementation)
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — canonical schemas
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — canonical schemas
