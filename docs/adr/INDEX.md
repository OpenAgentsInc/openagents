# ADR Index

Architecture Decision Records for the OpenAgents project.

**Quick links:** [README.md](./README.md) (process) | [TEMPLATE.md](./TEMPLATE.md) | Next available: **ADR-0016**

---

## All ADRs

| ADR | Title | Status | Date | Area | Supersedes | Owner | Last verified |
|-----|-------|--------|------|------|------------|-------|---------------|
| [ADR-0001](./ADR-0001-adoption-of-adrs.md) | Adoption of Architecture Decision Records | Accepted | 2026-01-13 | Ops | — | core | 2026-01-13 |
| [ADR-0002](./ADR-0002-verified-patch-bundle.md) | Verified Patch Bundle Contract | Accepted | 2026-01-13 | Execution | — | adjutant | 2026-01-13 |
| [ADR-0003](./ADR-0003-replay-formats.md) | Replay Formats and Migration | Accepted | 2026-01-13 | Execution | — | dsrs | 2026-01-13 |
| [ADR-0004](./ADR-0004-lane-taxonomy.md) | Lane Taxonomy and Naming | Accepted | 2026-01-13 | Routing | — | frlm | 2026-01-13 |
| [ADR-0005](./ADR-0005-step-utility-semantics.md) | Step Utility Semantics | Accepted | 2026-01-13 | Compiler | — | dsrs | 2026-01-13 |
| [ADR-0006](./ADR-0006-deterministic-hashing.md) | Deterministic Hashing and Canonicalization | Accepted | 2026-01-13 | Protocol | — | protocol | 2026-01-13 |
| [ADR-0007](./ADR-0007-tool-execution-contract.md) | Tool Execution Contract | Proposed | 2026-01-13 | Execution | — | dsrs | — |
| [ADR-0008](./ADR-0008-session-storage-layout.md) | Session Storage Layout | Accepted | 2026-01-13 | Execution | — | adjutant | 2026-01-13 |
| [ADR-0009](./ADR-0009-planir-canonical-schema.md) | PlanIR Canonical Schema | Proposed | 2026-01-13 | Compiler | — | dsrs | — |
| [ADR-0010](./ADR-0010-decision-pipeline-gating.md) | Decision Pipeline Gating | Proposed | 2026-01-13 | Compiler | — | adjutant | — |
| [ADR-0011](./ADR-0011-schema-ids-canonical.md) | Schema IDs Canonical | Proposed | 2026-01-13 | Protocol | — | protocol | — |
| [ADR-0012](./ADR-0012-objective-vs-subjective-jobs.md) | Objective vs Subjective Jobs | Proposed | 2026-01-13 | Protocol | — | protocol | — |
| [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) | Receipt Schema + Payment Proofs | Proposed | 2026-01-13 | Economy | — | protocol | — |
| [ADR-0014](./ADR-0014-cli-surface-ownership.md) | CLI Surface Ownership | Accepted | 2026-01-13 | UX | — | core | 2026-01-13 |
| [ADR-0015](./ADR-0015-policy-bundles.md) | Policy Bundles | Proposed | 2026-01-13 | Compiler | — | dsrs | — |

---

## By Area

### Execution (artifacts, receipts, replay)
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle Contract
- [ADR-0003](./ADR-0003-replay-formats.md) — Replay Formats and Migration
- [ADR-0007](./ADR-0007-tool-execution-contract.md) — Tool Execution Contract *(Proposed)*
- [ADR-0008](./ADR-0008-session-storage-layout.md) — Session Storage Layout

### Compiler (policy bundles, signatures, metrics)
- [ADR-0005](./ADR-0005-step-utility-semantics.md) — Step Utility Semantics
- [ADR-0009](./ADR-0009-planir-canonical-schema.md) — PlanIR Canonical Schema *(Proposed)*
- [ADR-0010](./ADR-0010-decision-pipeline-gating.md) — Decision Pipeline Gating *(Proposed)*
- [ADR-0015](./ADR-0015-policy-bundles.md) — Policy Bundles *(Proposed)*

### Routing (lanes, providers)
- [ADR-0004](./ADR-0004-lane-taxonomy.md) — Lane Taxonomy and Naming

### Protocol (job schemas, NIP-90, hashing)
- [ADR-0006](./ADR-0006-deterministic-hashing.md) — Deterministic Hashing and Canonicalization
- [ADR-0011](./ADR-0011-schema-ids-canonical.md) — Schema IDs Canonical *(Proposed)*
- [ADR-0012](./ADR-0012-objective-vs-subjective-jobs.md) — Objective vs Subjective Jobs *(Proposed)*

### Economy (treasury, payments)
- [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) — Receipt Schema + Payment Proofs *(Proposed)*

### UX (CLI surfaces)
- [ADR-0014](./ADR-0014-cli-surface-ownership.md) — CLI Surface Ownership

### Security (privacy, redaction)
- (none yet)

### Ops (governance, process)
- [ADR-0001](./ADR-0001-adoption-of-adrs.md) — Adoption of Architecture Decision Records

---

## By Status

### Accepted
- ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0008, ADR-0014

### Proposed
- ADR-0007, ADR-0009, ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0015

### Superseded
- (none)

### Deprecated
- (none)

---

## Backlog (Proposed Future ADRs)

These are identified needs, not yet written. Claim one by writing the ADR.

### Priority D — Governance + privacy (when ready)
- **ADR-0016**: Privacy defaults for swarm dispatch
- **ADR-0017**: Telemetry/trace contract
- **ADR-0018**: Forge adapter contract
