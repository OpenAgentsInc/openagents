# ADR-0002: Verified Patch Bundle Contract

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents agents produce code changes that need to be:
- Human-reviewable (what changed and why)
- Machine-verifiable (tests passed, CI green, costs tracked)
- Replayable (reproduce the agent's reasoning and actions)

We need a canonical output format that serves all three purposes and can be exported to various collaboration surfaces (GitHub PRs, GitAfter, git patches, NIP-34).

## Decision

**The canonical agent output is a Verified Patch Bundle consisting of three files:**

| File | Purpose | Format |
|------|---------|--------|
| `PR_SUMMARY.md` | Human-readable summary | Markdown |
| `RECEIPT.json` | Machine-verifiable receipt | JSON |
| `REPLAY.jsonl` | Replay log | JSON Lines |

### Schema Authority

The **canonical schema definitions** live in:
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — RECEIPT.json + PR_SUMMARY.md schemas
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — REPLAY.jsonl v1 schema

This ADR states **what is canonical** and **compatibility guarantees**. It does not duplicate the full schemas.

### File Purposes (Illustrative)

**PR_SUMMARY.md** — Human-readable summary of changes:
- Changes made
- Files modified
- Verification results
- Confidence score

**RECEIPT.json** — Machine-verifiable attestation:
- Session metadata
- Tool call hashes (params_hash, output_hash)
- Verification results (verification_delta)
- Policy bundle ID

**REPLAY.jsonl** — Event stream for replay (see [ADR-0003](./ADR-0003-replay-formats.md))

## Scope

What this ADR covers:
- The three-file structure of Verified Patch Bundle
- Which specs are canonical for schema details
- Naming conventions

What this ADR does NOT cover:
- Full schema definitions (see ARTIFACTS.md, REPLAY.md)
- Implementation details
- Forge adapter behavior

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| File names | Stable: `PR_SUMMARY.md`, `RECEIPT.json`, `REPLAY.jsonl` |
| Bundle term | Stable: "Verified Patch Bundle" |
| Location | Stable: `.autopilot/sessions/{session_id}/` |

Backward compatibility:
- Adding new optional fields to RECEIPT.json is allowed
- Removing or renaming existing fields requires a new ADR

### Naming Conventions

| Term | Status |
|------|--------|
| `Verified Patch Bundle` | Canonical |
| `Verified PR Bundle` | Deprecated |
| `Run Bundle` | Alias (acceptable) |

### Export Targets

The Verified Patch Bundle is the internal format. Forge Adapters export to:

- **GitHub** — PR with summary, checks linked
- **GitAfter** — NIP-34 patch events with trajectory hash
- **git** — Patch files with commit message
- **NIP-34** — Nostr events

## Consequences

**Positive:**
- Single source of truth for agent output
- Clear separation of concerns (human/machine/replay)
- Portable across collaboration surfaces

**Negative:**
- Three files to manage instead of one
- Requires tooling to generate and validate

**Neutral:**
- Forge Adapters handle export complexity

## Alternatives Considered

1. **Single combined file** — Harder to parse, mixing concerns.

2. **Database-only storage** — Not portable, can't attach to PRs.

3. **Separate per-tool receipts** — Too granular, hard to aggregate.

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `Verified Patch Bundle`, `policy_bundle_id`
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — Canonical artifact schemas
- [crates/dsrs/docs/REPLAY.md](../../crates/dsrs/docs/REPLAY.md) — Canonical replay schema
- [ADR-0003](./ADR-0003-replay-formats.md) — Replay format specification
