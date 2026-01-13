# ADR-0001: Adoption of Architecture Decision Records

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents needs a system to capture architectural decisions in a discoverable, stable format that:
- Clarifies which document is authoritative for what concern
- Preserves decision rationale for future contributors
- Provides clear "why" documentation for onboarding

## Decision

**We adopt Architecture Decision Records (ADRs) as the canonical source for architectural intent.**

### ADR System

1. ADRs live in `docs/adr/` with monotonic numbering (`ADR-0001-title.md`)
2. Numbers are never reused, even for superseded ADRs
3. Status values: `Proposed`, `Accepted`, `Superseded`, `Deprecated`
4. All ADRs must use canonical terminology from [GLOSSARY.md](../../GLOSSARY.md)

### Authority Hierarchy

OpenAgents documentation follows a 5-tier authority hierarchy:

| Priority | Concern | Authoritative Source |
|----------|---------|---------------------|
| 1 | **Behavior** | Code wins — if docs say X but code does Y, code is truth |
| 2 | **Terminology** | [GLOSSARY.md](../../GLOSSARY.md) wins — canonical names and definitions |
| 3 | **Architecture intent** | ADRs win — invariants, interfaces, contracts, tradeoffs |
| 4 | **Implementation status** | Crate sources + [SYNTHESIS_EXECUTION.md](../../SYNTHESIS_EXECUTION.md) — what's wired |
| 5 | **Priorities/sequencing** | [ROADMAP.md](../../ROADMAP.md) — what's next |

### When ADRs Are Required

An ADR is required for changes affecting interfaces or invariants:

- Artifact contracts (Verified Patch Bundle, receipt fields, replay format)
- Protocol surfaces (schema IDs, verification modes, hashing rules)
- Lane taxonomy / routing semantics
- Naming collision resolutions
- Persistence formats / paths meant to be stable
- Public CLI surface changes (command names, output formats)
- Cross-crate boundaries (runtime vs dsrs vs product responsibilities)

Implementation-only changes (bug fixes, perf tweaks) do not require ADRs.

## Consequences

**Positive:**
- Clear source of truth for architectural decisions
- Historical record of decision rationale
- Easier onboarding — read ADRs to understand "why"
- Reduced re-litigation of past decisions

**Negative:**
- Additional documentation overhead for significant changes
- Risk of ADRs becoming stale if not maintained
- Learning curve for contributors unfamiliar with ADR format

**Neutral:**
- Requires discipline to write ADRs before/during implementation
- Agents must check `docs/adr/` before implementing contract changes

## Alternatives Considered

1. **Continue with ad-hoc documentation** — Status quo. Rejected because it leads to scattered, inconsistent decisions.

2. **RFCs instead of ADRs** — RFCs are heavier-weight and imply a formal review process. ADRs are lighter and can be written incrementally.

3. **Decision log in a single file** — Harder to navigate, doesn't scale, no clear ownership per decision.

## References

- [docs/adr/README.md](./README.md) — ADR process documentation
- [docs/adr/TEMPLATE.md](./TEMPLATE.md) — ADR template
- [GLOSSARY.md](../../GLOSSARY.md) — Canonical terminology
- [SYNTHESIS_EXECUTION.md](../../SYNTHESIS_EXECUTION.md) — Implementation status
- [ROADMAP.md](../../ROADMAP.md) — Priorities
