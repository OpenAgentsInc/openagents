# Rust/SpacetimeDB Build Plan: Nostr Relay and Core Infra

Date: 2026-08-03 (rewritten same day at owner direction: Rust is the decided
direction — this is the path forward, not a should-we assessment)
Status: build plan and roadmap — candidate packets, not yet admitted dispatch
Grounded at: `35faceea7a` (current `main`)

Companions: [`analysis-nostr-relay.md`](analysis-nostr-relay.md) (relay
architecture this plan executes), [`analysis.md`](analysis.md) (client/state
model), [`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md)
(prior cycles — mined here for reusable assets and cutover technique, and for
the specific operational mistakes this plan must not repeat).

## 0. The plan in one page

Three tracks, ordered so Rust ships immediately and nothing waits on a vendor
conversation:

```text
Track R — Rust relay (starts now, ships in weeks)
  R0  crates/oa-relay: greenfield Rust gateway over the EXISTING Cloud SQL
      event store → replaces the Node host; fixes the measured
      connection-admission limit; zero data migration; Cloud Run deploy;
      differential conformance against nostr-effect on the same database
  R1  SpacetimeDB module owns admission + committed-event fanout (shadow)
  R2  Cutover: STDB = hot authority; Postgres = historical query projection
      (it never dies — it becomes the RelayQueryEngine backend, which the
      relay needs anyway)

Track S — SpacetimeDB substrate (starts now, in parallel)
  S0  GCE VM + systemd + pinned 2.7.1 + backup/restore drill + runbook
  S1  License resolution with Clockwork Labs (the long pole — start day 1)
  S2  openagents-core database: workroom / All Work hot state, Omega on the
      native Rust SDK, web/mobile on the TS SDK

Track B — Blossom media store in Rust (independent, needed regardless)
  B0  crates/oa-blossom: NIP-B7 server over GCS — the artifact layer the NIP
      survey already flagged as required for Full Auto evidence
```

The sequencing principle: **the Rust rewrite does not wait for SpacetimeDB,
and SpacetimeDB does not block on the relay.** R0 is pure Rust against
infrastructure that already exists and is already load-proven. The STDB pieces
join through a trait seam (`RelayState`) when the VM, module, and license are
ready. If any STDB item slips, Rust relay progress is unaffected.

## 1. Fork `nostr-rs-relay` or greenfield?

**Greenfield. Quarry `nostr-rs-relay` for fixtures and edge cases; do not fork
it.** The decision follows from measuring what a fork would actually carry
versus what we would keep.

What our relay must be, given the opinionation this plan encodes:

- a **gateway/store split** behind a `RelayState` trait, so Postgres and the
  SpacetimeDB module are swappable backends;
- **multi-gateway fanout** off a committed, `ingest_seq`-ordered feed, with
  fail-closed catch-up;
- **admission as one transaction** (dedup, replaceable heads, tombstones,
  policy, entitlements) that later moves into STDB reducers unchanged;
- **typed OpenAgents projections** (agent profiles, work units, receipts)
  written in the same admission transaction;
- an **indexed `SubscriptionIndex`** rather than per-connection linear filter
  scans;
- **differential conformance against `nostr-effect`** as a build gate;
- Buzz-profile custom NIPs and our policy/entitlement model.

Now measure `nostr-rs-relay` (v0.10.0) against that list. Its architecture is
the opposite shape on every load-bearing axis: the `NostrRepo` trait is
explicitly a *streaming-SQL-query* interface (its core read operation is
"convert Nostr filters to dynamic SQL, stream rows through a channel"); its
live path is a single **process-local** Tokio broadcast — no cross-instance
fanout exists; each connection linearly scans its own subscription list per
event; SQLite is the primary store with PostgreSQL still experimental; its
in-memory filter code **still contains prefix matching that current NIP-01
removed**; and its payment/admission logic is coupled directly to relay
storage. `analysis-nostr-relay.md` §16 lists the replace-column, and it is the
entire core: `NostrRepo`, the SQL query generators, the writer channel, the
global broadcast, the migration machinery, the monolithic server ownership.

A fork where you replace the repository trait, the storage engines, the query
planner, the live path, the subscription model, and the policy coupling is not
a fork — it is a greenfield build wearing someone else's directory layout,
paying three real costs for zero structural benefit: you inherit an
architecture that fights the `RelayState` seam (the whole point of R0→R2 is a
storage swap the upstream shape cannot express); upstream pulls become
worthless after week one because the diff is the codebase; and every future
contributor has to learn which half of the tree is live versus vestigial.

What `nostr-rs-relay` *is* excellent for — and this is real value, taken
deliberately (MIT, with attribution):

| Take | Into |
| --- | --- |
| Protocol parsing and canonical-event validation tests | `oa-relay-conformance` fixtures |
| Filter-matching test corpus (minus prefix cases) | `nostr-domain` tests |
| Replaceable/deletion edge-case fixtures, incl. hidden-then-arrives | conformance matrix |
| NIP-42 challenge/timing behavior and its tests | gateway auth module tests |
| WebSocket edge-case handling (oversized frames, slow clients) | gateway hardening checklist |
| Rate-limit and query-budget concepts, Prometheus metric names | gateway + ops parity |
| Its hand-written SQLite query-planner heuristics | `RelayQueryEngine` design notes for the R2 indexer |

Two more inputs settle it. First, **Buzz's relay is the closer architectural
reference** for what we are building — Axum, multi-tenant, agent-native,
idempotent Postgres insert, connection semaphore/heartbeat discipline, runtime
TLA+ conformance — and we already hold a deep teardown of it; nobody proposes
forking Buzz either, because reference-not-substrate is the established
pattern. Second, the wire-primitive layer (event/filter/tag types, Schnorr,
NIP serialization) doesn't need to come from either codebase: the MIT
`rust-nostr` crates are the standard dependency for exactly that, evaluated in
packet 1 — a dependency, not a fork, with domain rules staying owned in
`nostr-domain` regardless.

So the formula is: **rust-nostr as a wire-primitive dependency (pending week-1
evaluation), `nostr-rs-relay` and Buzz as fixture/technique quarries, and an
owned greenfield core** whose shape is the `RelayState`/`RelayQueryEngine`
seams this plan needs. That is also the posture the workspace already applies
to every reference repo in `projects/`: study, port ideas, don't vendor trees.

## 2. What we build on (assets already in hand)

| Asset | Use in this plan |
| --- | --- |
| `relay.openagents.com` Cloud SQL Postgres event store | R0's storage backend as-is; later the historical query projection. Zero migration to start |
| `nostr-effect` (full NIP set incl. 15 Buzz NIPs) | Conformance oracle: differential-test the Rust relay against it on identical inputs; remains the TS client library |
| Buzz relay teardown (`docs/teardowns/2026-07-21-buzz-teardown.md`) | Proven Rust relay shapes: `spawn_blocking` Schnorr verify, `ON CONFLICT DO NOTHING` idempotent insert, connection semaphore, heartbeat/slow-client policy, generated-column FTS, TLA+ runtime conformance |
| `nostr-rs-relay` (MIT) | Fixture and edge-case quarry per §1 — not a fork base |
| `analysis-nostr-relay.md` §§4–10, 16 | The gateway/module split, schema, reducer semantics, EOSE handoff, crate layout — adopted below nearly verbatim |
| Cycle-1 parity harness concept (`scripts/spacetime/parity-chaos-gate.sh`, recoverable at `9b2b978949`) | Template for the R1 shadow parity gate |
| Cycle-3 GCP runbook (recoverable at `3ee0785f51^`) | Base for the S0 VM runbook — with its recorded defects fixed (sudo-broken wrapper, missing decommission checklist) |
| Load-proof harness (`packages/sarah` load-proof) | Reused as the Rust relay's acceptance benchmark; current floor to beat: 185 ev/s, connect p99 7,065 ms at 120 sockets |
| All Work event families (`docs/teardowns/2026-08-02-linear-agents…` §9) | The S2 table model's source of truth |
| Khala Sync client (Node/Expo/web stores) | Stays serving existing surfaces; new All Work hot state lands on STDB greenfield — no migration of live data, one-way ratchet per object family |

## 3. Track R — the Rust relay

### R0: Rust gateway over the existing store (weeks 1–3)

New crates in the existing monorepo Cargo workspace (the workspace already
carries `oa-*` infra crates; this extends that pattern — it does not touch the
retired `apps/nostr-relay` path):

```text
crates/oa-relay/
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

Key decisions, made now so nobody relitigates them mid-build:

1. **Greenfield core, per §1.** rust-nostr evaluated as the wire-primitive
   dependency in week 1; domain rules (replacement, deletion, policy) stay
   owned in `nostr-domain` either way, so the semantics have one home.
2. **Schema:** R0 reads/writes the existing event tables so the TS and Rust
   relays can run against the same database simultaneously. The only additive
   migration is `ingest_seq BIGSERIAL` + the compound indexes from
   `analysis-nostr-relay.md` §10 that are missing.
3. **`RelayState` is a trait from day 1** — `oa-relay-store-pg` is the first
   impl, the STDB module client is the second. The gateway never knows which
   is behind it. This is the seam that makes R1/R2 a swap, not a rewrite.
4. **Ephemeral kinds (20000–29999) never touch the store.** In-process
   broadcast for the single-gateway deploy; the bus becomes NATS Core only
   when gateway replicas > 1 requires it.
5. **Conformance is differential.** Every fixture runs against both
   `nostr-effect` and the Rust relay; divergence is a build failure. This is
   how we get the TS library's five months of NIP correctness into the Rust
   relay for free — and it is the direct answer to cycle 3's silent-seam bug
   class.

Deploy: Cloud Run (the gateway is stateless; the store is Cloud SQL — no VM
needed for R0), shadow hostname first (`relay-rs.openagents.com` or internal),
then the production cutover below.

**R0 exit gates (ready-gates, all measurable):**

- Conformance matrix green, differential vs `nostr-effect`: duplicate races,
  replaceable same-timestamp tie, addressable `d`-tags, deletion-before-event,
  NIP-40 expiry, NIP-42/70 enforcement, filter AND/OR/limit/order.
- Load proof exceeds the current floor: ≥3× events/second and connect p99
  under 1 s at 120 sockets (the Node host's measured failure point).
- Race-free EOSE proven under concurrent publish (the buffered-handoff test).
- Forced-restart durability read-back, same as the 2026-07-25 proof.

**R0 cutover:** point `relay.openagents.com` at the Rust gateway; keep the
Node revision warm for one week as rollback; then retire it. Decommission
checklist written *before* cutover — DNS records, certificates, old revisions
— because the historical audit's Finding 1 (a dangling A record on a released
IP) is the exact class of loose end this plan does not leave.

### R1: SpacetimeDB module in shadow (weeks 3–6, gated on S0)

`modules/nostr-relay-stdb` (Rust, per `analysis-nostr-relay.md` §10):
`nostr_event` (with `ingest_seq`, `reverse_created_at`, `address_key`),
`nostr_indexed_tag` (denormalized compound indexes), `replaceable_head`,
`deletion_tombstone`, relay policy/entitlement tables, `admit_event` reducer
family, scheduled cleanup, and the `CommittedNostrEvent` event table for
gateway fanout.

The gateway gains `oa-relay-store-stdb`: the second `RelayState` impl over the
STDB Rust SDK — admission via reducer call, live feed via the event table,
catch-up via `ingest_seq` range reads.

Shadow mode: gateway dual-writes both stores; a parity daemon (pattern ported
from the cycle-1 parity-chaos gate) compares admission outcomes, head states,
and sequences continuously. Divergence pages; no silent drift.

### R2: authority cutover (weeks 6–8)

- STDB module becomes admission authority and the live fanout source.
- Postgres demotes to the **historical query projection**: a `relay-indexer`
  consumes the committed sequence and maintains the `RelayQueryEngine`
  backend. This is `analysis-nostr-relay.md`'s own Choice C — the relay needs
  a dedicated query store *regardless*, so Postgres is not discarded; it is
  reassigned to the job it is best at (arbitrary NIP-01 filter queries,
  NIP-45 counts, NIP-50 search feed, NIP-77 sorted index).
- Multi-gateway: replicas share the committed feed; each keeps its own
  `SubscriptionIndex`; a gateway that cannot catch up fails closed (drops its
  sockets) rather than serving gaps.

**R2 exit gates:** two-gateway cross-delivery with no missing/duplicate
events; STDB restart replay time measured and within the recovery objective;
restore-from-snapshot drill passed; commit-log archival to GCS running;
rollback path (re-promote Postgres) rehearsed once for real.

## 4. Track S — the SpacetimeDB substrate

### S0: the VM, done right this time (week 1, parallel)

One GCE instance, one production STDB process, multiple databases —
`nostr-relay`, `openagents-core`, later `verse`:

```text
Project openagentsgemini, us-central1-a
Instance  oa-spacetime-1   (start e2-standard-8; watch RSS, resize deliberately)
Disk      oa-spacetime-data-1 (200 GB pd-ssd, /stdb)
Binary    pinned 2.7.1 exactly (2.7 broke TS codegen casing — pin everything)
Service   systemd; nginx exposes ONLY /v1/database/*/subscribe + /v1/identity
Access    IAP SSH for publish/admin; loopback listener
Backups   disk snapshots (scheduled) + commit-log segment upload to GCS
Monitoring RAM, disk, commit-log growth rate, reducer p99, subscriber lag
```

Runbook is the cycle-3 runbook rebased with its recorded defects fixed, plus
two sections it lacked: a **restore drill** (executed, not documented-only,
before any production traffic) and a **decommission checklist** (DNS, static
IP, certs, disks) written on day 1.

### S1: license resolution (day 1, longest lead time)

BSL 1.1 permits one production instance and prohibits offering a "Database
Service." Execution consequences, handled as engineering constraints:

- One production instance is what S0 deploys — compliant as-is for everything
  in this plan's first two months.
- The Enterprise/BYO-GCP conversation with Clockwork Labs starts immediately,
  because two roadmap items eventually exceed the grant: an active standby for
  the relay, and the sovereign-relay commercial offering (per-tenant
  instances / customer-operated kit). Neither is needed in the first eight
  weeks; both are needed after. Owner action: authorize the outreach.
  Recorded in `NEEDS_OWNER.md` when this plan is admitted.
- Until terms exist, the per-tenant sovereign offering ships on the Postgres
  lane (dedicated Cloud SQL database per partner — already the audit's
  recommended isolation) and moves to per-tenant STDB only under signed terms.

### S2: `openagents-core` — the product hot-state plane (weeks 4–8)

The workroom / All Work hot state, as one STDB database, modeled from the
Linear-adaptation event families and the `analysis.md` §12 table sketch:

```text
workspace / team / member / capability_grant
work / issue projection / thread / message metadata
agent / agent_run / run_activity (append-only, ingest-sequenced)
approval / decision / operation_receipt
worker / worker_lease / pending_job
presence + transient signals via event tables
```

Consumption:

- **Omega first.** The Rust SDK client lands in the Omega repo behind an owned
  domain-model boundary (the `SyncRepository` layering from `analysis.md` §2
  — generated rows never leak into GPUI code). This is the payoff of the
  whole plan: the primary surface gets typed, live, transactional state in
  its native language with no codegen seam.
- Web (`apps/start`) and mobile via the TS SDK; mobile gets the connection
  controller (`AppState`/NetInfo/foreground-reconnect/outbox) that
  `analysis.md` §2 specifies — that adapter is part of this track, not an
  afterthought.
- **Authority boundary is unchanged:** the All Work admission gateway still
  admits commands; reducers are the mutation path for hot state; Cloud SQL
  remains the canonical Work Event archive per the projection law; the
  durable Nostr outbox still publishes signed safe projections. STDB is the
  live operational plane, not a new source of settlement/billing/release
  truth — same boundary cycle 3 proved workable (presence/interaction in
  STDB, business truth in owned projections).
- **Data placement rule (memory ceiling, enforced from day 1):** hot
  operational rows in STDB; full archives, raw traces, and large artifacts in
  Cloud SQL/GCS with digest references. Run activities carry `artifact_ref`,
  never payloads. Streamed model output batches into bounded rows, never
  per-token transactions.
- **Khala Sync:** keeps every surface it serves today. New All Work objects
  are greenfield on STDB — no live-data migration, no dual authority per
  object family, each later port is its own bounded packet. The Sarah Nostr
  cutover proceeds on its existing stage machine, untouched.

## 5. Track B — Blossom media store in Rust (weeks 2–4, independent)

`crates/oa-blossom`: NIP-B7 server over GCS, NIP-98 auth, NIP-94 metadata.
The NIP survey already established the need (NIP-44 caps payloads at 64 KB;
Full Auto evidence — diffs, logs, test output — does not fit in events). This
is a small, clean, self-contained Rust service with no STDB dependency: a good
first-blood crate for the relay team while R0 conformance is being built, and
it completes the artifact story that both the relay and All Work reference.

## 6. Packet breakdown (candidate issues, in order)

| # | Packet | Track | Depends on |
| --- | --- | --- | --- |
| 1 | `oa-relay` workspace scaffold + `nostr-domain` + rust-nostr evaluation | R0 | — |
| 2 | `oa-relay-store-pg`: admission transaction + `ingest_seq` migration | R0 | 1 |
| 3 | Gateway: NIP-01/11/42, SubscriptionIndex, EOSE handoff, ephemeral bus | R0 | 1 |
| 4 | Conformance crate: fixture matrix (incl. nostr-rs-relay quarry) + differential runner vs nostr-effect | R0 | 2, 3 |
| 5 | Shadow deploy + load proof + R0 cutover of `relay.openagents.com` | R0 | 4 |
| 6 | `oa-spacetime-1` VM, runbook, backup + restore drill | S0 | — |
| 7 | Clockwork license outreach (owner-gated) | S1 | — |
| 8 | `oa-blossom` NIP-B7/98/94 over GCS | B0 | — |
| 9 | `nostr-relay-stdb` module + `oa-relay-store-stdb` + parity daemon | R1 | 5, 6 |
| 10 | R2 cutover: STDB authority, Postgres → query projection, indexer | R2 | 9 |
| 11 | `openagents-core` schema + reducers + Omega Rust SDK client | S2 | 6 |
| 12 | Web/mobile TS SDK integration + mobile connection controller | S2 | 11 |
| 13 | Nostr outbox publisher daemon (Rust) for All Work safe projections | S2 | 11 |
| 14 | Verse revival on the same instance (owner-gated; cycle-3 scope) | — | 10, 11 |

Packets 1–8 have no cross-dependencies beyond what is listed and can run as
parallel lanes under the normal claim protocol. Admission of the plan itself
is one owner decision; each packet then lands end-to-end with its own
verification.

## 7. Housekeeping the plan requires (same change, not later)

1. **Contract updates land with packet 1:** AGENTS/INVARIANTS language for the
   new Rust surfaces (`crates/oa-relay*`, `oa-blossom`, the STDB module) —
   the owner's Rust direction is the authority; the docs must say so, so no
   future agent treats the crates as policy violations. The
   `nostr-effect`-first routing rule gets amended to name the Rust relay as
   the relay implementation home, with `nostr-effect` as TS client library
   and conformance oracle.
2. **INVARIANTS line ~137** still names the retired `apps/nostr-relay` as the
   isolated Effect-version exception — delete the vestigial clause in the
   same pass.
3. **Naming discipline:** the vendor is "SpacetimeDB"/"STDB" in every
   document, never "Spacetime" bare — cycle 1's Khala/Spacetime rename left
   ADRs reading "doctrine moves from Spacetime to Spacetime," and "Khala" is
   load-bearing again today. No renames of existing planes, ever, as part of
   this program.
4. **Stray dirs:** remove the untracked `node_modules` at
   `apps/nostr-relay/` and `apps/openagents-world/` in the canonical checkout.

## 8. Risks and their handling

| Risk | Handling |
| --- | --- |
| STDB memory ceiling on a growing relay archive | R2 makes Postgres the archive/query store; STDB holds the hot operational set with expiry sweeps — the split is the architecture, not a mitigation bolted on |
| License terms arrive slower than the build | Nothing in weeks 1–8 needs more than one instance; the sovereign per-tenant offering has a Postgres lane until terms sign |
| Module schema migrations vs three shipped clients | Schema-as-wire-contract discipline from `analysis.md` §9: additive first, dual-write window, N/N−1 client tests in CI, bindings generated in CI for Rust and TS on every module change |
| Version churn (2.7 casing break) | Everything pinned together: host, module, Rust SDK, TS SDK, CLI; upgrades are their own tested packet |
| VM as single failure domain | Measured restart-replay time + snapshot/commit-log archival + rehearsed restore before production authority; standby instance follows the license resolution |
| Silent generated-binding seams (cycle 3's bug class) | Differential conformance against `nostr-effect` for the relay; owned domain-model boundary over generated rows for clients; both are exit gates, not aspirations |
| Fourth-cycle drift (build fast, delete faster) | Every packet lands with its contract in an owned package; the historical audit showed contracts are what survive — so the contracts are the deliverable and hosts stay swappable behind them |
