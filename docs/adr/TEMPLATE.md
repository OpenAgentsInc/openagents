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

## Consequences

What are the results of this decision?

**Positive:**
- ...

**Negative:**
- ...

**Neutral:**
- ...

## Alternatives Considered

1. **Alternative A** - Brief description. Why rejected.
2. **Alternative B** - Brief description. Why rejected.
3. **Alternative C** - Brief description. Why rejected.

## References

- [Relevant code path](../../crates/example/src/file.rs)
- [GLOSSARY.md](../../GLOSSARY.md) - Canonical terminology
- [PROTOCOL_SURFACE.md](../PROTOCOL_SURFACE.md) - Protocol details

---

## Optional Sections

### Migration Plan

If this ADR changes existing behavior, describe the migration path.

### Deprecations

List any terms, files, or protocols being deprecated:

| Deprecated | Replacement | Removal Target |
|------------|-------------|----------------|
| `old_term` | `new_term` | v2.0 |
