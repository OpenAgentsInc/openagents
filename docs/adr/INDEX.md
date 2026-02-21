# ADR Index (Rust Era)

Status: Active
Last updated: 2026-02-21

## Numbering Policy

1. Rust-era ADR numbering restarts at `ADR-0001`.
2. Numbers are monotonic and never reused once published.
3. Active ADR records live only in `docs/adr/`.
4. Historical ADRs remain in the archive path and are not renumbered.

Next available ADR: **ADR-0005**

## Lifecycle Statuses

- `Proposed`
- `Accepted`
- `Superseded`
- `Deprecated`
- `Archived`

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
3. `ADR-0003` — Khala WS-only replay transport  
   - Status: `Accepted`  
   - Date: 2026-02-21  
   - File: `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`  
   - Source issue: `OA-RUST-076` (`#1891`)
4. `ADR-0004` — Rivet harvest posture and adoption boundaries  
   - Status: `Accepted`  
   - Date: 2026-02-21  
   - File: `docs/adr/ADR-0004-rivet-harvest-posture-and-adoption-boundaries.md`  
   - Source issue: `OA-RUST-077` (`#1892`)

## Legacy Archive

Pre-reset ADR corpus (archived by OA-RUST-004):

- `docs/plans/archived/adr-legacy-2026-02-21/`
- Catalog: `docs/plans/archived/adr-legacy-2026-02-21/CATALOG.md`
- Removed placeholder ADR stubs: `docs/plans/archived/adr-rust-stubs-2026-02-21/`

Legacy ADRs are historical context only and not authoritative for Rust-era architecture decisions.

## Authoring Process

Use:

1. `docs/adr/README.md` for workflow/review process.
2. `docs/adr/TEMPLATE.md` for new ADR content.
3. `docs/plans/active/rust-migration-invariant-gates.md` for invariant gate mapping required in ADR rationale.
