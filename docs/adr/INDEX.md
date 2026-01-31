# ADR Index

Architecture Decision Records for the OpenAgents project.

**Quick links:** [README.md](./README.md) (process) | [TEMPLATE.md](./TEMPLATE.md) | Next available: **ADR-0026**

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
| [ADR-0007](./ADR-0007-tool-execution-contract.md) | Tool Execution Contract | Accepted | 2026-01-13 | Execution | — | dsrs | 2026-01-13 |
| [ADR-0008](./ADR-0008-session-storage-layout.md) | Session Storage Layout | Accepted | 2026-01-13 | Execution | — | adjutant | 2026-01-13 |
| [ADR-0009](./ADR-0009-planir-canonical-schema.md) | PlanIR Canonical Schema | Accepted | 2026-01-13 | Compiler | — | dsrs | 2026-01-13 |
| [ADR-0010](./ADR-0010-decision-pipeline-gating.md) | Decision Pipeline Gating | Accepted | 2026-01-13 | Compiler | — | adjutant | 2026-01-13 |
| [ADR-0011](./ADR-0011-schema-ids-canonical.md) | Schema IDs Canonical | Accepted | 2026-01-13 | Protocol | — | protocol | 2026-01-13 |
| [ADR-0012](./ADR-0012-objective-vs-subjective-jobs.md) | Objective vs Subjective Jobs | Accepted | 2026-01-13 | Protocol | — | protocol | 2026-01-13 |
| [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) | Receipt Schema + Payment Proofs | Accepted | 2026-01-13 | Economy | — | protocol | 2026-01-13 |
| [ADR-0014](./ADR-0014-cli-surface-ownership.md) | CLI Surface Ownership | Accepted | 2026-01-13 | UX | — | core | 2026-01-13 |
| [ADR-0015](./ADR-0015-policy-bundles.md) | Policy Bundles | Accepted | 2026-01-13 | Compiler | — | dsrs | 2026-01-13 |
| [ADR-0016](./ADR-0016-privacy-defaults-swarm-dispatch.md) | Privacy Defaults for Swarm Dispatch | Accepted | 2026-01-13 | Security | — | dsrs | 2026-01-13 |
| [ADR-0017](./ADR-0017-telemetry-trace-contract.md) | Telemetry and Trace Contract | Accepted | 2026-01-13 | Execution | — | dsrs | 2026-01-13 |
| [ADR-0018](./ADR-0018-forge-adapter-contract.md) | Forge Adapter Contract | Accepted | 2026-01-13 | Execution | — | adjutant | 2026-01-13 |
| [ADR-0019](./ADR-0019-wgpui-hud-component-contract.md) | WGPUI HUD Component Contract | Accepted | 2026-01-13 | UX | — | autopilot | 2026-01-13 |
| [ADR-0020](./ADR-0020-pylon-local-ui-bridge.md) | Pylon Local UI Bridge (Pusher-Compatible) | Accepted | 2026-01-13 | UX | — | pylon | 2026-01-13 |
| [ADR-0021](./ADR-0021-rust-ts-type-generation.md) | Rust-to-TypeScript Contract Generation for Tauri IPC | Accepted | 2026-01-25 | UX | — | autopilot-desktop | 2026-01-25 |
| [ADR-0022](./ADR-0022-effuse-uitree-ipc.md) | Effuse UITree + UI Patch IPC Contract | Accepted | 2026-01-25 | UX | — | autopilot-desktop | 2026-01-25 |
| [ADR-0023](./ADR-0023-file-editor-open-save-events.md) | File Editor Open/Save Event Contract | Accepted | 2026-01-30 | UX | — | autopilot-ui | 2026-01-30 |
| [ADR-0024](./ADR-0024-openagents-api-moltbook-proxy.md) | OpenAgents API Moltbook Proxy + Index | Accepted | 2026-01-30 | UX | — | api | 2026-01-30 |
| [ADR-0025](./ADR-0025-moltbook-indexer-client-ingest.md) | Moltbook Indexer Client Ingest Endpoint | Accepted | 2026-01-31 | Data | — | indexer | 2026-01-31 |

---

## By Area

### Execution (artifacts, receipts, replay)
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle Contract
- [ADR-0003](./ADR-0003-replay-formats.md) — Replay Formats and Migration
- [ADR-0007](./ADR-0007-tool-execution-contract.md) — Tool Execution Contract
- [ADR-0008](./ADR-0008-session-storage-layout.md) — Session Storage Layout
- [ADR-0017](./ADR-0017-telemetry-trace-contract.md) — Telemetry and Trace Contract
- [ADR-0018](./ADR-0018-forge-adapter-contract.md) — Forge Adapter Contract

### Compiler (policy bundles, signatures, metrics)
- [ADR-0005](./ADR-0005-step-utility-semantics.md) — Step Utility Semantics
- [ADR-0009](./ADR-0009-planir-canonical-schema.md) — PlanIR Canonical Schema
- [ADR-0010](./ADR-0010-decision-pipeline-gating.md) — Decision Pipeline Gating
- [ADR-0015](./ADR-0015-policy-bundles.md) — Policy Bundles

### Routing (lanes, providers)
- [ADR-0004](./ADR-0004-lane-taxonomy.md) — Lane Taxonomy and Naming

### Protocol (job schemas, NIP-90, hashing)
- [ADR-0006](./ADR-0006-deterministic-hashing.md) — Deterministic Hashing and Canonicalization
- [ADR-0011](./ADR-0011-schema-ids-canonical.md) — Schema IDs Canonical
- [ADR-0012](./ADR-0012-objective-vs-subjective-jobs.md) — Objective vs Subjective Jobs

### Data (indexing, analytics)
- [ADR-0025](./ADR-0025-moltbook-indexer-client-ingest.md) — Moltbook Indexer Client Ingest Endpoint

### Economy (treasury, payments)
- [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) — Receipt Schema + Payment Proofs

### UX (CLI surfaces, UI components)
- [ADR-0014](./ADR-0014-cli-surface-ownership.md) — CLI Surface Ownership
- [ADR-0019](./ADR-0019-wgpui-hud-component-contract.md) — WGPUI HUD Component Contract
- [ADR-0020](./ADR-0020-pylon-local-ui-bridge.md) — Pylon Local UI Bridge (Pusher-Compatible)
- [ADR-0021](./ADR-0021-rust-ts-type-generation.md) — Rust-to-TypeScript Contract Generation for Tauri IPC
- [ADR-0022](./ADR-0022-effuse-uitree-ipc.md) — Effuse UITree + UI Patch IPC Contract
- [ADR-0023](./ADR-0023-file-editor-open-save-events.md) — File Editor Open/Save Event Contract
- [ADR-0024](./ADR-0024-openagents-api-moltbook-proxy.md) — OpenAgents API Moltbook Proxy + Index

### Security (privacy, redaction)
- [ADR-0016](./ADR-0016-privacy-defaults-swarm-dispatch.md) — Privacy Defaults for Swarm Dispatch

### Ops (governance, process)
- [ADR-0001](./ADR-0001-adoption-of-adrs.md) — Adoption of Architecture Decision Records

---

## By Status

### Accepted
- ADR-0001, ADR-0002, ADR-0003, ADR-0004, ADR-0005, ADR-0006, ADR-0007, ADR-0008, ADR-0009, ADR-0010, ADR-0011, ADR-0012, ADR-0013, ADR-0014, ADR-0015, ADR-0016, ADR-0017, ADR-0018, ADR-0019, ADR-0020, ADR-0021, ADR-0022, ADR-0023, ADR-0024, ADR-0025

### Proposed
- (none)

### Superseded
- (none)

### Deprecated
- (none)

---

## Backlog (Proposed Future ADRs)

These are identified needs, not yet written. Claim one by writing the ADR.

(none currently identified)
