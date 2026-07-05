# Repo-Wide Cleanup and Khala Sync Adoption Audit

**Lane:** Fable synthesis, five parallel Explore agents, verified and
corrected against `origin/main` on 2026-07-05.
**Posture (owner directive):** aggressive consolidation. Nothing in
production is sacred in its current form except user data (forum posts,
receipts, ledgers, private traces). Simplify, streamline, delete without
ceremony.
**Companion doc:** `2026-07-04-khala-sync-implementation-status.md` (what
Khala Sync shipped). **This doc:** what to cut, merge, or migrate onto it
next, across the whole `OpenAgentsInc/openagents` monorepo.
**Zero-tech-debt ledger:** the closest thing this repo has to a "zero tech
debt doc" is not prose — it's the executable ratchet
`apps/openagents.com/scripts/check-zero-debt-architecture.mjs`. It encodes
every acknowledged debt class as a pinned numeric budget that a CI check
enforces never grows. §1 below reads that ledger as the authoritative debt
inventory.

This repo moves fast — dozens of fleet lanes land daily. Every item below
was checked against a live `origin/main` fetch, not assumed from an agent
transcript; a first pass wrongly reported the Khala Sync packages as empty
and three already-removed directories as still needing removal, both
corrected here. Before acting on any item, re-verify the specific claim
(one `find`/`grep` against current main) rather than trusting the count —
not because the audit is unreliable in general, but because "true when
checked" and "true now" are different things in a repo this active.

Five audit lanes, run as independent deep-exploration agents against
`origin/main`:

- **A** — the `openagents.com` Worker monolith + the debt ledger itself
- **B** — UI surfaces (`apps/web`, the not-yet-created `apps/start`) + sibling worker apps
- **C** — `packages/*` (30 packages)
- **D** — `apps/pylon`, `clients/*`, root `scripts/*`, docs sprawl
- **E** — Khala Sync adoption gaps (legacy sync spine, polling, live-at-read aggregates)

Verdicts use: **REMOVE** (delete now), **CONSOLIDATE** (merge N things into
one), **REFACTOR** (restructure, keep behavior), **ADOPT-SYNC** (migrate
onto Khala Sync scopes), **DEPRECATE** (mark dead, schedule removal),
**KEEP** (active, leave alone).

---

## 1. The debt ledger (`check-zero-debt-architecture.mjs`) — read as the tech-debt inventory

Every budget below is a **pinned ceiling**, currently sitting exactly at
max (fully consumed, ratcheted up commit-by-commit as debt was
acknowledged rather than fixed). This is the authoritative "what tech debt
do we have" answer for the Worker.

| Budget | Now/Max | What it is | Retire by |
|---|---|---|---|
| Raw `env: Env` handler params | **166/166** | 166 route handlers bypass the typed config/binding boundary | Introduce `OpenAgentsWorkerConfig` + capability injection at the seam; touches ~166 signatures |
| Worker `Response`-returning domain surfaces | **134/134** | 134 symbols build HTTP `Response` inside domain code instead of returning typed values for a route-mapper | Extract a shared route-mapper; domain code returns data, not `Response` |
| Route `Effect.promise` bridges | **18/18** | Route modules still Promise-shaped, wrapping Effect deps | Migrate the named wave-3 routes to Effect end-to-end |
| Worker `throw new Error` | **0/0** | Untyped throws where `TaggedError` is the standard | Retired in #8371 |
| `Effect.runPromise` named bridges | 20 files (index.ts×7, omni-handlers.ts×7, 13 singles) | Temporary Promise↔Effect seams | Retire per-route as each becomes a full Effect program; `index.ts` and `omni-handlers.ts` are the two fat targets |
| Public-projection staleness ledger | 101 tracked surfaces, **0 legacy rows** | Public projections must carry `generatedAt`/`generatedAtUnixMs` and a declared `maxStalenessSeconds` contract unless statically exempt | Keep the legacy budget at `0`; new rows must be `staleness_declared` or `static_contract_exempt` |
| 9 other budgets (string classifiers, raw `JSON.parse`, raw time/id/random, direct config reads, direct runtime-capability access, raw console logging, response-helper misuse) | **0/0 each** | Already fully retired — these are the *finished* invariants | Don't touch; they're the guardrails proving the pattern works |

**Read:** four ceilings (`env:Env` 166, Response-surfaces 134, runPromise
bridges, promise adapters) are almost entirely a symptom of **one root
cause** — `index.ts` is a 15,530-line hand-rolled router with inline
handlers taking raw `env` and returning raw `Response`. Fixing the router
(§2.2) collapses three of the four ceilings at once. The `throw new Error`
(12) and staleness-ledger (16 rows) items have now been retired; keep their
budgets at zero while attacking the larger router-shaped ceilings.

---

## 2. `apps/openagents.com/workers/api` — the Worker monolith

**Scale:** 2,215 `.ts` files (1,168 source / 1,017 test). `index.ts` alone
is **15,530 lines**, 461 imports, ~184 inline exact-match route entries,
plus a 444-line inline `scheduled()` handler running 24 cron tasks every
minute. 224 `*-routes.ts` files. 306 migrations → 438 `CREATE TABLE`
statements.

### 2.1 The router monolith — highest-leverage structural move in the entire audit

`index.ts`'s `exactRouteRegistry` inlines ~184 route entries by hand, even
though ~140 of the imports are already `make*Routes()` factories that
*could* just be concatenated. Splitting the registry into per-domain route
bundles (index.ts becomes a ~300-line composition root) is estimated to
move **11,000–13,000 LOC out of index.ts** and directly retires most of
the `env:Env` and Response-surface debt-ledger ceilings. **REFACTOR, highest
priority.**

Companion moves: extract the 24-task `scheduled()` block into a task
registry (−450 LOC, makes dead-cron pruning trivial); introduce one
prefix/param route-mapper to replace ~15 hand-written path-param
dispatchers (retires most of the 134-ceiling Response-surface debt).

### 2.2 Route sprawl — concrete REMOVE/DEPRECATE list (of 224 route files)

| File(s) | What | Last touched | Verdict |
|---|---|---|---|
| `mullet/*` (6 files, ~800–1,200 LOC) | Power/energy-telemetry market memory | No functional change since 2026-06-12 | **DEPRECATE→REMOVE** |
| `inference/internal-stress-preemption*`, `GlmStressSchedulerDurableObject` | GLM stress scheduler | Removed 2026-07-05 in #8381; Wrangler keeps historical create tags plus delete migrations | **REMOVED scheduler + deleted DO class** |
| `inference/benchmark/live-adaptive-stress-runner.ts`, `stress-saturation-plan.ts` | GLM adaptive-stress benchmark | Removed 2026-07-05 in #8381 after reverted lane audit | **REMOVED** |
| `inference/gym/agentcl-vertex-runner.ts` + table `gym_agentcl_eval_*` | AgentCL Vertex eval runner | Removed 2026-07-05 in #8380 | **REMOVED runner + DROPPED table family** |
| `omni-investor-demo-bundle-export.ts` | Investor demo bundle | Untouched since repo genesis | **REMOVE** |
| `lander2/3/4/5-routes.ts` + `lander-shell.ts` | Landing-page A/B experiments | Actively iterated (2 days old) | **DEPRECATE now, REMOVE on ONE-UI cutover** — collapse to one winner |
| `voice-program-ingest-routes.ts` | Voice ingest, `VOICE_PROGRAM_INGEST_ENABLED` never armed | Removed 2026-07-05 in #8386 | **REMOVED route/core/tests/flag** |
| `inference/mpp/mpp-chat-completions-routes.ts` + `mpp-discovery-document.ts` | Standalone MPP/x402 Khala chat endpoint, `KHALA_MPP_ENABLED` never committed armed | Removed 2026-07-05 in #8387; no direct Khala Code dependency | **REMOVED route/discovery/smokes/Stripe MPP config/replay caches** — future no-account machine payments need a fresh owner-approved design |

**Write-dead D1 table sweep (feeds #8330):** #8378 added
`bun run d1:zero-reference-sweep` plus the deterministic report
[`2026-07-05-d1-zero-reference-sweep.md`](./2026-07-05-d1-zero-reference-sweep.md).
Current-main evidence supersedes the original 438-table audit estimate:
the migrations now contain 446 `CREATE TABLE` statements and 419 unique
table names. After #8380, the sweep scans root packages as well as
`apps/openagents.com` and classifies 392 referenced, 7 confirmed
zero-production-reference, 0 test-only, 20 migration-only, and 0 manually
retained tables. The confirmed zero-production-reference set now includes
`forum_trust_edges` / `forum_actor_forum_trust` plus the retired
`gym_agentcl_eval_*` family. Both families have explicit Worker D1 drop
migrations and the matching Khala Sync mirror/twin cleanup.

### 2.3 Flag-gated cleanup candidates

`KHALA_MPP_ENABLED` was removed in #8387 after current-main verification found
no committed arming and no direct Khala Code dependency. The standalone
`/mpp/v1/chat/completions` route, root MPP discovery document, Stripe MPP
profile config, tests, smokes, and replay caches are gone. The Spark-primary /
MDK-fallback Lightning invoice helper remains because the Khala Code paid-plan
purchase route directly uses it; the legacy-named `KHALA_MPP_LIGHTNING_ENABLED`
now gates that paid-plan Lightning rail, not a live MPP chat endpoint.
`INFERENCE_BATCH_JOBS_ENABLED` was removed in #8384 after the owner-preferred
remove decision: the route/queue/store/OpenAPI surface had no Khala Code
dependency and no owner-approved arming evidence.
`VOICE_PROGRAM_INGEST_ENABLED` was removed in #8386 after current-main
verification found no committed Wrangler arming and no Khala Code dependency:
the Worker route/core/tests/flag were deleted, while unrelated voice evidence
and mobile approval projection contracts stay intact.
`INFERENCE_DURABLE_STREAM_ENABLED` was reclassified in #8385 as **KEEP** after
current-main verification: it is owner-armed in production/staging Wrangler
config and is the live Khala MCP resume/status contract for caller-owned
Pylon/Codex assignments (`khala.request`, `khala.spawn`, `khala.resume`,
`khala.status`), plus the background-agent run session-event projection. Do
not delete it as inert cleanup without replacing that Khala Code contract.
The original estimated remaining **500–1,500 LOC** full-code flag slice is now
resolved for the audited set: batch jobs and voice ingest were removed,
durable-stream was kept with live Khala Code evidence, and MPP/x402 chat was
removed/deferred.

### 2.4 Dual-store layering (23 domain stores, 3-4 seam patterns)

23 `*-store.ts`/`*repository.ts` files each hand-roll their own
encode/decode/error/put/get boilerplate across ~4 seam shapes. **Before
any more domain migrations land, consolidate into one generic
`makeD1MirrorStore<Row,Key>({table, encode, decode, tenantScope})` engine +
per-domain registries** — otherwise the Khala Sync Postgres dual-write seam
(already landed 14× in `workers/api/src/*-domain-store.ts` per the epic)
gets copy-pasted into N more one-off shapes. Estimated **3,000–5,000 LOC
dedupe**. **CONSOLIDATE, medium effort, low risk** (behavior-preserving,
well-tested surface already).

### 2.5 Cron sweep (24 every-minute tasks)

Two concrete findings: `BusinessFulfillmentLoop.dailyMotion` runs on the
1-minute trigger despite the name — **REFACTOR** to daily cadence or move
to Postgres-side scheduling post-cutover. `TassadarTracePairing.tick` reads
as a research-program remnant — **REVIEW/DEPRECATE**, confirm live
consumers. The reconciliation/sweep family (tips, treasury, forum-tip
archival) are natural candidates for `pg_cron` once their domains finish
Postgres cutover, leaving the Worker cron for edge-only heartbeats/probes.

### 2.6 Auth-helper duplication

66 non-test files independently reference admin/agent-token auth helpers.
**CONSOLIDATE into one `auth/` module** — ~500–1,000 LOC dedupe, low risk.

2026-07-05 #8382: common Worker Authorization parsing now lives in
`workers/api/src/auth/bearer-token.ts`. The first pass removed the strict
split-parser copies from `index.ts`, Forum, Forge, onboarding,
customer-order, Pylon, agent-definition, trace, Tassadar, agent-home,
agent-site, and agent-proposal routes, including the `oa_agent_` prefixed
programmatic-agent helper. Remaining local parser exceptions are deliberate:
`agent-owner-claim-routes.ts` keeps its regex/claim-header fallback semantics,
and `agent-search-routes.ts` keeps its whitespace-trimming parser. Do not
collapse those without an explicit auth behavior review.

---

## 3. UI surfaces + sibling apps

### 3.1 `apps/openagents.com/apps/web` (Foldkit SPA) — the biggest single deletable surface in the repo

**188K LOC across 442 files, still hot (507 commits since Jun 4).**
Production bundle: **4.16 MB raw / 1.07 MB brotli** main JS + 630 KB isolated
lander3 scene bundle + 383 KB CSS + a 5.5 MB `.glb` model + 4 hero JPEGs
(208–535 KB each). 88 routes in one `route-table.ts`.

**`apps/start` (the ONE-UI React/TanStack Start replacement) does not exist
yet** — Wave 0 unstarted. So today's migration backlog is literally all 88
routes.

**Dead-on-arrival routes — delete now, in `web`, with zero dependency on
`apps/start` existing:**

| Route(s) | Verdict | Why |
|---|---|---|
| `Moksha`, `Moksha2` (`/moksha`, `/moksha2`) | **REMOVE** | Landing A/B experiments |
| `Landing`, `LandingPreview`, `ClientsPreview` | **REMOVE** | Superseded by canonical `Home` |
| `Forge` (`/forge`) | **REMOVE** | `apps/forge` (forge.openagents.com) is the live successor; the web page's own domain README calls it "source material only" |
| `PublicStatsArchive` (`/stats-old`) | **REMOVE** | Superseded by `Stats` (`/stats`) |
| `Animations`, `Components`/`ComponentsFamily` (`/animations`, `/components`) | **REMOVE** | Dev galleries, replaced by shadcn/Storybook in the new stack |
| `demo`/`demo2` twins (2,528 LOC) | **CONSOLIDATE** to one demo namespace | Duplicated sales-demo playback |

Deleting this list alone is **~15,000–20,000 LOC + the 5.5 MB GLB + hero
JPEGs out of the bundle graph**, with zero migration dependency — do it
before `apps/start` exists, not after. Every deletion must drop the
`route-table.ts` entry *and* the `worker-routes.ts` allowlist entry
together (the doc that documented the `/trace` 302-regression precedent
makes this the one hard rule here).

**Routes that must survive the React migration** (the real Wave 1/2
backlog): the public funnel (Home, Autopilot, Blog, Docs, Code, Khala,
KhalaChat, Pylons, Pro, Login, Terms/Privacy, PublicAgent, Trace, Share,
Stats, ProductPromises), and the logged-in app + forum web (Order,
AutopilotWork, Dashboard, Settings, Billing, Chat/Thread, Team*, Workroom,
Forum*). Full table in the raw agent report if needed for the migration
tracker.

**Asset finding worth a separate look:** `apps/web/public/` ships ~120 KB+
of internal agent-facing markdown (`AGENTS.md`, `SURFACES.md`, `PYLON.md`,
etc.) as public production assets. Audit whether these belong on the
public origin at all before the ASSETS repoint.

### 3.2 Sibling worker apps — verdicts

| App | Verdict | Why |
|---|---|---|
| `apps/forum` | **DEPRECATE or commit to extraction** | 35-LOC stub Effect contract; the real 6,823-LOC forum lives in the Worker's `forum-routes.ts` — extraction never happened, the stub is misleading dead weight |
| `apps/forge` | **KEEP** | Active, deployed, own domain — and the correct target that supersedes web's `/forge` |
| `apps/openagents-world` (Verse) + `packages/world-contract`/`world-client` | **KEEP if Verse is a live product, else CONSOLIDATE** | Self-contained (zero external importers of the packages), but recent commits are QA-harness-only, not features — smells parked. **Needs an explicit owner call**: if Verse isn't shipping, fold its projection into Khala Sync scopes and retire the standalone Worker + 2 packages |
| `apps/nostr-relay` | **KEEP** | Deployed infra, two live domains |
| `apps/oa-updates` | **KEEP** | Very active (38 commits), desktop OTA — load-bearing |
| `apps/qa-runner` | **KEEP** | Most active sibling (52 commits) — the QA substrate |
| `apps/acceptance-runner` | **KEEP, watch for CONSOLIDATE into qa-runner** | Low activity (1 commit), possibly redundant with qa-runner |

### 3.3 Packages tied to UI

`packages/ui` (root, 29,812 LOC, 40 live importers) and
`apps/openagents.com/packages/sync-worker`'s HTTP-primitives half are both
**KEEP-now, DEPRECATE-on-migration** — Foldkit is still the live stack
until ONE-UI's React/shadcn edition lands consumers. Do not delete either
before `apps/start` has something importing the replacement.
`design-tokens` survives the migration unchanged (it's the shared
Protoss-blue token substrate both stacks consume). `autopilot-ui`,
`composer-state`, `input-bindings` are the same Foldkit-era cohort, same
verdict.

---

## 4. `packages/*` — 30 packages, per-package verdicts

### 4.1 The single largest duplication finding: `probe` vs `pylon-runtime`

**`packages/khala-sync`, `packages/khala-sync-client`,
`packages/khala-sync-server` are fully built** — 11 + 26 + 87 = 124 source
files, the complete engine described in the companion status doc (contracts,
substrate, mutators, capture, hub, client store/overlay/session). Live on
`openagents.com` in production. No action item here.

**`packages/probe` (probe-runtime, ~32,500 LOC) and `apps/pylon`'s bundled
`pylon-runtime` (~17,300 LOC) are near-identical forks of the same
coding-agent runtime** — same directory names (`auth backends benchmark
blueprint contracts fleet llm omega runner runtime`), same core files
(`permission.ts`, `receipt-redaction.ts`, `workspace.ts`,
`opentui-renderer.ts`), neither imports the other. probe's own README
frames it as the intended canonical "reset" runtime. **This is the single
largest code-duplication finding in the whole audit (~50K LOC across two
copies of one runtime).** **CONSOLIDATE** — pick probe as the one surface,
collapse `apps/pylon/packages/runtime` onto it. High value, high risk
(both qa-runner and pylon depend on their respective fork) — needs its own
dedicated migration project, not a quick PR.

### 4.2 Orphans and small removals

| Package | LOC | Importers | Verdict |
|---|---|---|---|
| `connector-sidecar` | 479 | **0** | **REMOVE** — true orphan, kept "alive" only by an INVARIANTS.md test reference |
| `replay-clips` | 774 | 2 (1 app) | **DEPRECATE-watch** — verify EPIC #5411 is still live; if dead, REMOVE |

### 4.3 Future Khala Sync consolidation targets (do not touch yet)

`durable-stream` (1,335 LOC) and `public-activity-timeline` (753 LOC) are
both substrate that Khala Sync is *eventually* meant to subsume (see §5.4,
§5.7) — freeze feature work on them now, migrate consumers once the
relevant khala-sync scopes exist, then fold or retire.

### 4.4 Everything else

Confirmed live and correctly scoped: `behavior-contracts`, `arbiter-effect`,
`world-client`/`world-contract` (paired, tied to Verse's fate — see §3.2),
`atif`, `blueprint-contracts` (44 importers — not a dead remnant despite the
name), `tassadar-executor` (44 importers, heavily wired — rename candidate
only, not removal), `autopilot-control-protocol` (52 importers, very
active), `nip90` (27 importers, live market rails), `proof-replay`,
`khala-tools`, `khala-qa-harness`, `mcp-contract`, `agent-runtime-schema`,
`provider-account-schema` (194 importers — the single most-imported package
in the slice), `effect-boundary`, `forge-protocol`. **KEEP all, no action.**

---

## 5. `apps/pylon`, `clients/*`, root `scripts/*`

### 5.1 Immediate zero-risk wins

- **11 GB of untracked local build output** at `apps/pylon/dist/rc/1.0.0-rc.*` (33 dirs) — disk hygiene, not a repo change, but flag it: `rm -rf apps/pylon/dist/rc`.
- **`clients/openagents-desktop`** — REMOVE. Superseded by `khala-code-desktop`; current state is a 6-file empty electrobun stub (deleted 2026-06-29, re-stubbed empty the next day). Also drop its `test:openagents-desktop` wiring from root `package.json`.
- **Already done, no action needed** — `clients/khala-desktop`, `clients/khala-macos`, and `apps/openagents-world-spacetimedb` (the legacy Rust/SpacetimeDB world backend) have all already been fully removed from `origin/main`; none of the three exist in the current tree.
- **`clients/khala-mobile`** (new since the first pass of this audit, landed 2026-07-04, TS-8) is the real Expo React Native companion — 23 source files, active. Not a cleanup target; noted here so it isn't mistaken for dead weight by a future pass.
- **`scripts/vertex-fleet/`, `scripts/gemini-fleet/`** — REMOVE (~1,600 LOC). `vertex-fleet` carries its own `DEPRECATED.md`: *"RETIRED 2026-06-20 … superseded by scripts/codex-fleet/."*
- **`scripts/khala-demo/`** (2,930 LOC) and the GLM/Vertex continual-learning burn scripts (1,127 LOC) — DEPRECATE/ARCHIVE, unwired milestone-closure and one-off stress harnesses.

### 5.2 The supervisor policy migration (ROADMAP T2.3 — mostly done, has a tail)

The bash-to-typed-store demotion is largely complete — the store/coordinator
own claim/pause/desired-state and a bypass guard
(`check-supervisor-store-bypass.mjs`) enforces it. What's left: **~3,000
LOC of dispatch *policy*** still lives in shell (`lockout.sh`,
`priority-dispatch.sh`, `pr-review-refill.sh`, `replenishment.sh`,
`backoff-policy.sh`) rather than in `coordinator.ts`/`store.ts`.
**REFACTOR** — migrate policy into the typed store behind the existing
`.test.sh` oracles. Medium risk: this is the live 24/7 own-capacity burn
lane; migrate incrementally, don't big-bang.

**Verify first:** the audit found **zero references** to Khala Sync fleet
intents (#8332) inside `apps/pylon/src/orchestration/` — confirm where that
landed before starting this migration, since intent-steering may be the
intended new home for exactly this policy.

### 5.3 Dispatch front-end consolidation

`khala-burndown.ts`, `khala-spawn.ts`, `khala-dispatch.ts`,
`khala-requester.ts` are three-to-four overlapping dispatch front-ends to
the same underlying store (the shell supervisor exists specifically
*because* burndown/spawn cap at one lane per account). **CONSOLIDATE**
onto the typed coordinator. Separately, `khala-m6-shadow-preflight.ts` /
`khala-m7-conductor-*` (~2,750 LOC) read as closed-milestone scaffolding —
**DEPRECATE/archive**.

### 5.4 Executor duplication

`codex-agent-executor.ts` (2,085 LOC) and `claude-agent-executor.ts` (982
LOC) already share workspace materialization but duplicate a second tier —
identical lifecycle scaffolding, escape-guard checks, and even a literal
copy-pasted `sum_repair` test fixture. **REFACTOR**: extract a shared
`agent-executor-core`. ~300–500 LOC dedupe, medium risk (both are live
execution paths — extract behind existing tests).

### 5.5 Desktop UI retirement sequencing (ties to ONE-UI, §3.3)

`clients/khala-code-desktop/src/ui/*` (the vanilla-DOM shell — `main.ts`
alone is 4,603 LOC, ~20,000+ LOC total across panels/renderers) is the
flagship's current UI and **must be retired behind ONE-UI parity, not
deleted outright.** `src/bun/*` (RPC/services) and
`src/contracts/ux-contracts.ts` (the owner-mandated behavior-contract
registry) are the durable core the React rebuild will keep consuming —
**KEEP unconditionally.**

### 5.6 Docs sprawl (do not touch `docs/transcripts/` — protected)

Archivable into a `docs/archive/`: `docs/launch/gemini-fleet/` +
`docs/launch/vertex-fleet/` (pairs with the §5.1 retired lanes),
`docs/afteraction/`, `docs/tui/`, `docs/agi/`, `docs/asi/`, `docs/blitz/`,
`docs/sakana/`, and the dated status-audit subset of
`docs/autopilot-coder/` (keep the live AFK-loop runbook it also contains).
Low risk — these are moves, not deletes; grep for inbound links first.

---

## 6. Khala Sync adoption gaps — the highest-priority section per the owner's ask

**Headline finding:** the *database* migration (Postgres dual-write) is
essentially complete across ~14 domains. The *sync-engine surface*
adoption — actually routing live product state through scopes, the
changelog, and the hub instead of the legacy spine or raw polling — has
barely started. **Only two surfaces ride the new engine today**: the
public tokens-served counter (`scope.public.tokens-served`, #8304) and the
desktop fleet cockpit (`scope.fleet_run.<runId>`, #8302, behind a flag).
Everything else — team chat, thread files, agent goals, the settled-feed
firehose, gym run-progress, every desktop poll, every live-at-read public
aggregate — still runs on the pre-Khala-Sync spine. This is the real
"what hasn't adopted the sync engine" answer.

### 6.1 The legacy spine to retire

`apps/openagents.com/packages/sync-worker`'s D1 outbox
(`sync_scopes`/`sync_changes`/`sync_mutations`, a `SyncSequence` per-scope
counter that **literally duplicates** khala-sync's own version allocator)
+ `SyncRoomDurableObject` + `sync-notifier.ts`'s `notifySyncScopes` fan-out
is still the transport for six live consumers (tokens-served producer —
now *duplicated* alongside #8304's projection, the Tassadar settled feed,
gym run-progress, team chat, thread files, agent goals/goal-events). Web's
`subscriptions.ts` is the single chokepoint (`syncStreamHref` →
`/api/sync/${kind}/${id}/stream`) — repointing it to
`/api/sync/connect?scope=` is the one change that unblocks retiring the
entire legacy spine. Once all six consumers move, **`SyncRoomDurableObject`
and the D1 `sync_*` tables have zero remaining users and drop cleanly**
(explicitly called out as a non-migration, no-Postgres-twin item in
`MIGRATION_PLAN.md` §3.16).

### 6.2 Concrete ADOPT-SYNC list, ranked by effort/risk

1. **Tokens-served**: complete in #8372 — the *legacy* producer
   (`inference/khala-tokens-served-sync.ts`) and its stale old-sync
   broadcast throttle were deleted after #8304's `scope.public.tokens-served`
   projection was re-verified as the live producer path. (S, low)
2. **Desktop fleet cockpit**: complete in #8383 — `KHALA_SYNC_FLEET` is default-on, `0`/`false`/`off` are explicit opt-out values, and the Fleet panel no longer schedules the old visible 5s `fleet-status.ts` poll. Projection already proven at load (9,909 pushes, zero failures). (S, low)
3. **Settled feed** (`tassadar-settled-feed-sync.ts`): new `scope.public.settled-feed` projection, same #8304 pattern, then delete the legacy producer. (M, med)
4. **Team chat + thread files + agent goals**: the flagship "migration = sync adoption" case per KS-8.13/#8324 — land on `scope.team.<id>` / `scope.thread.<id>` / `scope.agent_run.<id>` / `scope.user.<id>`, replacing both the notifier fan-out and the desktop/web polling in one move. (L, med)
5. **Public aggregates** (demand-mix, model-mix, tokens-history, public activity timeline): project off live-at-read D1 onto `scope.public.*` counters — the Postgres rollup twins already exist from KS-8.2. (M, low)
6. **Desktop hot polls**: 1s Claude-approval poll, 2s thread-token-summary poll, 5s inbox poll — all map cleanly to `scope.agent_run`/`scope.thread`/`scope.user`. (M, med — the 1s approval poll is latency-sensitive, migrate carefully)

### 6.3 Explicitly do NOT consolidate

`openagents-world`'s Region DO (real-time game state, deliberately off the
shared DB per both CLAUDE.md and the migration plan), `nostr-relay`
(unrelated protocol), `event_ledger_entries` (owner-private GitHub intake,
correctly Postgres-only, not a public scope), internal admin
leaderboards/analytics (D1-until-KS-8.19 by design), device-local codex
telemetry.

### 6.4 Adjacent substrate — note, don't merge yet

`packages/durable-stream` + `DurableInferenceStreamObject` reimplement the
same DO-SQLite offset-log primitive the Khala Sync hub uses, but for opaque
byte/token streams (inference completions, world deltas) rather than typed
scoped entities — forcing token streams through the mutator/changelog model
would be a bad fit. **Keep separate**; flag as a future one-primitive/
two-schemas refactor, not urgent. Same note for the agent-definition
per-run live-surface spike doc — when its adoption gate opens, prefer
`scope.agent_run.<runId>` on the existing hub over building a parallel
per-run DO class.

---

## 7. Sequenced cleanup plan

Ordered by leverage-to-risk, respecting the aggressive-but-not-reckless
posture (delete freely where nothing points at it; sequence carefully
where live traffic depends on the old path).

**Wave 0 — zero-risk, do immediately, no dependencies:**
Reclaim the 11 GB pylon build cache; remove `clients/openagents-desktop`
(the only one of the three dead-client stubs still present — see §5.1);
remove `scripts/vertex-fleet`, `scripts/gemini-fleet`; remove the dead
`apps/web` routes (Moksha×2, Landing variants, `/forge`, `/stats-old`,
`/animations`, `/components`); convert the live untyped `throw new Error`
sites; delete the legacy tokens-served producer now that #8304's projection is
live.

**Wave 1 — mechanical, low risk, high leverage:**
After #8377, the legacy public-projection staleness budget is zero; after
#8378/#8379/#8380/#8381, the D1 zero-reference sweep exists, the forum trust
pair is dropped, the AgentCL Vertex runner plus `gym_agentcl_eval_*` table
family are retired, the orphaned GLM stress scheduler/adaptive runner is
gone, #8382 consolidated the 66-file auth-helper sprawl, and #8383 made the
desktop fleet cockpit Khala Sync-first by default while deleting the old
visible 5s poll, #8384 removed the batch-job async lane, and #8385 confirmed
durable-stream is a live Khala Code resume/status contract rather than an inert
flag. #8386 removed the unarmed voice ingest route/core/tests/flag. Next:
decide arm-or-remove on the remaining owner-decision flag.

2026-07-05 Wave 1 issue index:

- #8377 - Complete: retrofitted the 16 legacy public-projection staleness
  contracts and ratcheted the zero-debt legacy-missing-staleness count to zero.
- #8378 - Complete: added the repo-wide D1 zero-reference table sweep and
  committed the KS-8.19 evidence report for #8330.
- #8379 - Complete: dropped the write-dead forum trust D1 tables and removed
  the stale Khala Sync/forum fixture surfaces that kept them alive.
- #8380 - Complete: removed the AgentCL Vertex runner/CLI/test surface and
  retired the `gym_agentcl_eval_*` D1/Postgres table family.
- #8381 - Complete: removed the GLM stress-scheduler remnants, appended
  Durable Object delete migrations, and deleted the adaptive-stress benchmark
  paths.
- #8382 - Complete: consolidated the shared Worker bearer-token parsers into
  `workers/api/src/auth/bearer-token.ts`, leaving only documented
  route-specific parser exceptions.
- #8383 - Complete: flipped `KHALA_SYNC_FLEET` default-on with explicit
  opt-out values and deleted the desktop Fleet panel's visible 5s poll.
- #8384 - Complete: removed the default-off `INFERENCE_BATCH_JOBS_ENABLED`
  batch-job route/queue/store/OpenAPI surface and added forward D1/Postgres
  drop migrations for `inference_batch_jobs`.
- #8385 - Complete: kept `INFERENCE_DURABLE_STREAM_ENABLED` after current-main
  verification found it owner-armed and directly consumed by Khala MCP
  request/spawn/resume/status plus background-agent run projections.
- #8386 - Complete: removed the unarmed `VOICE_PROGRAM_INGEST_ENABLED` Worker
  route/core/tests/flag and updated the product-promise evidence to keep only
  voice evidence/projection contracts.
- #8387 - Complete: removed the unarmed standalone `KHALA_MPP_ENABLED`
  MPP/x402 Khala chat endpoint, discovery doc, Stripe profile config, smokes,
  and replay-cache tables while keeping the Khala Code paid-plan Lightning
  invoice helper.

**Wave 2 — structural refactors, medium risk, biggest long-term payoff:**
Split `index.ts`'s route registry into per-domain bundles (collapses three
debt-ledger ceilings at once); extract the `scheduled()` cron block into a
task registry; build the one generic D1-mirror-store engine before any
more domain migrations copy-paste a fourth seam pattern; finish the T2.3
bash-to-typed-store policy migration in pylon; extract the shared
codex/claude executor core.

**Wave 3 — sync-engine adoption (owner's stated priority target):**
Repoint web's `subscriptions.ts` to `/api/sync/connect`; migrate settled
feed, team chat, thread files, and agent goals onto khala-sync scopes;
project the remaining live-at-read public aggregates; migrate the
remaining desktop hot polls; then retire `SyncRoomDurableObject` and drop
the legacy `sync_*` D1 tables — the clean capstone of this whole
consolidation effort.

**Wave 4 — big, deliberate, needs an owner call before starting:**
Consolidate `probe-runtime` and `pylon-runtime` (~48K LOC of duplicated
coding-agent runtime — the single largest dedupe in the audit, but touches
both qa-runner and pylon's live execution path); decide Verse's product
status (ship it, or fold its projection into Khala Sync and retire
`apps/openagents-world` + 2 packages); sequence the Foldkit→ONE-UI
migration itself (already its own epic, #8339) using this doc's Wave-0
`apps/web` deletions as the pre-migration warm-up.

**Ongoing, not gated:** archive the docs-sprawl families listed in §5.6
into `docs/archive/` (moves, not deletes, `docs/transcripts/` stays
untouched).

---

## 8. Wave 0 execution ledger

2026-07-05:

- #8366 complete — the local ignored Pylon RC build cache at
  `apps/pylon/dist/rc` was removed from the active dirty checkout after
  verification with `du -sh` and `git status --ignored`. Both the root
  `.gitignore` and `apps/pylon/.gitignore` already ignore `dist/`, so no
  ignore-rule change was needed.
- #8367 complete — the retired `clients/openagents-desktop` Electrobun stub was
  removed from the workspace, root dev/test/typecheck wiring, and Bun lockfile.
  Current operator docs now point live desktop work at
  `clients/khala-code-desktop`.
- #8368 complete — retired `scripts/vertex-fleet` and `scripts/gemini-fleet`
  runnable lanes were removed. `scripts/codex-fleet` is now the only active
  script lane in that family and no longer references the deleted runner paths.
- #8369 complete — retired the dead Foldkit web route family (`/moksha`,
  `/moksha2`, `/landing`, `/preview/landing`, `/clients-preview`,
  `/components`, `/animations`, `/forge`, `/stats-old`) from the central route
  table, Foldkit parser, Worker document allowlist, startup policy, render
  branches, and tests. Root `/` now parses to `Home`; `/stats` is the only
  public stats document route.
- #8371 complete — converted all 13 live production Worker `throw new Error`
  sites counted by the zero-debt architecture ledger to typed `TaggedError`
  variants or existing repository errors, then ratcheted the Worker throw
  budget to `0/0`. A small `team-chat` helper input was narrowed so a
  pre-existing Response-surface overage was retired without raising that
  budget; `check:architecture` is green.
- #8372 complete — deleted the legacy
  `workers/api/src/inference/khala-tokens-served-sync.ts` producer and its tests,
  removed every production `publishKhalaTokensServedDelta` caller, and dropped
  the stale high-frequency `public-khala-tokens-served:*` SyncRoom throttle. The
  #8304 `scope.public.tokens-served` projection/reconcile path remains the live
  public counter producer.

---

## Appendix: revision note

2026-07-05: corrected against a fresh `origin/main` fetch — the Khala Sync
packages (§4.1) are fully built, not empty, and three items in the original
Wave 0 list (`clients/khala-desktop`, `clients/khala-macos`,
`apps/openagents-world-spacetimedb`) had already been removed before this
audit ran and are struck from the action list (§5.1). The five source
per-slice reports (dense per-file tables with line numbers, commit hashes,
importer counts) remain in this session's transcript for anyone picking up
a specific item — re-verify the specific file/count before acting, not
because the audit is unreliable, but because dozens of lanes land daily.
