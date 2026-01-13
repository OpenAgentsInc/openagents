# ADR Index

Architecture Decision Records for the OpenAgents project.

**Quick links:** [README.md](./README.md) (process) | [TEMPLATE.md](./TEMPLATE.md) | Next available: **ADR-0006**

---

## All ADRs

| ADR | Title | Status | Date | Area | Supersedes | Owner | Last verified |
|-----|-------|--------|------|------|------------|-------|---------------|
| [ADR-0001](./ADR-0001-adoption-of-adrs.md) | Adoption of Architecture Decision Records | Accepted | 2026-01-13 | Ops | — | core | 2026-01-13 |
| [ADR-0002](./ADR-0002-verified-patch-bundle.md) | Verified Patch Bundle Contract | Accepted | 2026-01-13 | Execution | — | adjutant | 2026-01-13 |
| [ADR-0003](./ADR-0003-replay-formats.md) | Replay Formats and Migration | Accepted | 2026-01-13 | Execution | — | dsrs | 2026-01-13 |
| [ADR-0004](./ADR-0004-lane-taxonomy.md) | Lane Taxonomy and Naming | Accepted | 2026-01-13 | Routing | — | frlm | 2026-01-13 |
| [ADR-0005](./ADR-0005-step-utility-semantics.md) | Step Utility Semantics | Accepted | 2026-01-13 | Compiler | — | dsrs | 2026-01-13 |

---

## By Area

### Execution (artifacts, receipts, replay)
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle Contract
- [ADR-0003](./ADR-0003-replay-formats.md) — Replay Formats and Migration

### Compiler (policy bundles, signatures, metrics)
- [ADR-0005](./ADR-0005-step-utility-semantics.md) — Step Utility Semantics

### Routing (lanes, providers)
- [ADR-0004](./ADR-0004-lane-taxonomy.md) — Lane Taxonomy and Naming

### Protocol (job schemas, NIP-90)
- (none yet)

### Economy (treasury, payments)
- (none yet)

### UX (CLI surfaces)
- (none yet)

### Security (privacy, redaction)
- (none yet)

### Ops (governance, process)
- [ADR-0001](./ADR-0001-adoption-of-adrs.md) — Adoption of Architecture Decision Records

---

## By Status

### Accepted
- ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005

### Proposed
- (none)

### Superseded
- (none)

### Deprecated
- (none)

---

## Backlog (Proposed Future ADRs)

These are identified needs, not yet written. Claim one by writing the ADR.

### Priority A — Closes recurring doc/code disagreements
- **ADR-0006**: Deterministic hashing + canonicalization rules
- **ADR-0007**: Tool execution contract (Adapter vs Runtime vs Refine)
- **ADR-0008**: Session storage layout + artifact paths
- **ADR-0009**: PlanIR canonical schema + unification
- **ADR-0010**: Decision pipeline gating + counterfactual schema

### Priority B — Makes marketplaces/receipts coherent
- **ADR-0011**: Schema IDs are canonical; kind numbers are incidental
- **ADR-0012**: Objective vs subjective jobs + settlement rules
- **ADR-0013**: Receipt schema + payment proof types + rail/AssetId semantics

### Priority C — Stops "surface drift" for users
- **ADR-0014**: CLI surface ownership and naming
- **ADR-0015**: Policy bundles: format + pin/rollback + rollout states

### Priority D — Governance + privacy (when ready)
- **ADR-0016**: Privacy defaults for swarm dispatch
- **ADR-0017**: Telemetry/trace contract
- **ADR-0018**: Forge adapter contract
