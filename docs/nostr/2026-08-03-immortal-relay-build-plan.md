# Immortal: the Rust Nostr Relay Build Plan

Date: 2026-08-03
Status: build plan — candidate packets, not yet admitted dispatch
Repo: <https://github.com/OpenAgentsInc/immortal> (CC0-1.0; skeleton pushed
2026-08-03: README, AGENTS.md doctrine, compiling Cargo scaffold, edition
2024)

Owner-converged design, same day, in order: Rust ASAP → FOSS only
(SpacetimeDB excluded — BSL) → one Rust binary + one Postgres, nothing
else → standard, hardened, simple → single-server deployable → basic Rust
primitives, minimal dependencies → truly greenfield. Prior revisions of
this plan lived at
`docs/spacetime/2026-08-03-rust-spacetimedb-nostr-infra-considerations.md`
and are in Git history; the SpacetimeDB assessments and the three-cycle
historical audit remain in [`docs/spacetime/`](../spacetime/README.md) as
reference.

## 1. The architecture

```text
Nostr clients  ⇄  immortal (one static Rust binary: WebSocket + NIP-11 HTTP)
                      │ tokio-postgres, prepared statements only
                  Postgres
                  ├── nostr_event            (raw_json, ingest_seq BIGSERIAL,
                  │                           reverse_created_at, address_key)
                  ├── nostr_indexed_tag      (compound indexes for NIP-01 access)
                  ├── replaceable_head       (created_at then event-id tie)
                  ├── deletion_tombstone     (incl. deletion-before-event)
                  ├── relay policy tables    (kinds, pubkeys, entitlements)
                  ├── FTS: GENERATED tsvector + GIN  (NIP-50 — the row write
                  │                           IS the index update)
                  └── LISTEN/NOTIFY          (fanout: notify carries ingest_seq
                                              + event id; listeners fetch)
```

**How far can Postgres go? All the way.** It is the store, the transaction
engine, the query engine (arbitrary NIP-01 filters — this is what a
relational planner is for), the search index, and the fanout bus. There is
no broker, no sync service, no cache tier, no sidecar, no second database.
TLS terminates at the reverse proxy the box already runs (nginx/Caddy).

**Admission is one transaction:** dedup (`ON CONFLICT DO NOTHING`),
replaceable-head compare-and-set, tombstone application including the
event-arrives-after-deletion case, policy check, tag indexing, `ingest_seq`
assignment, `NOTIFY`. The `OK` goes to the client only after commit — fail
closed, never optimistic.

**Fanout:** every immortal process `LISTEN`s; a `NOTIFY` payload carries
`ingest_seq` + event id (well under the 8000-byte limit); the process
fetches the row, runs its in-memory `SubscriptionIndex` (indexed by
id/author/kind/tag — no linear per-connection scans), and delivers. A
process that detects a sequence gap catches up by `ingest_seq` range read;
if it cannot, it drops its sockets and clients reconnect — safe by
NIP-01's own reconnect semantics.

**Ephemeral kinds (20000–29999):** verified, policy-checked, broadcast
in-process and via `NOTIFY` — never written to any table.

**Scale path:** one box → bigger box → N immortal processes against one
Postgres (same `NOTIFY`/catch-up mechanics, zero new components) → managed
Postgres if an operator wants it. Every rung is the same two pieces.

## 2. The doctrine (owner-set, recorded in immortal's AGENTS.md)

1. **Standard.** Rust, tokio, Postgres. Debian-stable deployable. Protocol
   behavior comes from NIP text and conformance fixtures, not novel
   architecture.
2. **Hardened.** Prepared statements only; bounds on frame size,
   subscriptions per connection, filters per REQ, query cost; per-IP and
   per-pubkey rate limits; least-privilege DB role; fail closed; systemd
   unit ships with hardening flags; no TLS or secrets in-process beyond the
   DB credential.
3. **Simple.** One crate, one binary, modules `domain/` `store/` `gateway/`.
   Dependency allowlist, total seven: `tokio`, `tokio-tungstenite`,
   `tokio-postgres`, `secp256k1` (bitcoin-core maintained),
   `sha2` (RustCrypto), `serde`, `serde_json`. No ORM, no web framework, no
   third-party Nostr crate — NIP-01 primitives are owned in `domain/` and
   fixture-tested against the NIP specifications. Additions need owner
   sign-off recorded in AGENTS.md.
4. **Deployable by non-specialists.** The standing acceptance test: fresh
   Debian stable + `apt install postgresql` + one binary + the README =
   serving relay, in minutes. A step that needs a specialist is a bug.
5. **CC0.** Public domain. Anyone takes it without asking.

What this doctrine deleted from earlier revisions: NATS (Postgres `NOTIFY`
is the bus), the SQLite dual-backend (owner: Postgres is good — one store),
any third-party Nostr crate (own the primitives), axum (a relay needs a WS
upgrade and one GET; tungstenite's own handshake suffices), the
redb/LMDB/Tantivy projection tier (Postgres indexes + FTS until
measurements say otherwise), and every SpacetimeDB artifact.

## 3. Electric: OpenAgents product infra only — never the relay

Electric (Apache-2.0) remains recommended for OpenAgents' own product read
path: the web app is TanStack end to end, TanStack DB's first-party sync
backend is Electric, and its "writes go through your API" posture matches
the All Work admission law. That adoption stays spike-gated: one week, one
real scope through `electricCollectionOptions` on web plus a minimal Rust
shape consumer, gatekeeper auth with scope-epoch `must-refetch` revocation,
txid-matched optimistic confirmation, and measured sync/latency/slot
behavior. Fallback is the hand-rolled invalidate-and-re-execute tier from
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`.

But Electric is an Elixir service, and the relay's deployment target is a
single box run by strangers. The boundary is absolute and written into
immortal's AGENTS.md: **no Electric, no NATS, no Redis/Valkey, no
SpacetimeDB in the relay, ever.** OpenAgents consumes immortal only through
its wire protocol.

## 4. Fork `nostr-rs-relay` or greenfield?

Greenfield, settled. `nostr-rs-relay` (MIT) is the opposite shape on every
load-bearing axis (streaming-SQL repo trait, process-local broadcast, linear
subscription scans, experimental Postgres, residual prefix matching) — a
fork would gut the entire core. Its test corpus is a **fixture quarry**:
validation and filter-matching corpora, replaceable/deletion fixtures
including deletion-before-event, NIP-42 timing behavior, WebSocket edge
cases, rate-limit vocabulary. The Buzz relay teardown
(`docs/teardowns/2026-07-21-buzz-teardown.md`) is the closer architectural
reference — its `spawn_blocking` Schnorr verify, idempotent insert,
connection semaphore/heartbeat discipline, and generated-column FTS are
adopted here; its TLA+ runtime-conformance idea is the stretch goal.

## 5. Packets

| # | Packet | Where | Depends on |
| --- | --- | --- | --- |
| 1 | ~~Create repo + scaffold~~ **done 2026-08-03**: CC0, README, AGENTS.md doctrine, compiling skeleton | immortal | — |
| 2 | `domain/`: event, tags, filters, canonical ID, replacement address, deletion semantics + NIP fixture corpus (nostr-rs-relay quarry) | immortal | 1 |
| 3 | `store/`: schema migration, admission transaction, `ingest_seq`, `NOTIFY`, FTS column | immortal | 2 |
| 4 | `gateway/`: WS handshake, NIP-01 framing, NIP-11, NIP-42, `SubscriptionIndex`, race-free EOSE handoff, ephemeral lane, bounds + rate limits | immortal | 2 |
| 5 | Conformance suite: per-NIP fixtures gating every packet | immortal | 3, 4 |
| 6 | Single-box acceptance: fresh-Debian script, systemd unit with hardening flags, nginx/Caddy snippet, operator README | immortal | 3, 4 |
| 7 | Our deploy: shadow against the existing Cloud SQL event store, load proof (beat 185 ev/s floor, connect p99 < 1 s at 120 sockets), cutover `relay.openagents.com`, decommission checklist for the Node host | openagents ops | 5 |
| 8 | Multi-process proof: two immortal processes, one Postgres — cross-delivery, gap catch-up, kill-one chaos | immortal | 5 |
| 9 | Blossom media endpoint (NIP-B7/98/94): filesystem storage default, GCS adapter for our deploy — second bin in the immortal repo | immortal | 4 |
| 10 | Electric spike for OpenAgents product read path (§3) | openagents | — |
| 11 | `khala-sync-rs` in its Electric-shaped or fallback form | openagents/omega | 10 |
| 12 | Nostr outbox publisher for All Work safe projections → immortal | openagents | 7 |

Packets 2, then 3+4 in parallel, are the immediate lanes. Packet 7 is the
only one touching OpenAgents production; everything else is public-repo
work a stranger could run.

## 6. Housekeeping

1. Workspace registration: add `immortal/` to the workspace-root contract's
   Workspace Model as a standalone CC0 sibling repo (this session).
2. Amend the workspace Nostr routing rule: immortal is the relay
   implementation home.
3. Delete the vestigial `apps/nostr-relay` Effect-version exception clause
   in `INVARIANTS.md` (~line 137) when implementation starts.
4. FOSS-only infrastructure rule into `INVARIANTS.md` on admission — with
   the sharper immortal corollary: the relay's bar is not just FOSS but
   *single-box, allowlisted-dependency* FOSS.
5. Stray untracked `node_modules` at `apps/nostr-relay/` and
   `apps/openagents-world/` — remove from the canonical checkout.

## 7. Risks

| Risk | Handling |
| --- | --- |
| Postgres `NOTIFY` loss (it is best-effort delivery) | `NOTIFY` is a wake-up hint, never the record: delivery correctness comes from `ingest_seq` catch-up, which every listener runs on reconnect and gap-detect |
| One Postgres as the failure domain | That is the thesis, stated honestly: single-box operators get pg_dump/WAL archiving in the operator README; our deployment gets Cloud SQL HA. Same binary either way |
| Filter queries outgrow Postgres planning | Bounded query budgets first (the budget design in `docs/spacetime/analysis-nostr-relay.md` §6); measured evidence required before any projection tier returns to the plan |
| Seven-dep allowlist proves too tight | The allowlist is a gate, not a wall — additions are one owner sign-off away, recorded in AGENTS.md, which is exactly the friction that keeps the tree readable |
| Protocol drift | Per-NIP fixture corpus is content-addressed and derived from NIP text; a protocol change without a fixture is incomplete by AGENTS.md rule |
| Scope creep back toward services | AGENTS.md rule 1: "if a feature needs another running service, the feature is wrong" |
