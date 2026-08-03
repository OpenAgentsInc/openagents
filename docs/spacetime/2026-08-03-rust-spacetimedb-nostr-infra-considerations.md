# Rust Build Plan: Nostr Relay and Core Infra (FOSS-Only)

Date: 2026-08-03 (third revision, same day. First: assessment. Second: owner
direction — Rust ASAP, build plan. **Third: owner ruling — FOSS only.
SpacetimeDB is BSL 1.1, not FOSS, and is excluded. No BSL, FSL, SSPL, or
otherwise source-available-encumbered dependency may sit in this
infrastructure.** This revision replaces every SpacetimeDB role with a FOSS
substitution and keeps everything else. Fourth same-day amendment: §6
standardizes the read path on Electric, and §0.1 records the owner direction
that this Rust Nostr infra lives in a NEW STANDALONE REPO, not the
monorepo — naming shortlist there.)
Status: build plan and roadmap — candidate packets, not yet admitted dispatch
Grounded at: `5e9c2ac2a0` (current `main`)

Companions: [`analysis-nostr-relay.md`](analysis-nostr-relay.md) and
[`analysis.md`](analysis.md) remain useful as *architecture* references (the
gateway/store split, admission semantics, client-state model) — their vendor
is now excluded, their shapes are not.
[`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md)
records the prior cycles. The owned-sync-engine design this plan builds on is
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md` —
the owner's July call: *"a full sync engine, on Postgres, built by us, fully
in our control."* That call is the FOSS answer; this plan extends it to Rust.

## 0. The plan in one page

```text
Track R — Rust relay (starts now, ships in weeks)
  R0  greenfield Rust gateway over the EXISTING Cloud SQL
      Postgres event store → replaces the Node host; fixes the measured
      connection-admission limit; zero data migration; Cloud Run deploy;
      differential conformance against nostr-effect on the same database
  R1  Multi-gateway: NATS Core fanout of committed events (ingest_seq-keyed),
      fail-closed catch-up from Postgres
  R2  Dedicated query/search projection when load demands it
      (redb or LMDB indexes + Tantivy for NIP-50) behind RelayQueryEngine

Track S — state plane (Khala Sync grows a Rust half)
  S0  khala-sync-rs: Rust Khala Sync client crate for Omega
      (same store semantics as the Node/Expo/web-WASM stores)
  S1  Read-path standard: Electric shapes over Postgres logical replication
      (spike-gated, §6) — hand-rolled invalidate-and-re-execute tier is the
      specified fallback
  S2  All Work hot state rides this plane — no new database
      (presence/ephemeral over NATS, never through a durable store)

Track B — Blossom media store in Rust (independent, needed regardless)
  B0  oa-blossom: NIP-B7 server over GCS, NIP-98 auth, NIP-94 metadata
  (all of Track R and B live in a NEW standalone repo — §0.1)
```

Dropping SpacetimeDB makes the plan **strictly simpler**: no GCE VM, no vendor
license negotiation, no dual-store shadow/parity phase, no module-migration
discipline against three client release trains, no in-memory ceiling on a
growing archive. Every runtime dependency below is OSI-licensed or owned
outright, and everything deploys on the surfaces already in production (Cloud
Run, Cloud SQL, GKE).

### 0.1 A new standalone repo, and its name

Owner direction (2026-08-03): the Rust Nostr infra is **not** part of the
`openagents` monorepo. It gets its own sibling repo under `OpenAgentsInc`,
registered in the workspace model like `probe`, `psionic`, and
`nostr-effect`. Scope of the new repo: the relay workspace (`nostr-domain`,
gateway, stores, conformance, indexer), the Blossom server, and the Nostr
outbox publisher daemon. Out of scope: `khala-sync-rs` (sync, not Nostr — it
lands per the Omega/monorepo decision) and `nostr-effect` (stays the TS
protocol library and conformance oracle).

Naming: house style is a lowercase single word, and the estate is already
Protoss-leaning (Khala, Khala Sync, Pylon, Psionic, Artanis, Tassadar).
Owner asked for hardening connotations ("diamond") tied to the StarCraft
universe. Shortlist:

| Name | Why | Notes |
| --- | --- | --- |
| **`khaydarin`** | The Khaydarin crystal is the substrate that anchors and amplifies the Khala — exactly what this infra is to Khala Sync and the signed record. Deep-lore, unique, zero collisions | Top pick for meaning |
| **`immortal`** | The Immortal's Hardened Shields are THE hardening mechanic in SC2; "immortal" also reads as uptime | Top pick for hardening |
| **`neosteel`** | Terran hardened building material ("neosteel frame") — the closest literal analogue to "diamond" | Unique, punchy, greppable |
| **`archon`** | Merged Templar — raw power; also reads as system-administrator | Common word, minor collisions |
| **`carrier`** | "Carrier has arrived." — a relay literally carries signed events | Telecom double meaning is apt |
| **`bunker`** | Terran structure whose whole job is holding the line | Blunt, memorable |
| **`hyperion`** | Raynor's flagship — endures everything | Used across many franchises |
| **`purifier`** | Protoss faction; a relay that validates and purifies admission | Slightly long |
| **`chitin`** | Zerg carapace material — biological hardening | Least obviously SC |

Avoid: `shieldbattery` (existing SC open-source project), `aegis` (prior
internal Hydra/Aegis authority naming), `oracle`/`sentry` (company
collisions), `nexus`/`pylon`/`forge`/`khala` (taken in this estate). Until
the owner picks, this document says "the relay repo."

## 1. What SpacetimeDB was doing, and the FOSS substitution for each role

| Role STDB was assigned | FOSS replacement | License |
| --- | --- | --- |
| Relay admission authority (dedup, replaceable heads, tombstones, policy — one transaction) | Postgres, where R0 already puts it. It simply stays there | PostgreSQL License |
| Committed-event fanout to gateway replicas | NATS Core, `ingest_seq`-keyed; catch-up reads from Postgres; gateways fail closed on gaps | Apache-2.0 |
| Live typed replicas across Rust/TS clients | Khala Sync — owned — extended with a Rust client for Omega | Ours |
| Query-subscription with server-computed deltas | Electric shapes over logical replication (recommended, spike-gated — §6); hand-rolled invalidate-and-re-execute tier as specified fallback | Apache-2.0 / ours |
| Hot workroom/All Work operational state | Cloud SQL + Khala Sync scopes (the plane that already exists) | PostgreSQL / ours |
| Presence and transient signals (event tables) | NATS subjects / the relay's ephemeral lane — no durable store at all | Apache-2.0 |
| Verse world state (if revived) | Rust region service on the same Postgres + NATS substrate | ours / above |

The honest observation: `analysis.md` estimated SpacetimeDB would save
"roughly two-thirds of the custom Linear-style sync backend." For OpenAgents
that estimate was always wrong, because **half of that backend already
exists** — Khala Sync's client stores (Node SQLite, Expo, web SQLite-WASM),
offline queue, optimistic overlays, rebase, cursors, and scope taxonomy are
built and shipping. What remains is the Rust client and the subscription
tier, and both are specified below.

## 2. Options survey

Hard filter first — **excluded by the FOSS ruling**, regardless of technical
merit:

| Excluded | License | Note |
| --- | --- | --- |
| SpacetimeDB | BSL 1.1 | One-instance grant, "Database Service" prohibition — the trigger for this revision |
| Convex (self-hosted backend) | FSL | Its invalidate-and-re-execute *idea* is freely reusable; its code is not needed |
| PowerSync | FSL | Its checkpoint/write-checkpoint *discipline* is reusable as design; service excluded |
| SurrealDB | BSL | — |
| Redis < 8.0 | RSALv2/SSPL | Redis 8+ offers AGPLv3, but Valkey makes the question moot |

Evaluated FOSS candidates for the live-sync role, and why each is a
reference rather than the substrate:

| Candidate | License | Verdict |
| --- | --- | --- |
| **ElectricSQL** | Apache-2.0 | Best-in-class read-path sync over Postgres logical replication. **Elevated from reference to recommended read-path standard — see §6**, added 2026-08-03 after reviewing the electric.ax docs and our TanStack DB position. Write path, permissions, and local durability remain ours — which is the division of labor Electric itself prescribes |
| **Zero (Rocicorp)** | Apache-2.0 | Steal the CDC topology (logical replication → single-writer replica → view-syncer readers) and its honest online-optimistic v1 write contract. TS-only, young, and its server would sit exactly where Khala Sync already sits |
| **LiveStore** | Apache-2.0 | Built on Effect — the closest artifact to Khala Sync's target shape; read its sync provider source before writing ours (fable doc's instruction, still right) |
| **Replicache** | Apache-2.0 (maintenance) | The canonical push/pull mutation-rebase spec; its "backend strategies" ladder is the roadmap we are already on step 3 of |
| **Automerge / yrs / Loro** | MIT | CRDTs for collaborative *text fields only* (stored as opaque columns, synced as values). Wrong tool for relational business data — no server-enforceable permissions in a merge function; every production system with a server converged on server-ordered mutations. Relevant later for Omega buffer collab if Zed's own protocol is ever insufficient |
| **FoundationDB** | Apache-2.0 | Serious transactional core, Rust bindings, but no live queries and a large ops burden — solves a problem we do not have while Postgres is nowhere near its limits |
| **NATS Core** | Apache-2.0 | Selected: the fanout/wakeup bus for relay and sync. CNCF, tiny, clustered, runs on the existing GKE cluster or a small GCE group. JetStream (also Apache-2.0) available later if a durable stream is ever wanted — not needed now since Postgres is the durable log |
| **Valkey** | BSD-3 | Acceptable pub/sub alternative to NATS (Buzz used Redis pub/sub in this seat); NATS preferred for subject hierarchy and clustering ergonomics |
| **redb / LMDB(heed) / Tantivy** | MIT / OpenLDAP / MIT | R2's dedicated query/search projection when needed |
| **rust-nostr** | MIT | Wire-primitive dependency candidate (week-1 evaluation) |
| **sqlx, axum, tokio-tungstenite, secp256k1** | MIT/Apache | R0's core dependencies — all standard FOSS Rust |

**The selected stack, in one line: Postgres (authority + durable log) +
NATS Core (fanout/wakeups) + owned Khala Sync (typed replicas, offline,
rebase) + owned Rust relay — with ElectricSQL/Zero/LiveStore/Replicache as
design quarries, not dependencies.** This is also the only option that adds
zero new licenses to the estate: PostgreSQL and Apache-2.0 are already in it.

## 3. Fork `nostr-rs-relay` or greenfield?

**Greenfield. Quarry `nostr-rs-relay` for fixtures and edge cases; do not
fork it.**

What our relay must be: a gateway/store split behind a `RelayState` trait;
multi-gateway fanout off a committed `ingest_seq`-ordered feed with
fail-closed catch-up; admission as one transaction (dedup, replaceable heads,
tombstones, policy, entitlements); typed OpenAgents projections written in
the same transaction; an indexed `SubscriptionIndex`; differential
conformance against `nostr-effect` as a build gate; Buzz-profile custom NIPs.

`nostr-rs-relay` (v0.10.0, MIT) is the opposite shape on every load-bearing
axis: its `NostrRepo` trait is a *streaming-SQL-query* interface; its live
path is a single **process-local** Tokio broadcast — no cross-instance fanout
exists; each connection linearly scans its own subscription list per event;
SQLite is primary and Postgres experimental; its in-memory filter code still
contains prefix matching that current NIP-01 removed; and payment/admission
is coupled directly to relay storage. A fork that replaces the repository
trait, both storage engines, the query planner, the live path, the
subscription model, and the policy coupling is a greenfield build wearing
someone else's directory layout — inheriting an architecture that fights the
`RelayState` seam, forfeiting upstream pulls after week one, and confusing
every future contributor about which half of the tree is live.

What it *is* excellent for — taken deliberately, MIT, with attribution:
protocol parsing and canonical-event validation tests; the filter-matching
corpus (minus prefix cases); replaceable/deletion edge-case fixtures
including deletion-before-event; NIP-42 challenge/timing behavior; WebSocket
edge-case handling; rate-limit and query-budget concepts; Prometheus metric
names; and its hand-written SQLite query-planner heuristics as design notes
for the R2 indexer.

Buzz's relay is the closer architectural reference for what we are building —
Axum, multi-tenant, agent-native, idempotent Postgres insert, connection
semaphore/heartbeat discipline, runtime TLA+ conformance — and we already
hold a deep teardown of it. The wire-primitive layer needs neither codebase:
the MIT `rust-nostr` crates are the standard dependency for event/filter
types, evaluated in packet 1, with domain rules staying owned in
`nostr-domain` either way.

**Formula: rust-nostr as wire-primitive dependency (pending evaluation),
`nostr-rs-relay` and Buzz as fixture/technique quarries, owned greenfield
core.** The same posture the workspace applies to every reference repo:
study, port ideas, don't vendor trees.

## 4. Track R — the Rust relay

### R0: Rust gateway over the existing store (weeks 1–3) — unchanged by the FOSS ruling

A new Cargo workspace in the relay repo (§0.1 — standalone sibling, not the
monorepo; the retired `apps/nostr-relay` path stays untouched either way):

```text
<relay repo>/crates/
├── nostr-domain        pure Rust: event classification, replacement address,
│                       filter matching, deletion semantics, policy vocabulary
├── oa-relay-gateway    WebSocket protocol server (axum + tokio-tungstenite):
│                       NIP-01 framing, NIP-11, NIP-42 per-connection auth,
│                       SubscriptionIndex, rate limits, backpressure,
│                       historical/live EOSE handoff, ephemeral fanout
├── oa-relay-store-pg   RelayState + RelayQueryEngine over Cloud SQL (sqlx),
│                       admission as ONE transaction: dedup, replaceable-head
│                       compare (created_at then event-id tie), tombstones incl.
│                       deletion-before-event, ingest_seq assignment
└── oa-relay-conformance NIP fixture matrix (analysis-nostr-relay §17, plus the
                        nostr-rs-relay quarry) + differential runner vs
                        nostr-effect on shared Postgres
```

Decisions, made now:

1. **Greenfield core, per §3.** rust-nostr evaluated week 1; domain rules
   owned in `nostr-domain` regardless.
2. **Schema:** R0 reads/writes the existing event tables so the TS and Rust
   relays run against the same database simultaneously. Only additive
   migration: `ingest_seq BIGSERIAL` + the missing compound indexes from
   `analysis-nostr-relay.md` §10.
3. **`RelayState` is a trait from day 1.** Postgres is the impl. (The trait
   stays because it is good engineering — a future store is a swap — not
   because a vendor swap is planned.)
4. **Ephemeral kinds (20000–29999) never touch the store.** In-process
   broadcast single-gateway; NATS when replicas > 1.
5. **Conformance is differential** against `nostr-effect` — divergence is a
   build failure. The TS library's accumulated NIP correctness transfers to
   the Rust relay for free.

Deploy: Cloud Run (gateway is stateless, store is Cloud SQL — no VM
anywhere in this plan), shadow hostname first, then cutover.

**R0 exit gates:** conformance matrix green (duplicate races, replaceable
same-timestamp tie, addressable `d`-tags, deletion-before-event, NIP-40
expiry, NIP-42/70 enforcement, filter AND/OR/limit/order); load proof ≥3× the
185 ev/s floor with connect p99 under 1 s at 120 sockets (the Node host's
measured failure point); race-free EOSE under concurrent publish;
forced-restart durability read-back.

**R0 cutover:** point `relay.openagents.com` at the Rust gateway; Node
revision stays warm one week as rollback; decommission checklist (DNS,
certificates, old revisions) written *before* cutover — the historical
audit's dangling-DNS finding is the loose-end class this plan does not leave.

### R1: multi-gateway fanout (weeks 3–4 — simpler than the STDB version it replaces)

- Admission stays in Postgres. On commit, the store publishes the committed
  event to NATS keyed by `ingest_seq` (subject per relay database, e.g.
  `relay.events.<db>`).
- Gateway replicas subscribe; each keeps its own `SubscriptionIndex`; a
  replica that detects a sequence gap catches up by `ingest_seq` range read
  from Postgres, and **fails closed** (drops its sockets) if it cannot —
  clients reconnect and re-`REQ`, which is safe.
- NATS Core deploys on the existing GKE cluster (or a small GCE instance
  group) — stateless enough to replace freely; no JetStream needed because
  Postgres is the durable log.
- The ephemeral lane moves onto NATS subjects at the same time
  (`relay.ephemeral.<db>`), still never touching storage.

**R1 exit gates:** two-gateway cross-delivery with no missing/duplicate
events under concurrent publish; kill-one-gateway chaos test with clean
client re-`REQ`; NATS node restart with gap-detected catch-up proven.

### R2: dedicated query/search projection (when load demands, not before)

The `RelayQueryEngine` trait already isolates this. When Postgres filter
queries or NIP-50 need more than indexes can give: a `relay-indexer` consumes
the committed sequence into redb or LMDB (via heed) for the hot NIP-01 access
patterns and Tantivy for NIP-50 search, plus the NIP-77 sorted set. Postgres
remains the authority and durable log; the projection is disposable and
rebuildable from `ingest_seq` 0. The nostr-rs-relay query-planner heuristics
inform the index selection logic.

## 5. Track S — the state plane: Khala Sync grows a Rust half

The July owner call (fable doc §4) stands and is the FOSS answer: the sync
engine is **Khala Sync, on Postgres, built by us, fully in our control**. The
2026-07-04 design predates the Cloudflare retirement, so its D1/Durable
Object/Hyperdrive machinery reads as historical — but its architecture
(per-scope monotonic sequences, mutation outbox, cursored snapshots,
server-ordered mutators, client rebase) is host-independent and is what the
current `packages/khala-sync*` family ships on Cloud SQL today.

### S0: `khala-sync-rs` — the Rust client (weeks 1–4, parallel with R0)

A Rust crate implementing the same store semantics as the Node/Expo/web
stores: confirmed state, cursors, tombstones, offline queue, optimistic
overlays, rebase, typed transport failures — over SQLite (rusqlite) for the
local store. Conformance: the same fixture corpus the three TS stores pass,
run against the Rust store in CI (the differential-testing pattern R0 uses
for the relay, applied to sync).

Consumer: **Omega**, behind an owned domain-model boundary (the
`SyncRepository` layering from `analysis.md` §2 — store rows never leak into
GPUI code). This delivers the thing SpacetimeDB's Rust SDK was going to
deliver — typed live state in the primary surface's native language — with no
vendor, no license, and no second sync plane.

### S1: query-subscription tier v1 (weeks 4–8)

The fable doc already picked the design and it is the buildable one:
**invalidate-and-re-execute on recorded read sets** (the Convex lesson —
"far simpler to build than incremental view maintenance; the correct v1/v2
split"). Concretely: named scoped queries record their read sets; the
mutation path publishes invalidations over NATS; subscribed clients
re-execute affected queries against their cursors. Restricting the filter
grammar to index-able predicate classes (the ElectricSQL lesson) keeps
change→subscriber matching O(1). v2 — incremental view maintenance or a
CDC topology à la Zero — only if v1's re-execution cost is measured to
matter.

### S2: All Work hot state rides this plane — no new database

The All Work service architecture is already: admission gateway → Cloud SQL
canonical Work Event + projection transaction → Khala Sync deltas + durable
Nostr outbox. That stands unchanged. What this track adds is the *live* tier
on top: workroom/run/activity scopes served through S1 subscriptions, Omega
consuming through S0, presence and transient signals (typing, "agent
thinking", cursor pulses) over NATS subjects only — never through a durable
store. Streamed model output batches into bounded rows on the existing rules.

Verse, if revived: a Rust region service on this same substrate — Postgres
rows for durable world state, NATS for position/interaction fanout, the
region service as the single-writer authority per region. The cycle-3 scope
boundary (presence/interaction only; business truth in owned projections)
carries over verbatim.

## 6. Standardizing the read path on Electric (TypeScript + Rust)

Added 2026-08-03 at owner direction, after reviewing the electric.ax docs
(TanStack integration, intro, HTTP API — fetched this date) against our
actual client position.

### 6.1 Where we stand

The web app is already TanStack end to end: `apps/openagents.com/apps/start`
ships `@tanstack/db` 0.6.14 + `@tanstack/react-db`, fed today by the owned
`@openagentsinc/khala-sync-db-collection` adapter, which bridges the Khala
Sync client into a TanStack DB `SyncConfig` (confirmed rows in the Khala
store, optimistic effects in the Khala overlay). No Electric packages are in
the tree. Meanwhile TanStack DB's first-party sync backend *is* Electric —
they co-built it, and the electric collection path (`electricCollectionOptions`
with txid-matched optimistic confirmation) is the ecosystem's paved road.

### 6.2 What Electric actually is, and why it fits unusually well here

Electric (Apache-2.0 — passes the FOSS rule) is a **read-path-only** sync
engine over Postgres logical replication. Its entire client contract is one
HTTP endpoint: `GET /v1/shape` with `table`/`where`, an `offset` cursor, a
shape `handle`, long-poll or SSE live mode, `up-to-date` and `must-refetch`
control messages, and CDN-grade caching with request collapsing. Writes are
explicitly out of scope: *"writes flow through your application's existing
API."*

Three properties line up with decisions this plan already made:

1. **The write boundary is our law already.** Electric's "reads sync, writes
   go through your API" is exactly the All Work admission gateway / projection
   law. Adopting Electric changes nothing about authority — it standardizes
   the half we were going to hand-roll.
2. **One wire protocol serves both languages.** The shape protocol is plain
   HTTP + JSON with an offset cursor. The official clients are TypeScript and
   Elixir, but a Rust consumer is a small fetch loop — a few hundred lines,
   not a port of a client library. This restructures `khala-sync-rs` (S0):
   instead of reimplementing the whole TS store contract in Rust, the Rust
   side becomes **shape consumer + local SQLite materializer + the
   write/outbox half**. The hardest third of the port disappears into a
   protocol.
3. **It deletes most of S1.** The query-subscription tier v1
   (invalidate-and-re-execute, read-set tracking, NATS invalidation fanout,
   predicate-grammar restrictions) is us rebuilding what Electric's shape log
   already does — change→shape matching over logical replication, with
   resumable offsets and CDN request collapsing that our design would not
   have had. The fable doc said "steal the HTTP offset-resumable log
   protocol"; running the engine that ships it is the stronger form of
   stealing, and it is the same team's protocol either way.

### 6.3 What Electric does not do — the parts that stay Khala Sync

Standardizing on Electric is an evolution of Khala Sync, not a replacement.
The name keeps meaning the owned sync contract; Electric becomes its read
transport:

| Concern | Owner after adoption |
| --- | --- |
| Write admission, named mutators, rebase, idempotency | Khala Sync / All Work gateway (unchanged) |
| Durable offline queue + optimistic overlay semantics | Khala Sync client stores (unchanged; TanStack DB overlay is in-memory) |
| Local durable stores (Node/Expo/web-WASM SQLite, Rust SQLite) | Ours — Electric's client is resume-from-offset, not a persistence layer |
| Scope taxonomy → shape mapping | Ours: scopes become `table` + `where` on indexed scope columns |
| Authorization | Ours, via the gatekeeper pattern: clients never hit Electric directly; the API issues shape-scoped grants and proxies `/v1/shape` (Electric's own recommended deployment) |
| Permission-change fanout | Ours, and it needs a design: Electric has no live permission checks (Figma's LiveGraph does; Electric doesn't). A member removal must invalidate their shapes — handle with a scope-epoch column in the `where` clause so revocation forces `must-refetch` |
| Presence/ephemeral | NATS, as before — never through the shape log |

### 6.4 Costs and risks, honestly

- **An Elixir service enters the estate.** One container, state rebuildable
  from Postgres (shape logs are a cache), deployable next to NATS on GKE.
  Real but bounded; it is the only new runtime this adoption adds.
- **Young ecosystem at the edges.** TanStack DB is 0.x and the electric
  collection packages are younger still; our adapter seam
  (`khala-sync-db-collection`) already isolates the app from this — keep the
  adapter, swap its feed.
- **Vendor evolution.** The electric.ax rebrand and Electric Cloud push are
  visible. Apache-2.0 plus a deliberately simple wire protocol is the exit:
  if the vendor drifts, the protocol is reimplementable over our own logical
  replication consumer (Zero's CDC topology is the sketch). The `RelayState`
  lesson applies: consume Electric behind our adapter seams, never let shape
  types leak into domain code.
- **Postgres display-format strings.** Shape values arrive as
  Postgres-formatted strings; both the TS adapter and the Rust materializer
  must own one typed decode layer (Effect Schema / serde) — a conformance
  fixture concern, already our house style.
- **The relay does not change.** Nostr's wire protocol is its own; R1's NATS
  fanout stays (push latency beats long-poll for `EOSE`-adjacent delivery).
  A shape over `nostr_event` keyed by `ingest_seq` is a *possible* future
  catch-up transport for gateway replicas, noted and not planned.

### 6.5 Verdict and the spike that gates it

**Yes — standardize the read path on Electric across TypeScript and Rust**,
as the transport under the Khala Sync contract. It aligns the web app with
its own framework's paved road, collapses the S1 build, halves the Rust
client port, and adds only FOSS. Gate it with a one-week spike before
committing packets 9–12 to the new shape:

- Stand up Electric against a Cloud SQL replica; sync one real scope
  (`fleet_run`) end to end into TanStack DB via `electricCollectionOptions`
  on web and a minimal Rust shape consumer.
- Prove the gatekeeper: shape-scoped grant issuance, proxying, and a
  permission-revocation `must-refetch` via scope-epoch.
- Prove txid-matched optimistic confirmation against our mutator path.
- Measure: initial sync bytes/time for a realistic scope, live-mode latency
  vs the current delivery path, and Electric's replication-slot behavior
  under our write volume.

Exit green → packets 9–12 execute in their Electric-shaped form (§8 table).
Exit red on any structural item → fall back to the §5 hand-rolled tier,
which remains fully specified.

## 7. Track B — Blossom media store in Rust (weeks 2–4, independent)

`oa-blossom` (in the relay repo per §0.1): NIP-B7 server over GCS, NIP-98
auth, NIP-94 metadata.
The NIP survey established the need (NIP-44 caps payloads at 64 KB; Full Auto
evidence does not fit in events). Small, self-contained, no dependency on any
other track — a good first-blood crate while R0 conformance is being built.
All FOSS dependencies (axum, GCS client via `google-cloud-rust` or signed
URLs).

## 8. Packet breakdown (candidate issues, in order)

| # | Packet | Track | Depends on |
| --- | --- | --- | --- |
| 1 | Create the relay repo (§0.1) + Cargo workspace scaffold + `nostr-domain` + rust-nostr evaluation | R0 | — |
| 2 | `oa-relay-store-pg`: admission transaction + `ingest_seq` migration | R0 | 1 |
| 3 | Gateway: NIP-01/11/42, SubscriptionIndex, EOSE handoff, ephemeral lane | R0 | 1 |
| 4 | Conformance crate: fixture matrix (incl. nostr-rs-relay quarry) + differential runner vs nostr-effect | R0 | 2, 3 |
| 5 | Shadow deploy + load proof + R0 cutover of `relay.openagents.com` | R0 | 4 |
| 6 | NATS Core deployment (GKE or small GCE group) + ops runbook | infra | — |
| 7 | R1 multi-gateway fanout + fail-closed catch-up + chaos gates | R1 | 5, 6 |
| 8 | `oa-blossom` NIP-B7/98/94 over GCS | B0 | — |
| 8b | **Electric spike (§6.5): deploy vs Cloud SQL replica, one real scope on web via `electricCollectionOptions` + minimal Rust shape consumer, gatekeeper + scope-epoch revocation, txid-matched optimistic confirm, measurements** | S1 | — |
| 9 | `khala-sync-rs`: Rust shape consumer + SQLite materializer + write/outbox half (spike green) — or full store-contract port (fallback) | S0 | 8b |
| 10 | Omega `SyncRepository` adoption of `khala-sync-rs` | S0 | 9 |
| 11 | Read-path tier: Electric deploy + gatekeeper + scope→shape mapping + `khala-sync-db-collection` fed from shapes (spike green) — or hand-rolled invalidate-and-re-execute tier (fallback, §5) | S1 | 6, 8b |
| 12 | All Work live scopes over S1 + presence subjects | S2 | 10, 11 |
| 13 | Nostr outbox publisher daemon (Rust) for All Work safe projections | S2 | 5 |
| 14 | R2 query/search projection (redb/LMDB + Tantivy) — load-triggered | R2 | 7 |

Packets 1–4, 6, 8, 8b, and (in fallback form) 9 are parallel lanes from day
one under the normal claim protocol. No packet anywhere waits on a vendor
license or a VM; packet 8b is one week and de-risks three others.

## 9. Housekeeping the plan requires (same change, not later)

1. **FOSS-only infrastructure rule becomes written policy.** The owner's
   2026-08-03 ruling — no BSL/FSL/SSPL/source-available runtime dependency in
   OpenAgents infrastructure; OSI-approved licenses or owned code only —
   should land in `INVARIANTS.md` when this plan is admitted, so the next
   evaluation applies it as a hard filter *first* (this directory now
   contains three documents that evaluated a BSL product on technical merit
   before license posture — the rule inverts that order permanently).
2. **Repo creation and contract updates land with packet 1:** create the
   `OpenAgentsInc/<relay repo>` repository (§0.1), register it in the
   workspace-root contract's Workspace Model, and amend the
   `nostr-effect`-first routing rule to name it as the relay implementation
   home, with `nostr-effect` as TS client library and conformance oracle
   (shared fixture corpus published with digests so the differential runner
   works cross-repo). `khala-sync-rs` placement is a separate note in the
   Omega/monorepo lane.
3. **INVARIANTS line ~137** still names the retired `apps/nostr-relay` as the
   isolated Effect-version exception — delete the vestigial clause in the
   same pass.
4. **Naming discipline:** no renames of existing planes, ever, as part of
   this program (cycle 1's Khala/Spacetime rename is the cautionary record;
   "Khala Sync" is always the two-word compound per its own naming note).
5. **Stray dirs:** remove the untracked `node_modules` at
   `apps/nostr-relay/` and `apps/openagents-world/` in the canonical
   checkout.

## 10. Risks and their handling

| Risk | Handling |
| --- | --- |
| Postgres write-path becomes the relay bottleneck someday | It is not today (admission held 100% at measured load; the limit was connections). `ingest_seq` + the R2 projection absorb read pressure; Cloud SQL scales vertically with years of credit headroom per the fable doc; the `RelayState` trait keeps a future store swap cheap if it ever comes to that |
| NATS adds an ops surface | Core only (no JetStream), stateless, on existing GKE; the durable log stays in Postgres so NATS loss degrades to catch-up reads, never data loss |
| `khala-sync-rs` drifts from the TS stores | Shared fixture corpus in CI — the same differential-conformance pattern as the relay; the store contract is the deliverable |
| Subscription tier v1 re-execution cost | Measured before optimized; v2 (IVM/CDC) is a defined upgrade path, not a rewrite, because scopes and read sets are the stable interface |
| Silent seam bugs (cycle 3's class) | Differential conformance for the relay; shared-fixture conformance for the sync stores; owned domain boundaries over store rows in every client |
| Scope creep back toward a vendor substrate | §9.1's written FOSS rule + this directory's audit trail; any future candidate passes the license filter before anyone spends a week on its architecture |
