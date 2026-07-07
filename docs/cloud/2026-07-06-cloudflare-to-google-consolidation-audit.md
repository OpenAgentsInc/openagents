---
status: canonical
last_updated: 2026-07-06
owner: OpenAgents
related:
  - ../../../docs/cloud/backend-strategy.md (workspace root — SHC/GCP/Cloudflare substrate split)
  - ../../../docs/cloud/shc-pilot.md
  - ../khala-sync/RUNBOOK.md
  - ../cleanup/2026-07-04-repo-wide-cleanup-and-sync-adoption-audit.md
  - ../adr/0004-prefer-cloudflare-native-product-infrastructure.md (superseded by this audit's direction)
---

# Cloudflare → Google Cloud consolidation audit

**Owner directive (2026-07-06):** stop paying Cloudflare for infrastructure that is
not in the revenue-critical path. We hold **$70k+ in Google Cloud credit**; burn
that before paying any other vendor. The previously envisioned Cloudflare-native
agent platform (Workers-for-Platforms Sites, Agents SDK ambitions, DO-everything)
is **not currently needed**; it is acceptable to postpone the Sites program.
Longer-term, the primitives Cloudflare sells should become **our own opinionated,
hot-swappable implementations** that run on big clouds (GCP now, AWS-able later)
or on Sovereign Hybrid Compute (SHC) own-metal — not on vendor-specific runtimes.

This audit covers: (1) the complete current Cloudflare footprint, (2) what already
runs on Google, (3) a service-by-service migration analysis, (4) whether the Rivet
stack (`projects/repos/{rivet,sandbox-agent,agent-os}`) fits as the vendor-neutral
replacement layer, (5) IaC/Kubernetes/Terraform posture, (6) database rigor, and
(7) a phased plan with what gets postponed.

---

## 0. Why now — three lock-in incidents in one week

The abstract argument for vendor neutrality became concrete three times in the
last few days:

1. **D1 lock/contention (pre-cutover):** production D1 saturation locked core
   tables until the sync-engine cutover relieved it. D1 gives no knobs — no
   pooling control, no replicas, no `EXPLAIN` access worth the name. The fix was
   *leaving D1* (Khala Sync on Cloud SQL Postgres), not tuning it.
2. **Hyperdrive write-loss (#8409, 2026-07-05):** a silently degraded Hyperdrive
   pool dropped **~77–79%/hour of Postgres writes**. Raising
   `origin_connection_limit` did nothing; the only fix was destroying and
   recreating the opaque Hyperdrive config. Hyperdrive exists *only* because
   Workers cannot hold direct TCP connections to our own database — it is pure
   lock-in tax, and it failed in the worst possible way (silent data loss).
3. **Analytics Engine deploy freeze (2026-07-06, today):** the account-level
   Analytics Engine feature flipped off and **every `wrangler deploy` (prod and
   staging) started failing with code 10089** — because the Worker *declares* an
   AE binding, an account-side toggle froze all backend shipping, including an
   urgent mobile-auth fix. A fail-soft metrics dataset held the entire deploy
   pipeline hostage.

Each incident shares a root cause: **critical-path behavior gated on opaque
Cloudflare control-plane state we cannot inspect, pool, or replace.** On GCP (or
any VM/container substrate) the equivalents are boring, inspectable systems:
pgbouncer we configure, Postgres we can `EXPLAIN`, deploys that don't consult a
product-upsell flag.

ADR-0004 ("prefer Cloudflare-native product infrastructure") is **superseded by
this audit's direction** and should get a successor ADR when Phase 1 lands.

---

## 1. Complete Cloudflare footprint (2026-07-06)

Six Workers + the full paid-feature menu. Billing-relevant surface in **bold**.

| App / Worker | Domains | Cloudflare services used |
|---|---|---|
| `openagents-autopilot` (+ `openagents-staging`) — the monolith | `openagents.com`, `auth.openagents.com`, `sites.openagents.com` | **D1** (~419 tables, still primary authority), **KV** (`AUTH_STORAGE` sessions), **R2** (`ARTIFACTS`), **Durable Objects** ×7 classes, **Queues** ×4, **Analytics Engine**, **Hyperdrive** →Cloud SQL, **Browser Rendering** (QA), **Containers** (`MdkSidecarContainer`, `MdkTreasuryContainer`, `MdkTipsBufferContainer` — the MONEY PATH: Spark/MDK checkout, treasury, tips), **Workers for Platforms** dispatch (`openagents-sites-production`), **Email send**, cron (every minute, 24 tasks), service binding → relay, static assets |
| `openagents-world` | workers.dev | D1 (`WORLD_DB`), `RegionDurableObject` (Verse realtime), Queue, cron |
| `openagents-market-relay` | `relay.openagents.com`, `nexus.openagents.com` | `NostrRelayDO` (SQLite DO = the whole relay) |
| `openagents-aiur` | `aiur.openagents.com` | assets + vars only (proxy to main API) |
| `openagents-forge` | `forge.openagents.com` | assets + vars only |
| `openagents-com-start-staging` | — | assets only |

DO classes on the monolith: `KhalaSyncHubDO` (per-scope sync hub — cache/fan-out,
Postgres authoritative), `SyncRoomDurableObject` (**legacy** sync spine, retirement
already planned, #8420), `DurableInferenceStreamObject` (resumable inference
streams), `AgentDefinitionSchedulerDurableObject`, `EventLedgerOwnerDurableObject`,
plus the three MDK container classes.

**Deep couplings worth naming:**
- **Sites program** = Workers-for-Platforms dispatch namespace + Cloudflare
  Custom Hostnames API + Cloudflare Containers preview runners + WfP payment
  middleware. This is the single most Cloudflare-shaped product we have.
  → **Owner-approved: postpone.**
- **MDK treasury containers** = production money movement on Cloudflare
  Containers (beta product, billed per vCPU-second). Highest-care migration item.
- **`auth.openagents.com`** = OpenAuth issuer inside the monolith Worker, session
  state in KV.

## 2. What already runs on Google (project `openagentsgemini`)

We are further along than "someday": **the data authority and the compute muscle
are already on GCP.** Cloudflare is increasingly just the edge shell.

- **Cloud SQL Postgres ×4** — incl. `khala-sync-pg` (PG17, db-custom-8-53248):
  the Khala Sync engine's authoritative store. The strategic D1 exit is already
  in motion (dual-writes across ~14 domains; reads cutting over domain by domain).
- **Cloud Run ×14 services** — incl. `oa-updates` (updates.openagents.com — our
  own EAS-Updates replacement), `oa-cloud-run-bridge` (agent-computer control),
  l402 stack, legacy web frontends.
- **GCE ×11 instances** — Hydralisk GPU inference fleet lanes, agent-computer
  Firecracker host (`agent-computer-gce-1`), `oa-codex-control-1`, nexus fleet,
  `oa-bitcoind`, and the SHC-pattern fallback node `oa-gcp-shc-katy-01`.
- **GCS buckets ×9**, **Secret Manager** (release signing), Cloud Build.
- **No GKE clusters** (deliberate — see §6).

Plus SHC own-metal: `oa-shc-katy-01` (Katy TX) runs `oa-node` + `oa-workroomd` +
Firecracker microVMs + `oa-codex-control` today. The workspace-root
`docs/cloud/backend-strategy.md` substrate split already says: **SHC primary for
bursty low/medium-trust agent execution, GCP for durable/sensitive/canonical,
Cloudflare optional edge.** This audit is the execution plan for shrinking that
"optional edge" to near-zero billed surface.

---

## 3. Service-by-service migration analysis

The monolith is a **Bun + Effect** TypeScript app. That matters: almost none of
the application code is Workers-specific — the Workers runtime is reached through
bindings (`env.X`). The migration shape is: **run the same Bun app on Cloud Run**,
swap each binding for an owned interface (§5), and keep route/domain parity.

| # | Cloudflare service | What it does for us | Google-consolidated replacement | Effort | Notes |
|---|---|---|---|---|---|
| 1 | Workers (monolith compute) | All API routes, auth issuer, forum, sites API, sync push/pull | **Cloud Run** (Bun server, min-instances≥1, us-central1) behind a Global External LB | L | Biggest single move. Bun+Effect code ports cleanly; the work is the binding seams below + request-context plumbing (`ctx.waitUntil` → background tasks / Cloud Tasks). Staging = second Cloud Run service. |
| 2 | D1 (`OPENAGENTS_DB`) | Legacy primary authority (~391 referenced tables) | **Cloud SQL Postgres** — *already the plan* (Khala Sync migration plan). Accelerate: dual-write is done for ~14 domains; finish read cutovers, then bulk-migrate remaining cold tables with a one-time D1 export → Postgres import | L (in progress) | The D1 exit stops being incremental once the Worker itself moves: on Cloud Run there is NO D1 binding, so the monolith move sets a hard deadline for full D1 evacuation. Sequence: cut reads → freeze writes per domain → export/import cold tables → drop. |
| 3 | Hyperdrive | TCP-to-Postgres shim for Workers | **Delete.** Cloud Run sits in-VPC with the Cloud SQL connector + **pgbouncer** (or Cloud SQL managed pooling). Direct connections restore LISTEN/NOTIFY, advisory locks, session prepared statements — everything Hyperdrive's transaction-mode pooling forbids | S | Kills the #8409 failure class outright. The capture daemon already uses direct connections; the Worker request path was the only Hyperdrive consumer. |
| 4 | KV (`AUTH_STORAGE`) | OpenAuth session/token storage | **Memorystore Redis** (or a `kv` Postgres table with TTL sweep — simpler, one less system) | S | OpenAuth storage adapter is pluggable; we already implement its storage interface. Recommend Postgres-table first (zero new infra), Redis only if latency demands. |
| 5 | R2 (`ARTIFACTS`) | QA/Codex raw artifacts, event chunks | **GCS** (S3-compatible XML API exists; better: native client behind our own `BlobStore` interface) | S | One-time `rclone` copy for history. Keep bucket-per-purpose naming parity. |
| 6 | Durable Objects — `KhalaSyncHubDO` | Per-scope live WS fan-out + hot catch-up window (cache only; PG authoritative) | **Own hub service on Cloud Run** (WebSockets are supported; session affinity per scope via consistent-hash on scope id) *or* **Rivet Actors** (§4). Postgres stays authoritative either way; the hub is a rebuildable cache by design — the sync engine was explicitly architected so this layer is swappable | M | This is the "update the sync engine to work all on Google and not use durable objects" item. Because the DO is already non-authoritative, the swap is contained: reimplement changelog-window + hibernating-socket semantics in a plain Bun WS server keyed by scope. Capture daemon repoints its hub-append calls. |
| 7 | Durable Objects — `SyncRoomDurableObject` | Legacy live push (homepage counter, team chat, thread files) | **Retire, don't port** — #8420 already plans draining these consumers onto Khala Sync. Accelerate that instead of migrating it | M (existing plan) | Do NOT rebuild this on GCP. Finishing the retirement removes a whole DO class for free. |
| 8 | Durable Objects — `DurableInferenceStreamObject` | Resumable inference stream offsets | Redis Streams / Postgres append table keyed by request id, same offset-resume contract | S–M | Contract is small (append, read-from-offset, done marker). Postgres table + `SELECT ... WHERE offset > $1` preserves the public resume API exactly. |
| 9 | Durable Objects — scheduler + event-ledger singletons | Serialization points (cron ticks, owner event ingest) | Postgres **advisory locks** (available again once Hyperdrive dies) or single-instance Cloud Run jobs | S | These DOs exist only to serialize; advisory locks are the boring correct tool. |
| 10 | Queues ×4 | runner events, adjutant enrichment, event-ledger ingest, codex metadata | **Cloud Tasks / Pub/Sub**, or — simpler and more owned — a **Postgres job queue** (`SELECT ... FOR UPDATE SKIP LOCKED`) worked by Cloud Run workers | S–M | All four consumers are batch-1 or batch-25 with modest volume. A single owned `JobQueue` interface (§5) backed by Postgres removes a vendor service entirely; Pub/Sub remains the escape hatch at scale. |
| 11 | Analytics Engine | Khala Sync compare-soak metrics (fail-soft) | **Drop the binding now** (it froze deploys today); write soak metrics to a Postgres table or BigQuery if we ever need SQL-over-metrics | XS | Immediate action regardless of the rest: removing the AE binding un-hostages deploys. |
| 12 | Browser Rendering | QA-runner headless Chrome | **Playwright/Chromium container on Cloud Run** (per-invocation or warm pool) | S | We already run Maestro/sim harnesses; this is a container image, not a platform feature. |
| 13 | Cloudflare Containers — MDK treasury/sidecar/tips | **Production money path** (Spark/MDK wallets) | **Cloud Run (min-instances=1, single-concurrency) or a dedicated GCE VM** with the same container images; state already lives in the wallet/daemon layer | M, **highest care** | Migrate LAST with a rehearsed cutover: bring up on GCP against staging, verify Spark connectivity + persistence, drain CF container, flip. Never run two live treasuries on one mnemonic (existing invariant). |
| 14 | Workers for Platforms + Custom Hostnames + Sites containers | Sites program (per-customer sites, domains, previews, checkout middleware) | **POSTPONED** per owner. When revived: per-site static/SSR bundles on Cloud Run + Cloud CDN, custom domains via cert-manager/Google-managed certs, previews on our Firecracker agent computers | — | This is the one genuinely Cloudflare-*shaped* product. Do not port it 1:1; redesign against owned primitives when it re-enters the critical path. Cancel the WfP namespace + custom-hostname billing when Sites tenants are archived. |
| 15 | Email send binding | Transactional email (Resend addresses already) | **Resend API directly** (we already pay/route through Resend identities) | XS | The binding is a thin convenience; direct API removes it. |
| 16 | Cron triggers | 24 scheduled tasks, every minute | **Cloud Scheduler → Cloud Run endpoint** (same `scheduled()` dispatch table) | XS | One scheduler job hitting `/internal/cron` preserves the existing task table. |
| 17 | nostr-relay (`NostrRelayDO`) | Whole relay in one SQLite DO | Container on Cloud Run/GCE + Postgres (or fold into monolith service). Nostr relays are commodity self-host software | S–M | Low coupling to the rest; can move independently, anytime. |
| 18 | openagents-world (`RegionDurableObject`) | Verse realtime regions | **Postpone with Sites** (audit already flags it "smells parked") or Rivet Actor if Verse revives | — | Don't spend migration effort on a parked product. |
| 19 | aiur / forge / start (assets-only Workers) | Static shells + owner proxy | Cloud Run static serving or GCS+LB; aiur's proxy logic is a 1-file Bun server | XS each | Trivial. |
| 20 | DNS + edge (domains, TLS, WAF, CDN) | All `*.openagents.com` domains terminate on CF | **Two options.** (a) Keep Cloudflare **free tier** as DNS+proxy only in front of Google LB origins — $0, keeps WAF/DDoS, zero registrar churn; (b) full exit: Cloud DNS + Global External LB + Cloud Armor + Cloud CDN | XS (a) / S (b) | Recommend (a) initially: the goal is ending *paid platform* dependence, and CF-free-DNS-in-front-of-GCP is a standard, non-lock-in posture. Move to (b) only if we want zero CF presence. |

**What the bill drops to:** after Phases 1–3 below, the paid Cloudflare surface
(Workers Paid, D1, DO storage/requests, Queues, R2, Analytics Engine, Browser
Rendering, Containers, WfP + custom hostnames) goes to zero; optional remainder
is the free DNS/proxy tier. All replacement spend lands on the $70k GCP credit.

> **Update 2026-07-07 (CFG-16 #8532) — post-cutover CF teardown done, one gap found.**
> With `openagents.com`/`auth`/`aiur` serving from Cloud Run (Google TLS) and
> DNS off Cloudflare, the orphaned monolith Workers were confirmed
> non-serving (`openagents-autopilot` + `openagents-staging`: 0 custom
> domains, 0 zone routes, DNS off CF) and **deleted**, which unblocked the
> long-blocked destructive cleanup: the **8 CF Queues** (4 prod + 4 staging
> CFG-7 lanes) are gone, the **`openagents-sites-production` WfP dispatch
> namespace** (0 scripts) is gone, and the orphaned **MDK checkout sidecar
> container** `a03cb880-…` is gone. `relay.openagents.com` (worker
> `openagents-market-relay`) and the out-of-scope `openagents-inference-batch-jobs`
> / `openagents-world-bridge` queues were left intact. Billing is cancelled,
> so any residual orphan costs nothing.
>
> **Row 9 gap (ledger DO → advisory locks) is NOT actually implemented.** Row 9
> claims "advisory locks replace the scheduler/ledger DOs," but on Cloud Run
> `EVENT_LEDGER_OWNER` is still a *typed-unavailable DO stub*
> (`cloudrun/do-shims.ts`) with **no** oa-infra `Mutex`/`pg_advisory` +
> Postgres sequence-store replacement wired. So an `event-ledger-ingest` job
> delivered to the Cloud Run monolith rejects with `BindingUnavailableError`
> → 500 → the pump nacks → dead-letter. `EventLedgerOwnerSequenceStore` has
> only a DO-SQLite impl; `Mutex` is not yet integrated in the monolith. This
> is a real CFG-9 completion item (own lane): add a Postgres/Mutex-backed
> owner sequence store + serialize per `ownerAgentUserId`, apply the
> migration, redeploy `openagents-monolith`, then prove one job
> lease→deliver→serialized-ingest→ack. Tracked on #8532.

---

## 4. Rivet stack evaluation (`projects/repos/{rivet,sandbox-agent,agent-os}`)

All three are one org (`rivet-dev`), Apache-2.0, Rust+TS. Verdict: **credible,
genuinely vendor-neutral — adopt selectively, don't bet the platform on it.**

**rivet (engine v2.3.2, mature, active):** stateful **Actors** with co-located
SQLite/KV state, durable **Workflows** (Gasoline), per-actor **queues**, timers,
WebSockets, hibernation — near-1:1 Durable Objects semantics, but your actor code
runs in **your own Node/Bun process on your own VMs/K8s**. Self-host = single
Rust binary; storage is pluggable (**Postgres or RocksDB — no FoundationDB
required**); full-prod topology adds NATS + ClickHouse. Ships k8s manifests and
compose files. This is exactly the "vendor-neutral Cloudflare" the owner
described.

**sandbox-agent (v0.5-rc):** a universal **control plane** for coding agents
(Claude Code/Codex/OpenCode adapters behind one HTTP+SSE API) — *not* a sandbox.
Isolation is BYO (it lists E2B/Daytona/Docker/**our own Firecracker**). Useful as
a normalization layer over our agent-computer fleet; irrelevant to the CF exit.

**agent-os (v0.2.6):** in-process V8/WASM "agent OS" (virtual FS, WASM coreutils,
Pyodide, deny-by-default permissions, ~6ms cold start). Browser-grade isolation,
not VM-grade — complements, doesn't replace, our Firecracker microVMs.

**Fit against our needs:**

| Our need | Rivet answer | Take |
|---|---|---|
| DO replacement (sync hub, streams, schedulers) | Rivet Actors | Strong fit technically. BUT our hubs are deliberately *non-authoritative caches over Postgres* — a plain Bun WS service (§3.6) covers it with zero new stateful infra. Adopt Rivet **if** we want actor semantics as a product primitive (agent sessions, per-user agents), not merely to replace two cache DOs. |
| Queues/workflows | Gasoline + actor queues | Fine, but Postgres SKIP LOCKED covers current volume with no new system. |
| Sandboxes | none (BYO) / agent-os V8 | We already own the stronger answer: Firecracker agent computers on GCE + SHC. |
| Risk | — | Companion repos are pre-1.0 with no compat guarantees; UniversalDB-over-Postgres and Epoxy are bespoke, less battle-tested than Rivet Cloud's FDB path; small-org bus factor. Swapping CF lock-in for rivet-dev coupling is only acceptable because it's Apache-2.0 and runs on our metal. |

**Recommendation:** Phase 1–2 need **no Rivet** — owned thin primitives on
Postgres/GCS/Cloud Run are smaller and fully ours. Slot Rivet in as the **actor
runtime for the agent-platform tier** (per-user persistent agents, workrooms,
Verse-if-revived) in Phase 4, run on GCE/GKE with the Postgres driver, once we
actually need thousands of stateful addressable entities rather than two cache
classes. Track sandbox-agent as the possible normalization API over agent
computers.

---

## 5. Owned, opinionated, hot-swappable primitives

The durable lesson from D1/Hyperdrive/AE week: **application code must depend on
our interfaces, never a vendor SDK.** We already practice this in spots (the sync
hub is a cache by design; `oa-updates` reimplemented EAS Updates; the treasury is
a container image). Make it doctrine. One package, e.g.
`packages/oa-infra` (Effect services):

| Interface | Semantics we define | Backend now (GCP) | Swap targets |
|---|---|---|---|
| `SqlStore` | Postgres, migrations-first, PITR assumed | Cloud SQL | AlloyDB, RDS/Aurora, self-hosted PG on SHC |
| `KvStore` | get/put/TTL | Postgres table (Memorystore if hot) | Redis anywhere |
| `BlobStore` | put/get/list/signed-URL | GCS | S3, MinIO on SHC metal |
| `JobQueue` | enqueue/lease/ack, retries, DLQ | Postgres SKIP LOCKED | Pub/Sub, SQS, NATS |
| `LiveHub` | per-scope WS fan-out + resumable window | Bun WS service on Cloud Run | Rivet Actors, any WS tier |
| `DurableStream` | append/read-from-offset/close | Postgres append table | Redis Streams, Kafka |
| `Scheduler` | cron dispatch table | Cloud Scheduler → `/internal/cron` | any cron + HTTP |
| `Mutex` | serialize named work | PG advisory locks | Redis locks |
| `SandboxExec` | run untrusted agent code | Firecracker on GCE + SHC (`oa-node`/`oa-workroomd`) | agent-os isolates for lightweight JS/Py |
| `MailSender` | transactional send | Resend API | SES, SMTP |

Rules: app code imports the interface Layer only; every backend is a config-time
Layer swap; each interface gets a conformance test suite that any new backend
must pass (this is the "hot-swappable" guarantee, and the AWS option is then a
set of Layers, not a rewrite).

---

## 6. Kubernetes, Terraform, and infrastructure-as-code

> **Update 2026-07-06 (#8527):** the Terraform/OpenTofu baseline now exists at
> `infra/` — GCS remote state, modules, and the critical Cloud SQL / Cloud Run /
> GCS resources imported with a clean no-op plan. See `infra/README.md`.

Today: **zero Terraform, zero k8s manifests** in owned repos; provisioning is
gcloud/wrangler/systemd scripts, and knowledge like "recreate the Hyperdrive
config to fix the pool" lives in runbooks. The cloud repo explicitly (and
correctly) defers GKE.

**Terraform: yes, now.** Adopt Terraform (or OpenTofu) with state in GCS as the
single description of: Cloud Run services, Cloud SQL instances + flags + users,
VPC/connectors, pgbouncer, LB/certs/domains, GCS buckets + lifecycle, Memorystore
(if used), Cloud Scheduler jobs, service accounts/IAM, Secret Manager refs,
monitoring/alerting policies, and the GCE templates behind agent computers and
inference lanes. Layout: `infra/` in the openagents repo, one module per §5
backend + one per deployable service; `prod`/`staging` workspaces; CI plan on PR,
gated apply on main. Terraform is also the multi-cloud hedge made real: an AWS
module set beside the GCP one, same interfaces above it.

**Kubernetes: not yet.** Cloud Run + GCE + systemd covers every current workload
(stateless HTTP, WS hub with session affinity, GPU VMs, Firecracker hosts,
containers-with-min-instances). GKE earns its keep only when we (a) adopt the
Rivet engine fleet-wide, (b) run many always-on stateful services with complex
scheduling, or (c) need portability to AWS/EKS and SHC metal under one deploy
model — revisit at Phase 4. Adopting k8s before then would add an ops tax with
no revenue offset, which is exactly the pattern the owner is cutting.

**Database rigor (never again a D1-style lock-in incident):**
- Postgres-only authority; every schema change via checked-in, linted migrations
  (khala-sync already enforces pending-migration gates — extend repo-wide).
- **PITR + automated backups + quarterly restore drills** on `khala-sync-pg` and
  the future monolith DB (Terraform-encoded settings, not console toggles).
- **Owned pooling** (pgbouncer in-VPC, config in git) — no opaque vendor pooler
  ever again on a write path.
- Read replica for analytics/soak queries so product reads never contend with
  dashboards.
- Connection/lock/replication-lag alerting in Cloud Monitoring (Terraform-encoded).
- Load-test gates before each read-cutover domain flip (the 2026-07-04 load-test
  report is the template).

---

## 7. Phased plan

**Phase 0 — stop the bleeding (days):**
Remove the Analytics Engine binding (unfreezes deploys — do this immediately);
cancel/disable unused paid features (Browser Rendering if QA can pause, WfP
namespace once Sites is archived); pull the Cloudflare invoice line-items and
map each to this audit's rows so nothing is paying for a parked product.

**Phase 1 — data authority fully on Google (1–2 weeks of focused lanes):**
Finish Khala Sync read cutovers domain-by-domain; bulk-migrate remaining cold D1
tables (export → Postgres import → verify counts/checksums); move R2→GCS and
KV→Postgres behind `BlobStore`/`KvStore`; stand up pgbouncer + direct connections
in staging to prove the Hyperdrive-free path.

**Phase 2 — compute off Workers (2–4 weeks):**
Monolith Bun app on Cloud Run behind Global LB (staging first, then prod with
CF-free-DNS fronting or full Cloud DNS); Cloud Scheduler replaces cron; Postgres
`JobQueue` replaces Queues; `LiveHub` Bun WS service replaces `KhalaSyncHubDO`;
`DurableStream` table replaces the inference-stream DO; advisory locks replace
the scheduler/ledger DOs; finish #8420 SyncRoom retirement instead of porting it;
Playwright-on-Cloud-Run replaces Browser Rendering; Resend-direct replaces the
email binding; port assets-only Workers (aiur/forge/start) trivially.

**Phase 3 — money path + relay (1 week, rehearsed):**
MDK treasury/sidecar/tips containers to Cloud Run/GCE with a rehearsed
single-treasury cutover; nostr-relay to a container + Postgres.

**Phase 4 — platform tier (when revenue justifies):**
Sites program redesigned on owned primitives (Cloud Run + Cloud CDN +
Google-managed certs + Firecracker previews); evaluate Rivet engine as the actor
runtime for per-user persistent agents/workrooms (GCE/GKE, Postgres driver);
openagents-world revival decision; AWS Layer set if a second cloud is wanted;
GKE decision gate.

**Postponed explicitly (owner-approved):** Sites/WfP program, openagents-world/
Verse backend, any new Cloudflare-native feature work. **Not postponed:** the
sync engine hub swap (§3.6) — that is in the critical path of "sync engine works
all on Google."

**Sequencing constraint to respect:** the monolith cannot leave Workers until D1
is evacuated (no D1 binding exists off-Workers), so Phase 1 gates Phase 2 —
which is healthy: it forces the D1 exit we already want.

---

## 8. Risks

| Risk | Mitigation |
|---|---|
| Monolith move breaks subtle Workers semantics (`waitUntil`, cron, per-request isolation) | Staging Cloud Run runs the full test + smoke suite for a soak week; keep the Worker deployable as rollback until parity proof |
| Treasury cutover error (money path) | Rehearse on staging Spark; single-treasury invariant; owner sign-off gate; do it last |
| DNS/TLS flip breaks auth callbacks (OpenAuth redirect allowlists) | Domains and paths don't change — only origins; test auth flows against staging LB first |
| GCP credit burn-rate surprise (Cloud SQL tier, LB, egress) | Terraform + budget alerts + weekly billing export review; right-size `khala-sync-pg` after load tests |
| Rivet bet too early | Not in Phases 0–3 at all; decision deferred to Phase 4 with real requirements |
| Team velocity hit during migration | Each phase ships behind the §5 interfaces incrementally — no big-bang branch; product lanes keep working against interfaces that don't change |

---

## 9. Bottom line

We are already half-moved: Postgres authority, GPU inference, agent computers,
OTA updates, and SHC own-metal all live outside Cloudflare today. What remains on
Cloudflare is the edge shell (Workers monolith), the D1 tail, two cache-layer
DOs, four small queues, blob/KV conveniences, the money-path containers, and a
postponed Sites program. Every one of those has a boring, owned, GCP-credit-funded
replacement, and the week's three incidents show the cost of waiting. Build the
thin owned-primitive layer, Terraform everything, finish the D1 exit, move the
monolith to Cloud Run, migrate the treasury last and carefully, and keep Rivet as
a deliberate Phase-4 option for the agent-platform tier rather than a dependency
of the escape itself.
