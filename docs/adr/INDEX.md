# ADR Index (Rust Era)

Status: Active
Last updated: 2026-02-26

## Numbering Policy

1. Rust-era ADR numbering restarts at `ADR-0001`.
2. Numbers are monotonic and never reused once published.
3. Active ADR records live only in `docs/adr/`.

Next available ADR: **ADR-0008**

## Lifecycle Statuses

- `Proposed`
- `Accepted`
- `Deprecated`

## Active ADRs

1. `ADR-0001` — Rust-only architecture baseline
   - Status: `Accepted`
   - Date: 2026-02-21
   - File: `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
   - Source issue: `OA-RUST-074` (`#1889`)
2. `ADR-0002` — Proto-first contract governance
   - Status: `Accepted`
   - Date: 2026-02-21
   - File: `docs/adr/ADR-0002-proto-first-contract-governance.md`
   - Source issue: `OA-RUST-075` (`#1890`)
3. `ADR-0003` — Rivet harvest posture and adoption boundaries
   - Status: `Accepted`
   - Date: 2026-02-21
   - File: `docs/adr/ADR-0003-rivet-harvest-posture-and-adoption-boundaries.md`
   - Source issue: `OA-RUST-077` (`#1892`)
4. `ADR-0004` — Compatibility negotiation and support window policy
   - Status: `Accepted`
   - Date: 2026-02-21
   - File: `docs/adr/ADR-0004-compatibility-negotiation-and-support-window-policy.md`
   - Source issue: `OA-RUST-083` (`#1918`)
5. `ADR-0005` — Wallet executor auth, key custody, and receipt canonicalization
   - Status: `Accepted`
   - Date: 2026-02-21
   - File: `docs/adr/ADR-0005-wallet-executor-auth-custody-receipts.md`
   - Source issue: `OA-RUST-095` (`#1930`)
6. `ADR-0006` — Bounded Vercel SSE compatibility lane
   - Status: `Accepted`
   - Date: 2026-02-22
   - File: `docs/adr/ADR-0006-bounded-vercel-sse-compatibility-lane.md`
   - Source issue: `OA-WEBPARITY-069` (`#2039`)
7. `ADR-0007` — Spacetime-only sync transport hard mandate
   - Status: `Accepted`
   - Date: 2026-02-26
   - File: `docs/adr/ADR-0007-spacetime-only-sync-transport-hard-mandate.md`
   - Source issue: `OA-SPACETIME-TOTAL-001` (`#2269`)

## Legacy Archive

Pre-reset ADR corpora were moved to backroom and are historical context only:

- `/Users/christopherdavid/code/backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/plans/archived/`

## Authoring Process

Use:

1. `docs/adr/README.md` for workflow/review process.
2. `docs/adr/TEMPLATE.md` for new ADR content.
3. `docs/plans/rust-migration-invariant-gates.md` for invariant gate mapping required in ADR rationale.
