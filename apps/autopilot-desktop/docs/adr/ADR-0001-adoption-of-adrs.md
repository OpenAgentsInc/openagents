# ADR-0001: Adoption of Architecture Decision Records

## Status

**Accepted**

## Date

2026-01-24

## Context

Autopilot needs a lightweight way to capture architectural decisions in a stable, discoverable format that:
- Records the rationale for future contributors
- Clarifies which document is authoritative for a decision
- Prevents repeated debates on settled choices

## Decision

**We adopt Architecture Decision Records (ADRs) as the canonical source for architectural intent.**

### ADR System

1. ADRs live in `docs/adr/` with monotonic numbering (`ADR-0001-title.md`)
2. Numbers are never reused, even for superseded ADRs
3. Status values: `Proposed`, `Accepted`, `Superseded`, `Deprecated`
4. ADRs must reflect the current system terminology and component names used in this repo

### Authority Hierarchy

Autopilot documentation follows this authority order:

| Priority | Concern | Authoritative Source |
|----------|---------|---------------------|
| 1 | Behavior | Code and runtime behavior |
| 2 | Architecture intent | ADRs |
| 3 | Implementation status | README and operational docs |
| 4 | Priorities | Project planning docs |

### When ADRs Are Required

An ADR is required for decisions that affect external contracts or long-lived architecture:

- Public API routes, schemas, or payload formats
- Agent integration details (ACP behavior, unified event format, session lifecycle)
- Data model changes that impact clients or migrations
- Deployment or hosting assumptions
- Cross-cutting architectural patterns (stream handling, storage, logging)

Implementation-only changes (bug fixes, refactors, performance tweaks) do not require ADRs.

## Consequences

**Positive:**
- Clear source of truth for architectural decisions
- Historical record of decision rationale
- Easier onboarding and handoff

**Negative:**
- Added documentation overhead for significant changes
- Risk of ADRs going stale if not maintained

**Neutral:**
- Requires discipline to write ADRs before or during implementation
- Engineers should consult `docs/adr/` before making contract changes

## Alternatives Considered

1. **Ad-hoc documentation** - Rejected due to inconsistency and poor discoverability.
2. **RFCs instead of ADRs** - Rejected as too heavy for this repo.
3. **Single decision log file** - Rejected due to poor scalability and ownership.

## References

- docs/adr/ADR-0001-adoption-of-adrs.md
