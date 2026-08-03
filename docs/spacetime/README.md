# docs/spacetime

Historical record for SpacetimeDB in OpenAgents.

**SpacetimeDB is not used in this repository.** It was adopted three times
between 2026-02-25 and 2026-06-22 and removed all three times. No tracked source
file outside `docs/` references it, and no invariant, contract, or runtime path
depends on it. Reviving SpacetimeDB world-service authority is an explicit
non-goal in `docs/omega/2026-07-24-omega-3d-avatar-verse-harvest-audit.md`.

This directory exists so that a reader who greps `SpacetimeDB` — and lands on a
dozen dated June 2026 documents describing a live system in the present tense —
can find the outcome.

## Contents

- [`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md)
  — full audit: all three adoption cycles, the production deployment and its
  decommission, current state, and six findings.
- [`analysis.md`](analysis.md) — external feasibility assessment for a
  prospective fourth adoption on a Rust/Zed desktop + TanStack web + React
  Native mobile stack. **Reference only, not implementation authority**, and
  not an adoption decision. It scores SpacetimeDB well as a hot collaborative
  state plane and poorly as a complete backend, and gates any commitment on
  offline durability, React Native lifecycle handling, per-caller
  authorization-view cost, self-hosted high availability, and the Business
  Source License single-instance grant.
- [`2026-08-03-rust-spacetimedb-nostr-infra-considerations.md`](2026-08-03-rust-spacetimedb-nostr-infra-considerations.md)
  — own analysis grounded in the current repo: whether the Nostr relay, sync
  layer, workroom state plane, or All Work event store should be rebuilt on
  Rust/SpacetimeDB. Recommends **no** on all four now (the measured relay
  bottleneck is connection admission, not storage; Khala Sync already owns the
  capability SpacetimeDB sells; the managed-relay commercial wedge collides
  with the BSL license; the Verse is the only real fit and is a current
  non-goal), decouples the Rust decision from the SpacetimeDB decision, lists
  the host-agnostic ideas worth taking from the external assessments
  (`ingest_seq`, query-engine seam, ephemeral bypass, conformance fixtures),
  and defines falsifiable reopening gates.
- [`analysis-nostr-relay.md`](analysis-nostr-relay.md) — external feasibility
  assessment for SpacetimeDB as the backend of a new Rust Nostr relay.
  **Reference only, not implementation authority**, and note that
  `apps/nostr-relay` is a retired, guard-enforced path. It argues for a Rust
  gateway owning the NIP protocol with SpacetimeDB owning transactional
  admission, replacement, and deletion state — explicitly *not* a direct
  SQLite-to-SpacetimeDB swap, not one SpacetimeDB subscription per Nostr
  `REQ`, and not ephemeral Nostr traffic through the commit log.

## Summary of the three cycles

| Cycle | Dates | Target problem | Production? | Ended by |
| --- | --- | --- | --- | --- |
| 1 | 2026-02-25 (one day) | Canonical sync/replay transport | No | Same-day repository prune |
| 2 | 2026-03-04 → 2026-06-09 | Reintegration as desktop sync | No — stopped at local projection | Bun/Effect workspace rebuild |
| 3 | 2026-06-17 → 2026-06-22 | Verse multiplayer world state | Yes — 5 days on GCE | Owner decision to replace outright |

## Open action from this audit

`spacetime.openagents.com` still resolves to `34.28.177.95`, a Google Cloud
address the project no longer reserves. The record should be deleted, and other
`openagents.com` records swept for the same pattern. See Finding 1.

## Related documents elsewhere

- `docs/game/2026-06-22-effect-typescript-world-backend-replacement-audit.md`
  — the 2026-06-22 owner decision to replace SpacetimeDB (POSTPONED banner)
- `docs/game/2026-06-22-cloudflare-world-production-release-receipt.md`
  — cutover and VM decommission receipt
- `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`
  — SpacetimeDB as a design reference for subscription-as-query
