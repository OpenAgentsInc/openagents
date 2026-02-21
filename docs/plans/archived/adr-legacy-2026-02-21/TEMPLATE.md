# ADR-XXXX: Title

## Status

**Proposed** | Accepted | Superseded | Deprecated

## Date

YYYY-MM-DD

## Context

What problem or situation triggered this decision? What forces are at play?

(Keep this section focused on the problem, not the solution.)

## Decision

State the decision clearly and normatively. This should be quotable.

Example format:
> We will use X for Y because Z.

Be specific about:
- What is now canonical
- What is deprecated (if anything)
- What constraints apply

### Schema / Spec Authority (if applicable)

If this ADR relates to schemas defined elsewhere, link to the canonical spec:

- [docs/execution/ARTIFACTS.md](../execution/ARTIFACTS.md) — RECEIPT.json, PR_SUMMARY.md
- [docs/execution/REPLAY.md](../execution/REPLAY.md) — REPLAY.jsonl
- [docs/dse/OPTIMIZERS.md](../dse/OPTIMIZERS.md) — Policy bundles
- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — Protocol schemas

This ADR states *what is canonical* and *compatibility guarantees*, not full schema duplication.

(Delete this section if not applicable.)

## Scope

What this ADR covers:
- ...

What this ADR does NOT cover (non-goals):
- ...

## Invariants / Compatibility

What must remain stable:

| Invariant | Guarantee |
|-----------|-----------|
| Filename/path | Stable: `example.json` |
| Field name | Stable: `step_utility` |
| Value range | Stable: -1.0 to +1.0 |

Backward compatibility expectations:
- ...

Versioning rules (if applicable):
- ...

## Consequences

What are the results of this decision?

**Positive:**
- ...

**Negative:**
- ...

**Neutral:**
- ...

## Alternatives Considered

1. **Alternative A** — Brief description. Why rejected.
2. **Alternative B** — Brief description. Why rejected.
3. **Alternative C** — Brief description. Why rejected.

## References

- Relevant code path: `path/to/code`
- [GLOSSARY.md](../GLOSSARY.md) — Canonical terminology
- [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — Protocol details

---

## Optional Sections

### Migration Plan

If this ADR changes existing behavior, describe the migration path.

### Deprecations

List any terms, files, or protocols being deprecated:

| Deprecated | Replacement | Removal Target |
|------------|-------------|----------------|
| `old_term` | `new_term` | v2.0 |

### Operational Considerations

**Telemetry:**
- What must be logged/traced?

**Security/Privacy:**
- Redaction requirements
- Secrets handling

**Rollout:**
- Shadow mode plan
- Canary requirements
- Rollback procedure
