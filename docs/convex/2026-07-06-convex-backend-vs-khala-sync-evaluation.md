---
status: evaluation
last_updated: 2026-07-06
owner: OpenAgents
author: engineering audit
subject: Should self-hosted Convex (get-convex/convex-backend) replace Khala Sync?
related:
  - ../khala-sync/SPEC.md
  - ../khala-sync/README.md
  - ../khala-sync/CVR_DESIGN.md
  - ../fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md
  - ../cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md
evidence:
  convex_repo: projects/repos/convex-backend @ 39bd49bc8b4237936c8e044d460d59b937cb61c5 (get-convex/convex-backend, main, 2026-07-07)
  convex_license: LICENSE.md — FSL-1.1-Apache-2.0 (Functional Source License)
---

# Convex self-hosted backend vs. Khala Sync — replacement evaluation

**STATUS (2026-07-08): SUPERSEDED by the GCP/Cloud SQL + Khala Sync
stack.** Kept as the historical record of the earlier decision; do
not implement from this document.


## 0. Executive summary

**Recommendation: DO NOT ADOPT Convex as a replacement for Khala Sync.**
Confidence: **high** for the "do not replace" verdict; medium-high for the
narrower "reference / partial-idea-adoption is fine" carve-out.

Convex is an impressive, battle-tested reactive backend, and its sync model is
literally one of the reference designs Khala Sync was built against
(`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md` §4.2
already cites Convex's "invalidate-and-re-execute on recorded read sets" and its
"A Map of Sync" checklist). But adopting the *product* now would reverse a
decision the team just made, on purpose, one week ago, and would re-introduce
the exact class of risk the owner just paid to escape.

The five decisive reasons:

1. **Licensing is a structural mismatch, not a footnote.** Convex is
   **FSL-1.1-Apache-2.0** (`LICENSE.md`), a source-available "Functional Source
   License," *not* OSI open source. Its whole point is to forbid a **Competing
   Use** — "offering the same or substantially similar functionality." A
   company whose product *is* a sync/collaboration/agent-data platform is
   exactly the actor that license is written to constrain. Internal use to power
   our own app is a Permitted Purpose, but the boundary between "internal use"
   and "substantially similar functionality offered to others" runs straight
   through the middle of our roadmap (Khala Sync as a data layer other people's
   agents/clients consume). It converts to Apache-2.0 only two years after each
   release. This is a legal-review-required dependency at the core of the
   product. (§5)

2. **It re-introduces exactly the lock-in the owner just cut.** The owner's
   2026-07-06 directive (`docs/cloud/.../consolidation-audit.md`) is: stop
   renting opaque vendor control-planes, own "opinionated, hot-swappable
   implementations." Convex's value *is* a vendor-shaped runtime — a single
   backend process running the reactor + V8 isolates, with Postgres demoted to a
   dumb blob/row store. Swapping D1/Hyperdrive lock-in for Convex-runtime
   lock-in is the same trade the audit rejects.

3. **Impedance mismatch with our substrate.** Our services layer is **Effect +
   Bun**; our data layer commitment is **TanStack DB + `khala-sync-db-collection`**
   (owner mobile decision, repo `CLAUDE.md`). Convex functions run **untyped-to-us
   TypeScript in Convex's own deterministic V8 isolate** (`crates/isolate`), not
   Effect, not Bun, with no arbitrary npm and a determinism contract. Every
   mutator/query we own (fleet, chat, ledger, payment legs) would be rewritten
   into Convex's function model, losing Effect Schema contracts and Effect
   services.

4. **The migration is a full rewrite of the authoritative store, not a swap.**
   Convex owns its own schema and document format; Postgres is an internal
   implementation detail it manages, not a database we model. Our data (D1 today,
   Cloud SQL Postgres authoritative for sync) would be exported and re-imported
   into Convex-managed tables, and the money path (Spark/MDK mutators, ledger
   idempotency, CVR permission retraction) plus every projection would be
   re-expressed in Convex functions. It deletes ~51k non-test LOC of owned sync
   code but replaces it with a from-scratch reimplementation on a foreign runtime.
   T-shirt size: **XL**, touching the revenue path. (§6)

5. **Self-hosted Convex is single-node and free-tier-only; the scaling story is
   the hosted cloud we are told not to use.** The self-hosted backend is one
   container (`self-hosted/docker/docker-compose.yml`: one `backend`, one
   `dashboard`), serializing mutations through one process
   (`APPLICATION_MAX_CONCURRENT_MUTATIONS` default 16). The README/self-hosted
   guide is explicit that "the cloud-hosted product is optimized for scale" and
   self-host "supports all the **free-tier** features." Khala Sync's hub is
   deliberately a *rebuildable, horizontally shardable cache over Postgres*
   (`SPEC.md` §1) — a better fit for Cloud Run + Cloud SQL than a single stateful
   backend box.

**Where Convex still earns its place:** as a **reference implementation** for
the reactive query tier (it already is), and as a candidate **only if** the
company ever decides to *stop owning* the sync substrate and accept a
source-available vendor runtime as a strategic dependency — the opposite of the
current direction. See the decision boundary in §8.

---

## 1. What each system actually is

### 1.1 Khala Sync (ours)

Khala Sync is an owned Replicache/Zero/Figma-LiveGraph-class incremental sync
engine, specified in `docs/khala-sync/SPEC.md` (v0.1) and rationalized in
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`. Shape:

- **Authoritative store: Cloud SQL Postgres** (`khala-sync-pg`, PG17), tables
  `khala_sync_changelog` (transactional outbox, per-scope versions),
  `khala_sync_scopes`, `khala_sync_mutations` (per-client mutation ids +
  results), `khala_sync_client_state`. We model and own this schema.
- **Scope model:** a *scope* (`scope.user.<id>`, `scope.thread.<id>`,
  `scope.fleet_run.<id>`, `scope.team.<id>`, `scope.agent_run.<id>`) is the unit
  of sync, **authorization**, and fan-out. Server-side `resolveScopeRead`-style
  gating decides membership; revocation *retracts* already-synced rows.
- **Delivery: hub.** Today `KhalaSyncHubDO` (per-scope Durable Object,
  non-authoritative cache: recent log window in DO SQLite, hibernating
  WebSockets, resumable HTTP catch-up). The GCP replacement is
  `apps/khala-live-hub` — a **single-instance Cloud Run Bun WS service** that is an
  **in-memory per-scope window cache** (not a Postgres tail): it warms a bounded
  window with one `SELECT ... FROM khala_sync_changelog` on cold start, then
  receives new changes by **HTTP `POST /append`** and fans them out over WS
  (`hubFor` is the documented sharding extension point). Both hub deployments run
  during the CFG-5/#8520 cutover (capture pushes to a primary + optional mirror).
  The actual Postgres tail is the **capture daemon** in
  `packages/khala-sync-server/src/capture.ts` — a long-lived Bun process on a
  *direct* Postgres connection (never Hyperdrive, which drops LISTEN/NOTIFY),
  using `LISTEN khala_sync_changelog_append` as a wake signal with a 5s poll
  fallback, that `POST`s ordered batches to the hub `/append`.
- **Client engine** (`packages/khala-sync-client/src/session.ts`): a per-scope
  state machine `idle → bootstrapping → catching_up → live`, with `must_refetch`
  and terminal `denied`/`auth_rejected` phases. Durable cursor
  (`store.cursor(scope)`); reconnect always resumes catch-up from that cursor;
  delivery is at-least-once with idempotent apply (stale `DeltaFrame`s whose
  cursor ≤ current are skipped); TRANSIENT faults reconnect forever with jittered
  exponential backoff. Optimistic mutators with **rebase**; a flag-gated **CVR**
  (Client View Record) diff-pull path (`KHALA_SYNC_CVR=1`,
  `docs/khala-sync/CVR_DESIGN.md`) so permission changes retract rows without a
  full re-bootstrap.
- **Contracts:** `packages/khala-sync` — **Effect Schema** types for scopes,
  versions, changelog entries, mutations, wire protocol, errors. Single source of
  truth shared by server and every client.
- **TanStack DB integration:** `packages/khala-sync-db-collection` binds scopes
  to TanStack DB collections with optimistic mutations — the committed data layer
  for web, Electrobun desktop, and the Expo mobile app.
- **Consumers today:** Khala Code desktop (`clients/khala-code-desktop`, first
  wired consumer, fleet scope, default-on), Expo mobile
  (`clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts`), web.
- **Server routes:** the new Khala Sync path is 24 `khala-sync-*.ts` route files
  in `apps/openagents.com/workers/api/src/` (~6,960 non-test LOC:
  `khala-sync-bootstrap-routes.ts`, `-log-routes.ts`, `-push-routes.ts`,
  `-cvr-routes.ts`, `-connect-routes.ts`, the `KhalaSyncHubDO` in
  `khala-sync-hub-do.ts`, and `resolveScopeRead`/scope-auth wiring). Pushes land
  in Cloud SQL Postgres via `@openagentsinc/khala-sync-server`.

> **Two sync systems coexist — do not conflate them.** The **legacy** "Sync
> Outbox" (`@openagentsinc/sync-worker`/`sync-schema`, the `SyncOutboxStore` bound
> to **D1** via `OPENAGENTS_DB`, the `sync_changes` table, the `SYNC_ROOM` DO, and
> `sync-routes.ts`/`runtime.ts`) is a *separate, older* system already slated for
> retirement (#8420). **Khala Sync** is the owned Postgres→hub→SQLite substrate
> described here. "Replace Khala Sync" and "retire the legacy D1 outbox" are
> distinct decisions.

**Owned code footprint** (non-test LOC; deletion candidate if replaced):
`packages/khala-sync-server` ~29.4k (61 files — Postgres schema, migrations,
projections, capture, compaction, mutation-ledger, scope-auth), `khala-sync-*`
Worker routes ~7.0k, `packages/khala-sync-client` ~4.6k, `packages/khala-sync`
~3.2k, `clients/khala-mobile/src/sync` ~3.5k, `clients/khala-code-desktop`
sync service ~1.4k, `apps/khala-live-hub` ~1.4k, `khala-sync-db-collection`
~1.1k. **≈51.5k non-test LOC across ~148 files** (roughly double with tests) —
owned, invariant-registered (`apps/openagents.com/INVARIANTS.md`, "Khala Sync
(SPEC §7 invariant set)"), and load-tested
(`docs/khala-sync/2026-07-04-load-test-report.md`).

### 1.2 Convex self-hosted (theirs)

`projects/repos/convex-backend` @ `39bd49bc` (`get-convex/convex-backend`). It is
a Rust monorepo (`crates/`) plus TypeScript packages (`npm-packages/`) that ships
"the open-source reactive database." Shape:

- **Reactive query engine (`crates/sync`).** WebSocket protocol. The server holds
  `SyncedQuery` state (`crates/sync/src/state.rs`): each subscribed query has a
  `subscription` and an `invalidation_future`; when a mutation commits writes that
  overlap a query's recorded read-set, the invalidation future fires and the query
  re-executes, and a `Transition` is pushed to the client (`QuerySetModification`,
  `QuerySetVersion`, `SerializedQueryJournal`). This is coarse
  invalidate-and-re-execute, *not* incremental view maintenance — which is exactly
  the model our rationale doc praised as the right v1 for a query-subscription
  tier.
- **Function runtime (`crates/isolate`).** User queries/mutations/actions run as
  **TypeScript in V8 isolates** (`isolate.rs`, `udf_runtime.rs`,
  `environment/`). Queries and mutations are **deterministic and side-effect-free**
  by contract (that is what makes read-set tracking and re-execution sound);
  network/nondeterminism is confined to "actions." It is Convex's runtime, not a
  place to run arbitrary Effect programs or arbitrary npm.
- **Storage backends.** SQLite by default; **Postgres** (`crates/postgres`) or
  **MySQL** (`crates/mysql`) via `POSTGRES_URL` / `MYSQL_URL`
  (`self-hosted/advanced/postgres_or_mysql.md`; tested on PG17 / MySQL 8). But
  Convex **owns the schema** inside that database — it is a persistence target, not
  a data model we design or query directly. File/module/search/export storage can
  go to **S3-compatible** object storage (`crates/aws_s3`, `S3_ENDPOINT_URL` +
  `S3_STORAGE_*` buckets in the compose file). GCP caveats confirmed in the crates:
  **GCS has no native client** — it is reachable only via the S3-interoperability
  endpoint override (`S3_ENDPOINT_URL` + `AWS_S3_FORCE_PATH_STYLE`,
  `crates/aws_utils`), untested as a first-class target; **Cloud SQL Postgres**
  works over standard TCP+TLS (rustls, `crates/postgres`) but there is **no Cloud
  SQL IAM-auth / socket-connector** support. The only cloud-object dependency is
  the AWS S3 SDK; no DynamoDB/SQS/other AWS-managed-service coupling.
- **Auth (`crates/authentication`, `crates/keybroker`).** OIDC/JWT identity
  (`access_token_auth.rs`; JWKS handling in `keybroker`). Convex has **no
  row-level security**: all access control is enforced *inside* user-defined
  query/mutation functions via `ctx.auth.getUserIdentity()`. Every scope gate we
  have would become imperative checks inside Convex functions.
- **Client SDKs (`npm-packages/convex`).** First-class `convex/react`
  (`use_queries.ts`, `use_subscription.ts`, `use_paginated_query.ts`), optimistic
  updates via the local query cache, auth adapters for Clerk/Auth0
  (`react-clerk`, `react-auth0`). React Native/Expo is supported *in practice* via
  the same WebSocket browser client (no dedicated `react-native` export exists in
  the package). **Offline/local-first** is the `npm-packages/local-store` package —
  and it is **`version 0.0.0`, browser-only (Dexie/IndexedDB), React-only**; there
  is no evidence of a React Native local persistence story in-repo.
- **Deployment (`self-hosted/`).** `docker compose up` runs **two** services: one
  `backend` container and one `dashboard`. The backend is a **single process with a
  single writer**: exactly one `Committer` task (`crates/database/src/committer.rs`)
  serializes all commits and assigns monotonic timestamps (OCC conflict checks);
  the reactor (write log + subscription interval-map,
  `crates/database/src/subscription.rs` + `write_log.rs`) is in-process memory. You
  cannot run multiple backend replicas against one database — scale is **vertical
  only** (`APPLICATION_MAX_CONCURRENT_*` knobs tune in-process concurrency, default
  16). The self-hosted guide states self-host "supports all the **free-tier**
  features" and
  "the cloud-hosted product is optimized for scale." A minimal anonymized **beacon**
  phones home unless `--disable-beacon` is set.

---

## 2. Side-by-side capability comparison

| Capability | Khala Sync (owned) | Convex self-hosted | Verdict for us |
|---|---|---|---|
| **Reactive sync model** | Per-scope changelog + cursor, at-least-once + idempotent apply, resumable catch-up | Read-set invalidate-and-re-execute over WS subscriptions | Both strong; Convex's is the model we already emulate. **Even.** |
| **Offline / local-first** | Local SQLite store, durable cursor, reconnect-forever, rebase; works on desktop/mobile/web | `local-store` is v0.0.0, **browser-only (IndexedDB/Dexie), React-only**; core client needs a live WS connection | **Khala wins** for our RN + desktop + local-first requirement. |
| **Optimistic updates** | First-class, with server rebase + pending/confirmed visibility (KS-9.2) | Yes, via client query-cache optimistic updates | **Even.** |
| **Scoped / row-level auth** | Scope = unit of auth; `resolveScopeRead`, revocation retracts synced rows, CVR diff-pull | **No RLS**; all checks imperative inside functions; retraction is your code | **Khala wins** — our model is native, Convex makes us rebuild it in functions. |
| **Self-host on GCP** | Cloud Run (Bun WS hub) + Cloud SQL Postgres + GCS — the current plan | Single backend container on Cloud Run/GCE + Cloud SQL + GCS-via-S3-compat | Both *can*; Convex forces a stateful single-writer box. **Khala fits GCP better.** |
| **Client SDKs (RN/desktop/web)** | Our TanStack DB collection everywhere; Effect Schema contracts | Excellent React; RN via browser client; desktop via browser client; **no RN local-first** | **Khala wins** on our exact surface set. |
| **Storage / data ownership** | We own the Postgres schema; can `EXPLAIN`, index, back up, PITR | Convex owns the schema; DB is opaque-to-us persistence | **Khala wins** on inspectability the owner just prioritized. |
| **Function runtime fit** | Effect + Bun mutators, Effect Schema contracts | Convex V8 isolates, deterministic TS, no arbitrary npm, not Effect | **Khala wins** — Convex is an impedance mismatch. |
| **Ops burden** | We run: capture daemon, hub service, migrations, compaction (real work) | Run one backend + one dashboard container; upgrades are theirs | **Convex wins** — genuinely less to operate. |
| **Scale ceiling (self-host)** | Horizontal: stateless hub shards + Postgres | **Single-node backend**, free-tier features only; scale = their cloud | **Khala wins** for growth without adopting the hosted product. |
| **Licensing** | Ours, Apache-compatible internal code | **FSL-1.1** (source-available, non-compete), Apache only after 2 yrs | **Khala wins decisively** — see §5. |
| **Maturity / test rigor** | New (v0.1), our tests/invariants | Battle-tested, though "test frameworks are not part of the OSS offering" (README) | **Convex wins** on maturity. |

Net: Convex wins **ops burden** and **maturity**. Khala Sync wins **local-first,
scoped auth, client-surface fit, data ownership, runtime fit, self-host scaling,
and licensing** — i.e., every axis the current company direction weights most.

---

## 3. What Convex would simplify

Honest credit — these are real:

- **Delete the reactive plumbing.** The capture daemon, the changelog-window hub,
  the catch-up/live state machine, and cursor bookkeeping — a large share of the
  ~51k non-test LOC, concentrated in `packages/khala-sync-server` (~29k) and the
  client engine/hub — would largely disappear; Convex gives reactive queries for
  free. (This is the single strongest argument *for* Convex: it is a lot of owned
  code to carry.)
- **No hub to operate.** No `khala-live-hub` Bun WS service, no per-scope fan-out
  shard logic, no LISTEN/NOTIFY capture connection to babysit.
- **No compaction / CVR machinery to maintain.** Convex handles query
  invalidation internally; we would not need `CVR_DESIGN.md`'s read-set diffing.
- **A dashboard for free.** `crates` + `npm-packages/dashboard` gives a data
  browser/log viewer we would otherwise build.
- **Fewer sync edge cases to own.** Reconnect, dedupe, ordering — someone else's
  tested code.

## 3.1 What Convex would complicate or cannot do (for us)

- **Rewrite every mutator into Convex functions.** Fleet mutators, chat mutators,
  ledger idempotency, and the **money path** (Spark/MDK checkout, treasury, tips —
  `MUTATORS.md`'s single-transaction rule, replay safety, in-band rejection) all
  become Convex TS functions in a deterministic isolate. We lose Effect Schema
  contracts and Effect services at the mutation boundary.
- **Rebuild scope authorization + revocation-retraction** as imperative
  in-function checks; Convex has no native scope/RLS concept and no built-in
  "retract already-synced rows on access change" — that is our CVR design, which we
  would re-implement on top of Convex or lose.
- **Give up direct Postgres.** Cross-cutting SQL — joins across the ~419 legacy
  tables, `EXPLAIN`, ad hoc reporting, dual-write during the D1 exit, BigQuery
  export — assumes a database *we* model. Convex's DB is not that; the D1→Postgres
  migration plan (`MIGRATION_PLAN.md`) would have to become a D1→Convex plan with
  no direct SQL landing zone.
- **No RN local-first.** Mobile is a committed Expo target with offline
  expectations; Convex's `local-store` is browser-only v0.0.0.
- **Single-writer scaling.** One backend process is a throughput ceiling the
  hosted cloud is designed to relieve — and we are told not to use the cloud.
- **A second control-plane to reason about** at the very moment we are collapsing
  onto boring, inspectable GCP primitives.

---

## 4. Constraint-by-constraint scorecard

The task named five hard constraints. How Convex does against each:

1. **Runs on GCP, self-hosted, no new SaaS lock-in.** *Partial.* It can self-host
   on Cloud Run/GCE + Cloud SQL Postgres + GCS-via-S3-compat. But (a) it is a
   **single stateful backend**, an awkward Cloud Run shape (needs a persistent
   volume or careful single-instance config), and (b) the "no new lock-in" spirit
   is violated: you adopt the **Convex runtime** as the lock-in, and the scale
   path is their **cloud**. The owner cut Cloudflare specifically to stop renting
   opaque runtimes; Convex is another one. **Fails the spirit of the constraint.**
2. **Fits Effect/Bun/TanStack clients.** *Weak.* Great React SDK, workable RN/
   desktop via the WS client, but no Effect, no Bun functions, no RN local-first,
   and it displaces `khala-sync-db-collection` (the committed data layer).
3. **Scopes + fine-grained auth + optimistic + payment path.** *Weak.* No native
   scopes/RLS; all gating is imperative in functions; the money-path mutators are
   a rewrite into a determinism-constrained runtime. **Highest-risk area.**
4. **Migration cost from D1/Postgres.** *High.* Export/import into Convex-owned
   schema; loss of the direct-SQL migration landing zone; the in-flight D1 exit
   plan would be re-planned around Convex. See §6.
5. **Effect TS substrate.** *Mismatch.* Convex's function runtime is its own; our
   substrate is Effect everywhere. This is the deepest architectural friction.

---

## 5. Licensing / production-use analysis (potential dealbreaker)

**License: `LICENSE.md` = Functional Source License, Version 1.1, Apache 2.0
Future License (`FSL-1.1-Apache-2.0`), Copyright 2026 Convex, Inc.**

This is **source-available, not OSI open source.** The operative clauses:

- **License Grant / Permitted Purpose.** You may "use, copy, modify, create
  derivative works … and redistribute the Software for any **Permitted
  Purpose**." A Permitted Purpose is *any purpose other than a **Competing Use***.
- **Competing Use (the trap).** "making the Software available to others in a
  commercial product or service that: (1) substitutes for the Software; (2)
  substitutes for any other product or service we offer using the Software …; or
  (3) **offers the same or substantially similar functionality as the
  Software.**"
- **Explicitly permitted:** "for your **internal use and access**"; non-commercial
  education/research; and professional services to a compliant licensee.
- **Grant of Future License.** Each release relicenses to **Apache-2.0 on the
  second anniversary** of its availability.

**Why this matters for OpenAgents specifically.** Most companies embed Convex to
power an unrelated product (a store, a game) — clearly "internal use." OpenAgents
is different: our product roadmap *is* a sync/collaboration/agent-data platform
("Khala Sync" as a substrate that external users' agents and clients connect to,
per the repo `CLAUDE.md` "help a user connect their fleet to Khala" flows). The
closer we get to **exposing sync-as-a-capability to third parties**, the closer we
get to Convex's clauses (2) and (3) — "substitutes for a product we offer" /
"substantially similar functionality." That is a fact-specific legal question, not
an engineering one, and it sits at the *core* of the product rather than a leaf
dependency.

**Verdict:** the license is **not an automatic dealbreaker for pure internal use**,
but it is a **serious, legal-review-required constraint** that would need sign-off
before Convex touched a customer-facing surface, and it is fundamentally at odds
with the owner's stated goal of **owned, hot-swappable, freely-relicensable
infrastructure.** Combined with §2's technical findings, it turns a "maybe" into a
"no" for replacement. (Contrast: Khala Sync is ours, with no such restriction.)

*Note:* the self-hosted build also ships an anonymized **beacon**
(deployment id, migration version, git rev, uptime) unless run with
`--disable-beacon`. Minor, but it is another "phones home by default" behavior the
owner would want off.

---

## 6. Migration effort & risks

**T-shirt size: XL.** This is not a library swap; it is re-hosting the
authoritative store and rewriting the mutation/authorization layer.

Rough decomposition:

- **Data migration (L):** export D1 + Cloud SQL authoritative tables → import into
  Convex-owned schema; re-model documents; re-establish indexes. The in-flight
  D1→Postgres cutover (`MIGRATION_PLAN.md`) becomes a D1→Convex cutover with no
  direct-SQL landing zone for the ~419 legacy tables.
- **Mutator/query rewrite (L):** every scope's projections and mutators (fleet,
  chat, ledger, **payment legs**) reimplemented as Convex deterministic TS
  functions; re-prove idempotency and single-transaction semantics in Convex's
  model; drop Effect Schema at the boundary.
- **Auth/scope rewrite (M):** rebuild scope resolution + revocation-retraction as
  in-function checks + Convex-side patterns; re-validate against
  `apps/openagents.com/INVARIANTS.md` "Khala Sync (SPEC §7 invariant set)".
- **Client rewrite (M):** replace `khala-sync-db-collection` / TanStack DB
  bindings with `convex/react` (+ RN adaptation); rebuild any offline behavior
  mobile relies on (Convex has none for RN).
- **Ops (M):** stand up a single-writer stateful backend on GCP, wire GCS-as-S3,
  dashboard, backups/PITR of a schema we no longer control.

**Biggest risks:**

- **Money path in a rewritten runtime.** The highest-care surface (Spark/MDK
  checkout, treasury, tips, ledger idempotency) would be re-expressed in a new
  execution model. Highest blast radius, lowest tolerance for subtle regressions.
- **Single-writer throughput ceiling** discovered under load, with the only fix
  being the hosted product we are told not to buy.
- **Licensing surprise** if a sync-as-a-capability product surface later trips the
  Competing-Use clause after we are already dependent.
- **Reversing a one-week-old, deliberate decision** — sunk design, invariants,
  load tests (`docs/khala-sync/2026-07-04-load-test-report.md`) and a live cutover
  already in progress.

---

## 7. What we should take from Convex anyway (partial adoption)

We do not need to adopt the product to benefit:

- **Keep it as the reference for the reactive tier.** The read-set
  invalidate-and-re-execute model (`crates/sync/src/state.rs`:
  `SyncedQuery`/`invalidation_future`) is precisely the v1/v2 split our rationale
  doc endorsed. If we ever add server-computed reactive *queries* (beyond scoped
  changelog tailing), copy this design, not the code.
- **"A Map of Sync" as a requirements checklist.** Already cited in
  `docs/fable/2026-07-04-...` §4.2 — run new scopes through its nine axes.
- **Determinism boundary discipline.** Convex's hard split between deterministic
  queries/mutations and side-effecting "actions" is a good lens for our mutator
  rules (`MUTATORS.md`).
- **Dashboard ideas** for a Khala Sync data/log browser.

All of this is FSL-safe (reading and learning is a Permitted Purpose; we write our
own implementation in our own stack).

---

## 8. Decision boundary

**Adopt Convex (replace Khala Sync) only if ALL of these flip:**

- The company decides it **does not want to own** the sync substrate and will
  accept a source-available, non-compete vendor runtime as a strategic core
  dependency (reversing the 2026-07-06 direction).
- Legal signs off that our roadmap's sync-as-a-capability surfaces do **not**
  constitute a Competing Use under FSL-1.1.
- We accept the Convex **function runtime** in place of Effect/Bun at the data
  layer, and re-tool the money-path mutators accordingly.
- Mobile's local-first requirement is dropped, or we build RN offline on top of
  Convex ourselves.
- We accept a single-writer self-host and are willing to move to Convex's **cloud**
  when we outgrow it (or invest in scaling their backend ourselves).

**Do NOT adopt (the current reality) if ANY of these hold — and today they all
do:**

- Owner direction is "own opinionated, hot-swappable, cloud-agnostic
  infrastructure; no new vendor runtimes." ✔ holds
- Effect/Bun/TanStack is the committed substrate and `khala-sync-db-collection`
  is the committed data layer. ✔ holds
- Fine-grained scope auth + revocation-retraction + optimistic + **payment path**
  are first-class and safety-critical. ✔ holds
- Local-first on Expo RN and Electrobun desktop is required. ✔ holds
- We are mid-evacuation and want *fewer* control-planes, not a new one. ✔ holds
- Khala Sync already exists, is specced, load-tested, invariant-gated, and has a
  live cutover in progress. ✔ holds

**Conclusion: keep building Khala Sync. Use Convex as a design reference, not a
dependency.** The one thing to *not* do is treat "Convex has a nice reactive
engine" as a reason to undo a sound, recent, deliberate architectural decision at
the core of the product and the money path.

---

## Appendix A — key evidence paths

Convex (`projects/repos/convex-backend` @ `39bd49bc`):
- `LICENSE.md` — FSL-1.1-Apache-2.0
- `README.md`, `self-hosted/README.md` — self-host = free-tier features; cloud for scale; beacon
- `self-hosted/docker/docker-compose.yml` — one `backend` + one `dashboard`; single-writer knobs; S3/Postgres/MySQL env
- `self-hosted/advanced/postgres_or_mysql.md` — Postgres/MySQL support (PG17/MySQL8)
- `crates/sync/src/state.rs`, `worker.rs` — WS subscriptions, read-set invalidation/re-execution
- `crates/isolate/` — V8-isolate deterministic TS function runtime
- `crates/authentication/`, `crates/keybroker/` — OIDC/JWT; access control in functions (no RLS)
- `crates/postgres/`, `crates/mysql/`, `crates/sqlite/`, `crates/aws_s3/` — storage backends
- `npm-packages/convex/src/react/` — React client, optimistic updates
- `npm-packages/local-store/` — offline package, **v0.0.0, browser-only (Dexie), React-only**

Khala Sync (`openagents`):
- `docs/khala-sync/SPEC.md`, `README.md`, `CVR_DESIGN.md`, `MUTATORS.md`, `MIGRATION_PLAN.md`, `RUNBOOK.md`
- `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md` (§4.2 already evaluates Convex's model)
- `docs/cloud/2026-07-06-cloudflare-to-google-consolidation-audit.md` (§3.6 hub swap; owner direction)
- `packages/khala-sync/`, `packages/khala-sync-client/src/session.ts`, `packages/khala-sync-db-collection/`, `apps/khala-live-hub/`
- `apps/openagents.com/workers/api/src/sync-routes.ts`, `khala-sync-connect-routes.ts`
- `clients/khala-code-desktop/`, `clients/khala-mobile/src/sync/use-khala-sync-scope-entities.ts`
</content>
