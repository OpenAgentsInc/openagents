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
  — **the Rust build plan, FOSS-only** (third same-day revision; owner ruling:
  no BSL/FSL/SSPL dependencies — SpacetimeDB is excluded, its architecture
  shapes retained). Selected stack: Postgres (authority + durable log) + NATS
  Core (fanout/wakeups) + owned Khala Sync grown a Rust client + owned
  greenfield Rust relay. Track R ships `crates/oa-relay` over the existing
  Cloud SQL store in weeks (differential conformance against `nostr-effect`,
  then cutover of `relay.openagents.com`), with NATS multi-gateway fanout and
  a load-triggered redb/LMDB/Tantivy query projection; Track S builds
  `khala-sync-rs` for Omega and the invalidate-and-re-execute
  query-subscription tier from the July owned-sync-engine decision; Track B
  is a Rust Blossom media store over GCS. Answers fork-vs-greenfield:
  greenfield core, `nostr-rs-relay` and Buzz as fixture/technique quarries,
  rust-nostr as wire-primitive dependency candidate. Surveys and rejects the
  non-FOSS field (Convex, PowerSync, SurrealDB), maps FOSS references
  (ElectricSQL, Zero, LiveStore, Replicache), and calls for the FOSS-only
  rule to land in INVARIANTS on admission. Fourteen ordered candidate
  packets; no packet waits on a vendor, license, or VM.
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
