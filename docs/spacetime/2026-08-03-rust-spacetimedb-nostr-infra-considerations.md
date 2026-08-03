# Rebuilding the Nostr Relay and Adjacent Infra in Rust/SpacetimeDB: Considerations, Practicalities, and a Roadmap

Date: 2026-08-03
Author: repository analysis (Claude), commissioned by the owner
Status: **own analysis — recommendation, not implementation authority.** Nothing
here dispatches work, admits a packet, or changes roadmap priority.
Grounded at: `f5f5e3c85f` (current `main`)

Companions in this directory:

- [`2026-08-03-spacetimedb-historical-usage-audit.md`](2026-08-03-spacetimedb-historical-usage-audit.md) — the three prior adoption/removal cycles
- [`analysis.md`](analysis.md) — external assessment: SpacetimeDB for the Rust/Zed + TanStack + React Native stack
- [`analysis-nostr-relay.md`](analysis-nostr-relay.md) — external assessment: SpacetimeDB as a Nostr relay backend

Evidence base beyond those: the Buzz teardown
(`docs/teardowns/2026-07-21-buzz-teardown.md`), the Linear All Work adaptation
(`docs/teardowns/2026-08-02-linear-agents-openagents-nostr-adaptation.md`), the
sovereign relay audit
(`docs/omega/2026-07-25-web-omega-mobile-workroom-and-sovereign-relay-audit.md`),
the NIP adoption survey (`docs/omega/2026-07-24-nip-adoption-candidates.md`),
the owned relay deploy runbook
(`docs/ops/2026-07-24-owned-nostr-relay-deploy.md`), the Sarah Nostr cutover
stage machine (`docs/omega/2026-07-24-sarah-nostr-cutover.md`), the Omega
roadmap (`docs/omega/ROADMAP.md`), and root `INVARIANTS.md`.

## 0. The question, stated correctly

The two external assessments answer *"could SpacetimeDB do this job?"* and both
answer yes with conditions. That was worth knowing, but it is not the decision.
The decision is:

> Given what OpenAgents has already built, measured, mandated, and retired —
> should any of the Nostr relay, the sync layer, the workroom state plane, or
> the All Work event store be rebuilt on Rust/SpacetimeDB, and if not now, what
> would have to become true first?

My answer, in one paragraph: **do not rebuild the relay or any sync-adjacent
infrastructure on SpacetimeDB now.** The measured relay bottleneck is not a
database problem; the one capability SpacetimeDB uniquely sells is the one
OpenAgents has already built twice in-house and runs today; the commercial
relay product the roadmap actually wants is the use-case SpacetimeDB's license
most directly threatens; and a fourth adoption would have the shape of cycle 1
(replacing a working in-house sync lane — the worst-fit cycle), not cycle 3
(greenfield world state — the best-fit one). Separately, **"Rust" and
"SpacetimeDB" are independent decisions that `analysis-nostr-relay.md`
couples**, and decoupling them dissolves most of the apparent tension. What
*should* be taken from the assessments is a short list of host-agnostic
architecture ideas (§6) and a set of falsifiable reopening gates (§7).

## 1. Ground truth: the Nostr estate as it exists today

Any honest analysis has to start from how much already exists, because the
external assessments were written as if for a greenfield.

**A live, load-proven owned relay.** `relay.openagents.com` runs the
`nostr-effect` Node 24 host (pinned `7707334`) on Cloud Run against a Cloud
SQL Postgres event store, with NIP-42 auth and NIP-11 advertisement. The
2026-07-25 load proof accepted 3,710/3,710 events with zero rejections across
10–120 concurrent sockets, sustained ~185 events/second *as a floor* (still
rising at 120 connections), and survived a forced revision restart with
durable read-back. Its measured failure mode is **connection admission** —
connect p99 rose to 7,065 ms at 120 sockets while publish p99 stayed near
637 ms and acceptance held at 100%
(`docs/ops/2026-07-24-owned-nostr-relay-deploy.md` §9.5).

**A protocol library far ahead of the product.** `nostr-effect` implements the
full committed standard set (NIP-01, 09, 11, 17, 29, 32, 34, 40, 42, 44, 46,
58, 59, 65, 70, 77, 85, 86), all fifteen Buzz custom NIPs, and every Tier-1/2
candidate in the adoption survey. The survey's headline: *"the library is far
ahead of the product… the cost is wiring and a decision, not a protocol
implementation"* (`docs/omega/2026-07-24-nip-adoption-candidates.md` §1).

**A working local-first sync layer.** `@openagentsinc/khala-sync-client`
provides one local-store core across Node SQLite, Expo SQLite, and web
SQLite-WASM (SharedWorker + OPFS + Web Locks): confirmed state, cursors,
tombstones, an offline queue, optimistic overlays, rebase, and typed transport
failures. This matters enormously for the SpacetimeDB question, because
**durable offline behavior is precisely what `analysis.md` scores 3/10
natively and says you must build yourself** — and OpenAgents already built it.

**A settled authority doctrine.** The Omega roadmap partitions the planes:
Nostr owns the Sarah conversation record on the owned relay (post-cutover;
stage machine admitted, production flag off), Khala Sync keeps that role for
every other surface, and — invariant, repeated across every document — *relay
acceptance is never an OpenAgents admission; a signed event proves key and
bytes, never permission, execution, or truth.* The All Work architecture
(`…linear-agents-openagents-nostr-adaptation.md` §9) fixes the pipeline:
command gateway → Cloud SQL canonical event + projection transaction → Khala
Sync deltas + a durable Nostr outbox with retry and per-relay loss accounting.

**A commercial direction for the relay itself.** The sovereign relay audit
recommends a *managed private workroom relay* as the first commercial wedge —
dedicated database per design partner, signed export and offline verification,
then a customer-operated deployment kit, then a hybrid plane
(`…sovereign-relay-audit.md` §12).

**Standing fences.** `apps/nostr-relay` and `apps/openagents-world` are
retired, guard-enforced paths (`scripts/google-cloud-authority-guard.mjs`);
Google Cloud is the sole infrastructure authority; the workspace contract
routes Nostr primitive work to `nostr-effect` first ("use `nostr-effect`
directly or extend it first instead of rebuilding parallel Nostr
primitives"); reviving Cloudflare or SpacetimeDB world-service authority is an
explicit Omega non-goal; and new Rust surfaces in this monorepo require
explicit owner direction, with the Cloud crates and `oa-desktop-audio` as the
only current exceptions.

**Three prior SpacetimeDB cycles**, all removed, none rejected on merits —
with the durable output each time being the *contract*, not the host (the
historical audit's §7).

## 2. Rust ≠ SpacetimeDB: decouple the decisions

`analysis-nostr-relay.md` presents one bundled proposal — a Rust gateway over a
SpacetimeDB module. But the bundle hides that its two halves have independent
justifications and independent costs:

| Motive | What actually satisfies it | Needs SpacetimeDB? |
| --- | --- | --- |
| Relay performance / connection admission | More gateway instances, session affinity, an indexed subscription matcher | No — gateway concern |
| Rust for the relay hot path | A Rust gateway or a Buzz-shaped Rust relay (Axum + Postgres + Redis, proven in the teardown) | No |
| Transactional admission (dedup, replaceable heads, tombstones) | A single-writer transaction — Postgres does this today in `nostr-effect` | No |
| Historical `REQ` queries | An indexed relational/KV store — the assessment itself concludes this must live *outside* SpacetimeDB (its Choice C) | No |
| Multi-gateway committed-event fanout | A monotonic `ingest_seq` + LISTEN/NOTIFY or the existing outbox pattern | No |
| Live typed client replicas across Rust/TS clients | SpacetimeDB's actual differentiator | **Yes — but Khala Sync already occupies this role** |
| One shared transactional world state (MMO-shape) | SpacetimeDB's other differentiator | Yes — but the Verse is a non-goal today |

Read this way, the striking result is that **every relay-shaped motive is
satisfiable without the vendor**, on stacks the repo already runs or has
already proven (Buzz's relay is exactly the Rust-relay existence proof:
Axum, `spawn_blocking` Schnorr verification, idempotent Postgres insert with
`ON CONFLICT DO NOTHING`, Redis pub/sub fanout, generated-column FTS — plus
TLA+ runtime conformance, which is *more* formal assurance than SpacetimeDB
offers). The two motives that genuinely need SpacetimeDB are exactly the two
the current roadmap has either already solved in-house or explicitly fenced
off.

## 3. Candidate-by-candidate assessment

### 3.1 The Nostr relay — do not rebuild on SpacetimeDB

Four reasons, in decreasing order of decisiveness.

**(a) The measured bottleneck is upstream of the database.** The load proof
shows the relay degrading on *connection admission* while writes stay fast and
100% accepted. `analysis-nostr-relay.md`'s architecture puts connection
handling in the gateway on both the current and proposed stacks — so the
component SpacetimeDB would replace is the component that is not the problem.
The scaling lever named in the runbook (instances, concurrency, session
affinity) is available on Cloud Run today.

**(b) The commercial wedge collides with the license.** The BSL 1.1
Additional Use Grant permits production use with *no more than one instance*
and prohibits providing a "Database Service." The sovereign relay audit's
recommended product is a **managed relay service with a dedicated database per
design partner**, followed by a **customer-operated deployment kit**. On
SpacetimeDB that is: multiple production instances (per-partner isolation),
operated as a service, plus shipping customers a kit to run their own — three
separate collisions with the grant, or at minimum three questions for
Clockwork Labs and counsel *before the first design partner signs*. On
Postgres these questions do not exist. A commercial roadmap should not carry a
licensing dependency in its critical path when the alternative is the stack
already deployed.

**(c) Hosting shape regresses to the pattern that killed cycle 3.**
SpacetimeDB is a stateful, in-memory, persistent-disk singleton — it cannot
run on Cloud Run, which is the repo's deployment surface for the relay today.
It means a GCE VM with systemd, nginx, certificates, disk management, and an
operator runbook: *precisely* the `spacetimedb-world-1` operational lane whose
seam count the cycle-3 post-mortem cited as the reason to leave, and whose
cleanup left the dangling DNS record in Finding 1 of the historical audit. The
current relay redeploys as a Cloud Run revision and proved durable read-back
across a forced restart during its first load test.

**(d) It would be a parallel Nostr primitive.** The workspace contract routes
relay/NIP/event work through `nostr-effect` first. A Rust SpacetimeDB module
implementing NIP-01/09/40/42 admission semantics is a second implementation of
what `nostr-effect` already implements and serves — reintroducing the exact
dual-implementation drift the contract rule exists to prevent (and which the
Buzz teardown lists among its own weaknesses).

What *is* worth doing on the relay is in §6.

### 3.2 The sync layer / workroom hot-state plane — do not; this is the cycle-1 trap

This is where `analysis.md` is most enthusiastic (9/10 Rust desktop, 9/10
online sync), and where the repo's own history is most instructive. Cycle 1
failed not because SpacetimeDB couldn't carry sync, but because **replacing a
working in-house sync lane with a vendor equivalent is a lateral move that
adds a vendor, a module toolchain, and a deployment surface to reimplement
guarantees the system already has** (historical audit §7). In February that
working lane was Khala's `(topic, seq)` transport. Today it is Khala Sync —
more capable than the 2026-02 lane, with the offline/outbox/rebase layer that
SpacetimeDB *still* lacks natively (its own open feature request, cited in
`analysis.md`, asks for exactly this).

Adding SpacetimeDB here would create a **third** synchronization plane
alongside Khala Sync and the Nostr record — while the settled doctrine
partitions exactly two. Every document from the cycle-1 plan ("no long-term
dual-primary sync planes") through the Buzz teardown ("the system must define
one direction of authority… not accept both a database row and a relay event
as independent truth") warns against precisely this.

The Rust-desktop-fit argument deserves one honest concession: Omega is a
Zed-fork, and `analysis.md` is right that SpacetimeDB's Rust SDK would remove
the generated-TS-binding seam that produced cycle 3's silent-empty-world bug.
But Omega already consumes Khala Sync projections and signed Nostr events
through owned typed contracts, and the seam-bug class is addressed by owning
the contract — which the repo now does — not only by changing databases.

### 3.3 The All Work event store — do not; the projection law already picked its shape

The Linear adaptation's service architecture is: admission gateway → **Cloud
SQL canonical Work Event + projection transaction** → Khala Sync + durable
Nostr outbox, with the projection law ("all clients derive from the same
accepted event sequence and cursor; optimistic UI cannot show a mutation as
confirmed before admission"). This is reducer-shaped discipline implemented in
Effect over Postgres. Substituting SpacetimeDB would buy atomic-transaction
semantics the design already has, at the cost of the license, the VM, the
memory ceiling on an archive that grows with every Work Event forever (the
never-compacted commit log is `analysis-nostr-relay.md`'s own §14 warning),
and a migration of the one store the whole product's authority model now
points at. There is no version of this trade that wins.

### 3.4 The Verse — the only genuine fit, and it is fenced off

The historical audit's assessment stands: multiplayer world state was the one
workload where SpacetimeDB earned its keep (21 tables, 27 reducers,
server-enforced movement limits, live in five days). If the Verse revives, a
cycle-3-shaped bounded adoption — presence and interaction only, business
truth stays in owned projections — is the only SpacetimeDB proposal in this
document I would take seriously. But the Omega 3D avatar harvest audit
currently specifies **Nostr-primary presence with optional cache or fanout
acceleration** and lists reviving SpacetimeDB world authority as a non-goal.
That is an owner decision, already made, and the ephemeral-presence path
(NIP-38 statuses, ephemeral kinds over the gateway bus — both surveyed and
implemented in `nostr-effect`) is a coherent alternative. §7 records what
would reopen this.

## 4. The Buzz lesson, applied to this question

Buzz is the strongest available evidence that a *relay can be the workspace* —
and the teardown's central decision was to adopt the protocol profile and
**reject the relay event log as product authority**. The four-posture ladder
it establishes (signed protocol edge → signed projection bus → admitted
collaboration input → relay as workspace) is the right frame for SpacetimeDB
too, because SpacetimeDB-as-substrate is structurally the fourth posture with
the relay swapped for a database: it makes the synchronized store the product
authority. OpenAgents chose the second posture — signed projections from a
canonical store — for Nostr, deliberately, twice (Buzz teardown §6.9.3; Linear
adaptation §3). Choosing the fourth posture for SpacetimeDB while refusing it
for Nostr would be architecturally incoherent: it would grant a BSL-licensed
in-memory database the authority position the team denied to an open,
exportable, signed event log.

One more Buzz datum worth keeping visible: Buzz achieved runtime formal
conformance (TLA+ trace replay against `MultiTenantRelay.tla`) on a plain
Rust/Axum/Postgres relay. Formal assurance of relay semantics — the thing that
would genuinely raise trust in a rebuilt relay — is available without any
database migration, and aligns with the workspace invariant discipline
("narrow the production contract, model the bounded state space, run the
checker").

## 5. Practicalities inventory

For completeness, the full cost surface a Rust/SpacetimeDB rebuild would have
to carry — most items sourced from the repo's own prior evidence:

| Practicality | Reality |
| --- | --- |
| License | BSL 1.1, one production instance, no "Database Service"; per-tenant relay product and customer kit both implicated; Enterprise/BYO-GCP conversation is a prerequisite, not a follow-up |
| Hosting | GCE VM (stateful, in-memory, persistent disk, WebSocket) — Cloud Run ineligible; resurrects the cycle-3 ops lane incl. certificate, disk, and decommission hygiene (see the dangling-DNS finding) |
| Memory ceiling | Hot state must fit host RAM; a relay archive and an All Work event log both grow unboundedly; commit log is never compacted |
| Rust surface policy | New Rust in this monorepo needs explicit owner direction; module could live in the Omega repo, but relay semantics belong to `nostr-effect` per the workspace routing rule |
| Client release trains | Module schema is a wire contract across Omega, web, and mobile with lagging store releases; N/N−1 dual-write migration discipline required (`analysis.md` §9) |
| Naming hygiene | The Khala→Spacetime rename produced self-contradictory ADRs in cycle 1; "Khala" is now load-bearing again (`packages/khala-sync*`); any adoption must never rename existing planes |
| Guard updates | `google-cloud-authority-guard.mjs`, the retired-path list, and INVARIANTS would all need deliberate amendment — these are policy changes, not incidental edits |
| Version churn | 2.7 broke TS-generated API casing; 2.7.1 shipped reconnect/migration/auth fixes — pin everything, own a compat layer |
| Team stack | The conversion contract is Node/pnpm/Vite Plus/Effect; a Rust module + Rust SDK client path re-splits the stack the 2026-06-09 rebuild unified |

## 6. What to take from the assessments — without the vendor

The external documents contain real engineering that should not be lost when
their headline recommendation is declined. Each of these is host-agnostic and
lands on the current stack:

1. **A monotonic `ingest_seq` in the relay's Postgres store.** The single best
   idea in `analysis-nostr-relay.md`. It gives gateway catch-up after
   disconnect, a race-free historical/`EOSE`/live handoff boundary, and
   deterministic multi-replica fanout — and it directly addresses the deploy
   runbook's *unverified* item: NIP-77 and subscription fanout under two
   instances (§9.4). This is the highest-value relay hardening available.
2. **The `RelayQueryEngine` seam.** Formalize the bounded query interface
   (filters, cursor, budget: max events/rows/bytes/deadline) inside
   `nostr-effect`'s relay core, so the historical-query engine can later be
   swapped (Postgres today; LMDB/Tantivy projection if a public relay ever
   demands it) without touching admission or the gateway.
3. **Ephemeral bypass as an invariant.** Kinds 20000–29999 must never reach
   the durable store — enforce it in the relay core with a test, not as a
   convention. (Buzz's relay routes ephemeral kinds before the insert; same
   shape.)
4. **Admission as one transaction.** Dedup, replaceable-head compare
   (timestamp then lexicographic event-ID tie), and tombstone application —
   including the deletion-before-event resurrection case — atomically in the
   Postgres store, with the conformance fixtures `analysis-nostr-relay.md`
   §17 enumerates. Most of this exists in `nostr-effect`; the fixture matrix
   is the missing part worth porting from the assessment verbatim.
5. **A gateway-level indexed `SubscriptionIndex`** (by ID/author/kind/tag,
   broad, authenticated-recipient) replacing linear per-connection filter
   scans — relevant the moment the workroom product brings real concurrent
   subscription counts, and independent of any storage decision.
6. **Contract-first, always.** The one lesson all three cycles agree on: the
   durable artifact is the typed contract (`world-contract` survived two
   hosts; `(stream_id, seq)` survived its transport twice). The All Work
   event families and the relay admission semantics should live as
   host-agnostic schema + fixture packages from day one, so that *any* future
   host decision — including a SpacetimeDB one — is an implementation swap,
   not a rewrite.

## 7. Falsifiable reopening gates

To keep this from becoming a fourth undocumented verdict (the failure mode
Finding 2 of the historical audit records), here is what would legitimately
reopen the SpacetimeDB question. **All gates in a group must hold.**

**Gate group A — Verse revival (cycle-3 shape, the only strong fit):**
- A1. The owner reopens real-time world/presence as a product surface, and the
  Nostr-primary presence design measurably fails it (position-update rates or
  interaction volumes that signed-event or ephemeral-bus paths cannot carry).
- A2. Scope is presence/interaction only; business, training, and settlement
  truth stay in owned projections (the cycle-3 boundary that held).
- A3. An Enterprise/BYO-GCP license resolves the instance-count and
  Database-Service questions in writing.
- A4. The typed contract exists first, in an owned package, with the
  SpacetimeDB module as one host behind it.

**Gate group B — relay (weak fit; expect this never to fire):**
- B1. The Postgres store fails admission-transaction throughput *after* the
  `ingest_seq` + gateway-index work of §6, at measured load, with the failure
  in the store rather than admission or fanout.
- B2. The commercial relay product's licensing posture is resolved for
  per-tenant instances and the customer kit.
- B3. A GCE stateful ops lane is accepted by the owner as a standing cost,
  with decommission hygiene specified up front (the DNS lesson).

**Gate group C — any adoption at all:**
- C1. A ProductSpec and admitted packet exist naming SpacetimeDB, per current
  authority rules — no adoption arrives as a side effect of an audit or
  assessment, which is how cycles 1 and 2 arrived.
- C2. The historical audit and this document are cited in that packet, so the
  fourth adoption starts from the record of the first three.

## 8. Recommended roadmap

Near term (relay hardening, current stack — candidate packets, not dispatch):

1. `ingest_seq` in the `nostr-effect` Postgres store + gateway catch-up +
   two-instance fanout verification (closes runbook §9.4's open item).
2. NIP-11/NIP-42 advertisement mismatch (already tracked as
   `OpenAgentsInc/nostr-effect#169`) and the conformance fixture matrix from
   `analysis-nostr-relay.md` §17 ported into the relay test suite.
3. The `RelayQueryEngine` + budget seam, and the ephemeral-bypass invariant
   test.

Mid term (product, already directionally admitted elsewhere):

4. The All Work durable Nostr outbox exactly as the Linear adaptation
   specifies — signed safe projections, retry, per-relay loss accounting —
   over Cloud SQL canonical state. This, not a database swap, is what makes
   the Nostr estate real for work objects.
5. The sovereign relay wedge on the current stack, where per-tenant isolation
   is a Cloud SQL database per partner — no license questions, one deploy
   surface, and export/portability proofs as designed.

Long term:

6. Revisit only through §7's gates. If Gate group A ever fires, the adoption
   is Verse-shaped, contract-first, Omega-hosted on the Rust SDK — and scoped
   so that removing it a fourth time, if it comes to that, is once again
   cheap.

## 9. Incidental findings

1. **INVARIANTS.md line ~137 names `apps/nostr-relay` as the current isolated
   Effect-version exception** ("through `nostr-effect@0.0.12` only") while the
   same file and the infrastructure guard record `apps/nostr-relay` as a
   deleted, retired path, and the deploy runbook (Option A) places the relay
   host in the `nostr-effect` repository. The exception clause appears
   vestigial and worth a cleanup pass so the invariant text stops naming a
   path the guard forbids.
2. The stray untracked `node_modules` directories at both retired paths
   (`apps/nostr-relay/`, `apps/openagents-world/`) noted in the earlier
   analyses remain present in the canonical checkout.
3. The NIP-31 divergence (both the parity plan and the memory audit still
   cite an upstream-unrecommended NIP as fallback guidance) is already
   recorded in the NIP adoption survey §6 and still needs its decision.
