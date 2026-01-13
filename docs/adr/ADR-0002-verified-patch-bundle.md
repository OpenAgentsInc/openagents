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

### PR_SUMMARY.md

Human-readable summary of changes:

```markdown
## Summary
Brief description of what was done.

## Changes
- File-by-file breakdown
- Rationale for key decisions

## Test Results
- Tests run: X
- Tests passed: Y
- Coverage: Z%

## Verification
- [ ] Tests pass
- [ ] CI green
- [ ] No regressions
```

### RECEIPT.json

Machine-verifiable attestation:

```json
{
  "schema_version": "1.0.0",
  "session_id": "uuid",
  "policy_bundle_id": "string",
  "started_at": "ISO8601",
  "ended_at": "ISO8601",
  "verification": {
    "tests_before": 0,
    "tests_after": 0,
    "verification_delta": 0,
    "ci_status": "pass|fail|skip"
  },
  "costs": {
    "total_tokens": 0,
    "total_sats": 0
  },
  "tool_calls": [{
    "id": "string",
    "tool": "string",
    "params_hash": "sha256:...",
    "output_hash": "sha256:...",
    "step_utility": -1.0,
    "latency_ms": 0,
    "side_effects": []
  }],
  "replay_hash": "sha256:..."
}
```

### REPLAY.jsonl

See [ADR-0003](./ADR-0003-replay-formats.md) for replay format details.

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
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — Detailed artifact schemas
- [ADR-0003](./ADR-0003-replay-formats.md) — Replay format specification
