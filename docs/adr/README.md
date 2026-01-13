# Architecture Decision Records (ADRs)

This directory contains Architecture Decision Records (ADRs) for the OpenAgents project.

## What are ADRs?

ADRs capture significant architectural decisions along with their context and consequences. They are the **source of truth for architectural intent** — the "why" behind major design choices.

ADRs do not duplicate tutorials or usage guides. They set the decision; other docs explain implementation.

## Authority

ADRs are authoritative for architecture decisions (invariants, interfaces, tradeoffs, deprecations). See [ADR-0001](./ADR-0001-adoption-of-adrs.md) for the full authority hierarchy.

| Priority | Concern | Source |
|----------|---------|--------|
| 1 | Behavior | Code wins |
| 2 | Terminology | GLOSSARY.md wins |
| 3 | Architecture intent | **ADRs win** |
| 4 | Implementation status | Crate sources + SYNTHESIS_EXECUTION.md |
| 5 | Priorities/sequencing | ROADMAP.md |

## When to Write an ADR

Write an ADR for changes affecting **interfaces or invariants**, including:

- Artifact contracts (Verified Patch Bundle, receipt fields, replay format)
- Protocol surfaces (schema IDs, verification modes, hashing rules)
- Lane taxonomy / routing semantics
- Naming collision resolutions
- Persistence formats / paths meant to be stable
- Public CLI surface changes
- Cross-crate boundaries

If it's "just implementation" (bug fix, perf tweak) and doesn't change contracts, no ADR needed.

## Naming Convention

ADRs use monotonic numbering that is **never reused**:

```
ADR-0001-title-kebab.md
ADR-0002-another-title.md
...
```

Do not rename ADRs unless absolutely necessary. The title should be short and stable.

## Status Values

| Status | Meaning |
|--------|---------|
| `Proposed` | Under discussion, not yet decided |
| `Accepted` | Decision made, should be followed |
| `Superseded` | Replaced by a newer ADR (link to replacement) |
| `Deprecated` | No longer relevant, kept for historical reference |

## Creating a New ADR

1. Copy [TEMPLATE.md](./TEMPLATE.md)
2. Use the next available number: `ADR-XXXX-your-title.md`
3. Fill in all required sections
4. Set status to `Proposed`
5. Submit PR for review
6. Once merged, update status to `Accepted`
7. Update [INDEX.md](./INDEX.md)

## Superseding an ADR

When a decision changes:

1. Write a new ADR explaining the change
2. Update the old ADR's status to `Superseded by ADR-XXXX`
3. Do not delete or heavily modify the old ADR (preserve history)

## Required Terminology

All ADRs must use canonical terms from [GLOSSARY.md](../../GLOSSARY.md):

| Use | Not |
|-----|-----|
| `policy_bundle_id` | `policy_version` |
| `step_utility` (-1..+1) | (0..1 is `step_utility_norm`) |
| `Verified Patch Bundle` | `Verified PR Bundle` |
| `Cloud` lane | `Datacenter` |
| `autopilot` CLI | `adjutant` CLI |

Do not hardcode NIP-90 kind numbers; reference schema IDs and link [PROTOCOL_SURFACE.md](../PROTOCOL_SURFACE.md).

## Index

See [INDEX.md](./INDEX.md) for the full list of ADRs.
