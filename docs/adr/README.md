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
| 4 | Implementation status | Active codebase (see [PROJECT_OVERVIEW.md](../PROJECT_OVERVIEW.md)) |
| 5 | Priorities/sequencing | ROADMAP.md |

**Stale path note:** Many ADRs reference `crates/*`, `Cargo.toml`, and Rust paths. Rust and those crates were removed and archived (see [RUST_DOCS_ARCHIVE_2026-02-11.md](../RUST_DOCS_ARCHIVE_2026-02-11.md)). Treat those references as historical; the *decisions* in the ADRs still apply. For current implementation, see [PROJECT_OVERVIEW.md](../PROJECT_OVERVIEW.md).

## Accepted vs Wired

Important distinction:

- **Accepted** = we commit to this direction/contract. The ADR is the decision.
- **Wired/Implemented** = code path exists and is used in production.

An ADR being "Accepted" does **not** mean the code is implemented. For implementation status, check the active codebase (apps/web, apps/autopilot-worker, packages/dse, etc.) and [PROJECT_OVERVIEW.md](../PROJECT_OVERVIEW.md).

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

## Don't Restate Canonical Specs

ADRs should **decide**, not **reproduce** full schemas.

If a schema is canonical elsewhere (or was; some paths are in the [Rust/docs archive](../RUST_DOCS_ARCHIVE_2026-02-11.md)):
- ARTIFACTS.md / REPLAY.md (dsrs artifact and replay schemas; see ADR-0002, ADR-0003; archived if not in repo)
- docs/protocol/PROTOCOL_SURFACE.md (protocol contracts; archived to backroom if not in repo)

Then ADRs should:
1. Link to the canonical spec
2. State what is canonical
3. Define migration/compatibility guarantees
4. NOT duplicate the full schema (examples are "illustrative only")

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
2. Use the next available number (see [INDEX.md](./INDEX.md))
3. Fill in all required sections
4. Set status to `Proposed`
5. Submit PR for review
6. Once merged, update status to `Accepted`
7. Update [INDEX.md](./INDEX.md)

## PR Requirements (Checklist)

When a PR touches a contract, the author must:

- [ ] Update existing ADR **or** create new ADR (status: Proposed)
- [ ] Update [INDEX.md](./INDEX.md) with new/changed ADR
- [ ] Update [GLOSSARY.md](../GLOSSARY.md) if terminology changes
- [ ] Update affected canonical specs (ARTIFACTS/REPLAY/PROTOCOL as applicable; see [RUST_DOCS_ARCHIVE](../RUST_DOCS_ARCHIVE_2026-02-11.md) if those paths are archived)
- [ ] Mention ADR in PR_SUMMARY.md (if agent-generated)

## Lint Policy (Future)

CI may enforce:
- New ADR files have corresponding INDEX.md entry
- All required headings present (Status, Date, Context, Decision, Consequences)
- No hardcoded kind numbers (reference schema IDs instead)

Not yet implemented — stated here so tooling can be built.

Suggested implementation: `scripts/adr_lint.rs` (or `.py`). Do not create alternative linters elsewhere.

## Superseding an ADR

When a decision changes:

1. Write a new ADR explaining the change
2. Update the old ADR's status to `Superseded by ADR-XXXX`
3. Do not delete or heavily modify the old ADR (preserve history)

## Required Terminology

All ADRs must use canonical terms from [GLOSSARY.md](../GLOSSARY.md):

| Use | Not |
|-----|-----|
| `policy_bundle_id` | `policy_version` |
| `step_utility` (-1..+1) | (0..1 is `step_utility_norm`) |
| `Verified Patch Bundle` | `Verified PR Bundle` |
| `Cloud` lane | `Datacenter` |
| `autopilot` CLI | `adjutant` CLI |

Do not hardcode NIP-90 kind numbers; reference schema IDs and link [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md).

## Index

See [INDEX.md](./INDEX.md) for the full list of ADRs.
