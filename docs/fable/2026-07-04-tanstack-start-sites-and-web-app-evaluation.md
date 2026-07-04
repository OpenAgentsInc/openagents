# TanStack Start × Autopilot Sites × openagents.com — Full Evaluation

Date: 2026-07-04
Status: analysis/evaluation in the Fable lane. No promise state flips, no
public copy changes, no issues filed (§10 proposes the map). This doc
evaluates an owner-raised strategic question and recommends a path; the
final call changes a standing repo default (the Bun/Effect/Foldkit working
rule in CLAUDE.md) and is therefore an **owner decision** — when made, the
rule, AGENTS.md, and INVARIANTS surfaces update in the same change.

Sources: the TanStack reference lane (`projects/tanstack/` — manifest of
query/table/router/virtual/form/db/react-charts/ai/bling/tanstack.com, all
cloned), the tanstack.com production app (a real TanStack Start app
deployed as a Cloudflare Worker), TanStack DB's adapter contract, the
Lovable engineering post ("Building apps using TanStack Start",
lovable.dev, read 2026-07-04) and the TanStack announcement of it, our
Autopilot Sites implementation (site-runtime/sites/builder-sessions in the
Worker), the site-speed lane baseline (`docs/perf/2026-07-02-landing-page-baseline.md`),
the Khala Sync design (`2026-07-04-database-alternatives-and-postgres-sync-engine.md`)
and embryo (`@openagentsinc/sync-worker`), and yesterday's mobile report
(`2026-07-04-mobile-companion-and-khala-sync-report.md`).

## 0. The one-paragraph verdict

Adopt the Lovable playbook in both directions: **make TanStack Start the
canonical output of Autopilot Sites, and migrate the openagents.com web
surface to TanStack Start on our existing Worker infrastructure** —
incrementally, funnel pages first, with Effect preserved everywhere (it is
framework-agnostic and stays the substance of the server and services) and
Foldkit preserved tactically (Start's per-route `ssr: false` lets existing
Foldkit programs mount inside client-only routes during a long migration,
and Foldkit remains the desktop-shell direction until this decision says
otherwise). The dogfood symmetry is the point: we build in the stack our
product generates, every rule we learn steering our own fleet becomes the
curated rules pack Sites injects into user builds (Lovable's core lesson:
framework primitives are guardrails for AI codegen and a measurable
quality lever), and our own site finally gets the SSR/code-splitting fix
the site-speed lane already prescribed — the same fix that made Lovable's
prerendered apps **+98.5% visible to AI tools**, which is literally the
product our agent-readiness campaign sells. The connective tissue is
**TanStack DB**: its `SyncConfig` adapter contract maps almost 1:1 onto
the Khala Sync outbox we already run in production, so
`@openagentsinc/khala-sync-db-collection` becomes the client half of Khala
Sync — replacing the bespoke client plan with a maintained reactive store
that ships live queries, optimistic writes, and SQLite persistence for
browser, Cloudflare DOs, **and React Native/Expo**, which softens (but
does not overturn) yesterday's native-mobile recommendation.

## 1. What Lovable actually did (and the three lessons that transfer)

From their engineering post and TanStack's announcement: new Lovable
projects ship on TanStack Start by default (since May 13) — full-stack
React, SSR/SSG/CSR **per route**, server functions colocated with
components (`createServerFn` compiles to typed fetch stubs client-side),
explicit server/client boundaries (`*.server.ts`,
`server-only` modules), deployed as **edge Workers on Cloudflare** with
secrets injected as Worker bindings at request time. Their data layer
stayed put (Supabase) — the framework swap didn't force a data migration.

1. **Framework primitives are AI guardrails.** "When AI is generating
   full apps, framework primitives are a direct translation to product
   quality" — typed routing, typed server-fn boundaries, and route-level
   rendering choices constrain the model toward correct programs. Lovable
   pairs this with **curated TanStack-specific rules injected into every
   request**, co-developed with the TanStack team and refined from usage.
   For us this is doubly native: our behavior-contracts machinery is
   exactly the enforcement half of "rules for generated apps."
2. **Prerendering is agent-readiness.** Their measured result: +2.9%
   organic search, **+98.5% traffic from AI tools** (ChatGPT/Perplexity)
   across deployed domains, because crawlers get server-rendered HTML
   instead of an empty shell. That is the exact failure class our
   agent-readiness prober flags (LG-1), the exact fix wave we ran on
   ourselves (commit `55297c5deb`), and — embarrassingly — the exact
   defect our own web app still has (4.1 MB SPA, nothing paints without
   JS). Sites generated as Start apps pass our own audit **by
   construction**; we should also bake the rest of the audit checklist
   (llms.txt, ai-catalog, JSON-LD, robots/sitemap) into the site template
   so every Autopilot Site ships agent-ready. We sell the audit; our
   product output should be its best advertisement.
3. **The stack you generate should be the stack you run.** Their team
   builds tanstack.com and Lovable-adjacent tooling on the same
   primitives their users get. The owner's instinct here ("I generally
   prefer if the stack WE build in is the stack Autopilot Sites builds
   in") is the same compounding loop: every paper cut we fix for
   ourselves improves the product output, and vice versa.

## 2. What the reference lane gives us (facts)

**tanstack.com is the architecture reference we wanted** — a production
TanStack Start app running as a plain Cloudflare Worker, and its patterns
transfer directly onto our infra:

- `wrangler.jsonc` with a **custom Worker entry** (`main: src/server.ts`)
  wrapping Start's `server-entry` handler — so one Worker does SSR *plus*
  security headers, proxies, content negotiation (they rewrite
  `Accept: text/markdown` docs requests to `.md` — an agent-readiness
  touch), **cron `scheduled` handlers**, R2 bindings, and Workers Assets.
  This is our Worker shape already.
- Build: `@cloudflare/vite-plugin` (`viteEnvironment: {name: 'ssr'}`) +
  `tanstackStart()` + React — **not Nitro** for Cloudflare.
  `importProtection` hard-fails client bundles that import server-only
  modules (a CI-time claims-lint for code); router code-splitting defaults
  split component/loader per route; manual vendor chunks.
- Per-route rendering: SSR by default; `ssr: false` for the two
  client-only surfaces (their in-browser builder). API routes are file
  routes with `server.handlers`; ~25 server-fn files.
- Versions in the lane: `@tanstack/react-start` 1.168.x,
  `react-router` 1.170.x, `query` 5.101.x, `db` 0.6.14 (beta),
  `react-form` 1.33, `react-table` 9-beta, plus **TanStack AI** 0.39
  (providers incl. Anthropic, `ai-sandbox-cloudflare`,
  `ai-isolate-cloudflare`, MCP/ACP packages — a lane to watch for the
  Sites builder and gateway, not part of this decision).
- **React Native: not supported by Router/Start** (React DOM + Solid
  only, per their own docs). TanStack **DB**, however, ships SQLite
  persistence for `react-native-`/`expo-`/`capacitor-`/`electron-`/
  `tauri-`/`browser-`/`node-` and — notably —
  `cloudflare-durable-objects-db-sqlite-persistence`.

## 3. Autopilot Sites today, and what "all user sites are TanStack Start" means

**Today** (from the implementation audit): Autopilot Sites is a generic
host, not a framework. A site = D1 rows (`site_projects`/`site_versions`/
`site_deployments`) + R2 artifacts; two runtime kinds — `omega_static_r2`
(static files streamed from R2; the live default) and
`workers_for_platforms` (per-site Worker module sub-dispatched through the
`SITES_DISPATCH` namespace). Content comes from agent **builder sessions**
(file snapshots in D1 → `saveVersion` → R2). Production deploy is
owner/operator-gated; all four `autopilot_sites.*` records are yellow/red
with the last mile staged (custom hostnames INERT pending Cloudflare-for-
SaaS arming; email sends flag-gated pending deliverability proof; no paid
customer site receipt). There is **no SSR, no template engine, no
framework** in the serving path — sites are whatever files an agent wrote.

**The Lovable move, translated onto our infra** (the good news: the hard
infrastructure already exists):

1. **Canonical site shape**: a TanStack Start project template — typed
   routes, server fns, Tailwind 4 with our token system available,
   `wrangler.jsonc` per site — built to a **Worker module** and deployed
   through the *existing* `workers_for_platforms` runtime kind. WfP
   becomes the default runtime for new sites; `omega_static_r2` remains
   for trivially-static output (Start's prerender can emit that too).
2. **Build pipeline**: Start sites need `vite build` (dependency install +
   build execution). The preview runner already classifies exactly this
   (`runtimeNeeds` → `container_metered` tier); the Cloudflare Containers
   lane referenced in the sites docs is the build executor. This is the
   one genuinely new machine: version → containerized build → Worker
   module + assets into R2 → existing deploy gates. Deterministic,
   receipted builds (lockfile pinned, build log in R2 — already a field).
3. **The rules pack**: Lovable's curated-rules lesson, implemented our
   way — a versioned `sites-tanstack-rules` doc injected into every
   builder session *plus* behavior contracts generated per site (the
   customer-invariant catalog the QA Swarm sells: dead controls, broken
   flows, claim safety). Generated sites get QA'd by our own swarm; the
   loop closes.
4. **Agent-ready by construction**: the template ships SSR-by-default,
   llms.txt, `/.well-known` agent surfaces, JSON-LD, robots/sitemap —
   every Autopilot Site passes the LG-1 audit on day one, which is both
   product quality and marketing for the audit campaign.
5. **Secrets as bindings** (Lovable's pattern): per-site server env
   injected as Worker bindings at deploy, never bundled — WfP supports
   per-script bindings; our deploy gate records what was bound.

What this does *not* change: the ownership/gating model (builder →
saved version → operator-gated deploy → receipts), custom-hostname and
email machinery, or the promise states — those advance on their existing
blockers regardless of the framework inside the artifact.

## 4. Migrating apps/openagents.com itself — evaluation

**Current state** (the migration surface): `apps/web` is a Foldkit
(Elm-architecture) SPA — `foldkit@0.102`, Effect 4 beta, Vite, Tailwind 4,
Three.js scenes via `three-effect` — built to static assets served by the
Worker's ASSETS binding. The measured pain is not stylistic: **one 4.10 MB
JS bundle (1.07 MB brotli), no route splitting, no SSR**; FCP/LCP 1.16 s
desktop / 3.0 s mobile-mid, while the plain-HTML control page (`/lander2`)
paints in ~0.28–0.30 s (3.9×/10.9× faster). The site-speed lane's own P5
prescription — route-level splitting + server-rendered critical HTML +
hydrate-after-paint — is a description of TanStack Start's defaults. And
our agent-readiness surfaces (`/.well-known/*`, robots, sitemap) had to be
hand-built on the API Worker precisely because the SPA shell serves agents
nothing; SSR closes that class structurally.

**Recommended architecture** (tanstack.com's shape, adapted):

- A new **`apps/openagents.com/apps/start`** package: TanStack Start +
  `@cloudflare/vite-plugin`, custom `src/server.ts` Worker entry (security
  headers, agent content-negotiation, the well-known surfaces moving in
  from the API Worker over time). Deployed as **its own Worker** bound to
  the web routes, with the existing `workers/api` Worker unchanged as the
  authority (service binding + same-origin routing). Two Workers, one
  domain: web shell vs API authority — a seam we already respect
  internally. (Merging into one Worker à la tanstack.com is possible
  later; separate-first keeps the API Worker's blast radius zero.)
- **Route migration order = funnel first**: landing, `/business`,
  `/blog`, `/docs`, `/code/download`, vertical pages — the pages where
  SSR/SEO/agent-readability pay immediately and where the StarCraft theme
  reset already zero-based the content. The heavy logged-in panels (Khala
  chat, fleet views, forum) migrate last or stay client-rendered.

**Preserving Effect** — no compromise needed. Effect is
framework-agnostic: it is the substance of the API Worker, the services,
`sync-worker`, and every `@openagentsinc/*-effect` package, none of which
change. In the Start app: server functions and loaders run Effect programs
(`Effect.runPromise` at the handler boundary, or a small
`effect-start` bridge helper package with managed runtime + per-request
context — worth building once, reusing everywhere). Effect Schema stays
the single contract language (the Start app consumes the same schemas the
API serves). The CLAUDE.md rule's *Effect half* survives intact; only the
*Foldkit-for-web half* is on the table.

**Preserving Foldkit** — tactically, via Start's own escape hatch:
`ssr: false` routes are client-only, and a Foldkit program is just
`Runtime.run(program)` against a container element — exactly how the
isolated `lander3-scene` library already mounts. So during migration,
existing Foldkit surfaces (the app shell panels) mount unchanged inside
CSR routes; Three.js/`three-effect` scenes likewise (they're
DOM-container-based and framework-indifferent). Long-run, the honest
statement is: **the web surface converges on React** (that's what "build
in what Sites builds in" means), Foldkit remains the desktop direction
(`autopilot-desktop` patterns; the Khala Code Effect-integration plan)
unless a later decision extends this one — and `packages/ui` design
tokens (CSS/Tailwind) port cleanly since the uniform-StarCraft-blue theme
lives in tokens, not in Foldkit views.

**Honest costs**: greenfield React in a repo with zero TanStack today; a
dual-framework interim (bounded by the CSR-mount bridge); Start is 1.x
but fast-moving — pin versions and use the parity-contract discipline we
already run against Codex upstream; the CLAUDE.md/AGENTS/INVARIANTS
default must be updated in the same change as the decision, or every
future agent will fight the migration.

## 5. Khala Sync × TanStack DB — the adapter is the client half

This is the strongest single finding. TanStack DB's backend contract
(`SyncConfig`, `db/src/types.ts:327`) is a plain object whose `sync`
function drives `begin() / write(change) / commit() / markReady() /
truncate()`, with optional `loadSubset` (cursor/offset-based on-demand
sync) — **no Electric dependency**. Compare the Khala Sync embryo we run
in production: per-scope monotonic cursors, `readChangesAfter(scope,
cursor)`, snapshots, `put|patch|delete` change rows, WS rooms for wake-up.
The mapping is almost mechanical:

| TanStack DB SyncConfig | Khala Sync embryo (shipping) |
| --- | --- |
| `sync()` subscribe | join `SyncRoomDurableObject` scope room (WS) |
| initial load + `markReady()` | `readSnapshot(scope)` → cursor |
| `begin/write/commit` batch | `readChangesAfter(scope, cursor)` batch on each poke |
| `truncate()` + resync | the design's `must-refetch` / compaction path |
| `loadSubset({cursor})` | cursor-paged catch-up (already the API shape) |
| optimistic `onInsert/onUpdate/onDelete` → matching strategy | named mutators through `sync_mutations`; **`mutation_id` accept/reject is our txid** — cleaner than Electric's Postgres-txid matching because idempotent mutation ids are already first-class in the outbox |

So: build **`@openagentsinc/khala-sync-db-collection`** —
`khalaSyncCollectionOptions({ scope, collection, mutators })` — as the
canonical client half of Khala Sync. This **supersedes the bespoke
`khala-sync-client` v0 plan** from the mobile report (KS-2): instead of
hand-rolling a store, we implement one adapter and inherit live queries
(`useLiveQuery`, incremental view maintenance), optimistic overlay with
automatic rollback, multi-collection transactions, `localStorage`/
`localOnly` collections for free, and the persistence layer below. The
empty `packages/khala-sync-client` becomes this adapter (plus a thin
non-React core if the CLI wants it). Server side changes not at all —
the outbox, scopes, and mutation machinery are exactly what the adapter
consumes; the Khala Sync design's Postgres/Hyperdrive Phases 1–2 proceed
underneath unchanged.

Bonus: `cloudflare-durable-objects-db-sqlite-persistence` means a DO can
hold a *materialized* collection server-side — a plausible future for
per-scope read replicas — and `offline-transactions` is the maintained
path to the offline queue the sync design deliberately deferred.

## 6. One UI ecosystem — owner decision recorded (2026-07-04)

An earlier revision of this section presented the fork ("one UI ecosystem
across web/desktop/mobile vs two") as an open flag. **The owner has since
decided: ONE UI ecosystem — React + Tailwind everywhere, with React
Native via Expo for mobile** ("yes expo, then we don't need both Swift
and Kotlin apps"). This section now records that decision and carries the
velocity assessment the owner asked for. Two standing rules change with
it, and the rule text updates ride the same change set: the CLAUDE.md
Foldkit-for-web default, and the 2026-06-26 no-Expo mobile mandate
(reversed for the Expo framework; see the EAS nuance in §6.3).

### 6.1 The velocity assessment: does React/Tailwind actually make us faster?

Yes — and the strongest argument is specific to *how this company builds*.

1. **Our engineering throughput is fleet throughput, and the fleet is
   dramatically more fluent in React+Tailwind than in Foldkit.** React is
   the largest UI corpus in every frontier model's training data;
   Tailwind is its styling lingua franca. Foldkit is an in-house
   Elm-architecture framework that requires curated rules
   (effect-solutions consultation is a standing instruction), produces
   more correction cycles per PR, and gives reviewers fewer ecosystem
   priors to check against. This is the harness-evolution and Lovable
   thesis pointed at ourselves: *primitives the model already knows are a
   direct quality and speed lever for AI-generated code.* Every workday
   here is dozens of agent-written UI diffs; shifting them onto the
   stack the models are best at is a compounding velocity gain, not a
   taste preference.
2. **One component system, three surfaces.** Web (Start), Khala Code
   desktop (Electrobun **is** web tech — React drops into it without
   changing the runtime), and mobile (Expo RN). Honest boundary: React
   DOM components do not render in RN — what's fully shared across all
   three is business logic, Effect services, schemas, the TanStack DB
   data layer (khala-sync-db-collection), hooks, and the design tokens;
   styling parity on RN comes via NativeWind (Tailwind-for-RN) over the
   same token set, and component *structure* stays parallel even where
   the leaf primitives differ (`div` vs `View`). Web↔desktop share
   literally everything.
3. **Maintenance we stop paying.** Foldkit is ours to maintain, document,
   and teach to every agent; the vanilla-DOM desktop shell (2,598 lines)
   was due for a rewrite regardless (the planned Foldkit migration).
   Redirecting that rewrite to React costs nothing extra *because it has
   not started* — this was the entire point of flagging the fork now.
   The React path replaces framework-maintenance hours with ecosystem
   leverage (TanStack, shadcn/radix-class components re-skinned onto the
   StarCraft tokens, testing tooling our QA harness already drives via
   the DOM).
4. **What we give up, named honestly.** Foldkit's Elm discipline —
   single-state-atom, message-typed updates, principled effects — is a
   real correctness asset; React's default culture is looser. Mitigation
   is architectural, not nostalgic: TanStack DB collections + live
   queries carry app state (not ad-hoc useState sprawl), Effect services
   stay the logic layer, `importProtection` and lint rules enforce the
   boundaries, and the UX behavior-contract registry keeps product
   behavior pinned regardless of framework. Effect is untouched
   everywhere — this decision is about the **view layer only**.
5. **Net:** faster agent PRs, one hiring/ecosystem story, one design
   system, one data layer, minus a framework we maintain ourselves. The
   velocity claim will be *measured*, not asserted: fleet cycle time and
   review-minutes per merged UI PR are already instrumented (BF-7.2
   metric shapes) — compare Foldkit-era vs React-era after TS-2 lands.

### 6.2 What one ecosystem looks like, surface by surface

- **Web** — TanStack Start on the tanstack.com Worker pattern (§4);
  React + Tailwind 4 + tokens; Foldkit surfaces bridge via `ssr:false`
  CSR mounts strictly as a migration vehicle, deleted route-by-route.
- **Khala Code desktop** — Electrobun stays (runtime unchanged, all the
  Codex/RPC/fleet machinery untouched); the shell rewrite goes to
  **React + Tailwind instead of Foldkit**. The Effect-integration audit's
  substance survives intact — its schema-first RPC contracts and scoped
  process/services phases are view-layer-independent; only its "staged
  Foldkit shell migration" phase is superseded. Existing UX behavior
  contracts are the safety net for the shell swap: the oracles don't care
  which framework renders the pixels.
- **Mobile** — **Expo React Native app** replacing the both-Swift-and-
  Kotlin future: one codebase for iOS + Android, expo-modules API for the
  native pieces we already wrote in Swift (push-to-talk/STT, the Apple FM
  bridge port as an Expo native module), TanStack DB
  `expo-db-sqlite-persistence` + khala-sync-db-collection as the data
  layer, NativeWind + tokens for the theme. Expo Router provides typed
  file-based routing on mobile (TanStack Router does not support RN —
  accepted; routing is the one per-surface piece).
  The shipped SwiftUI app remains the interim companion and the
  reference implementation for the native modules until the Expo app
  reaches parity; the chat-sync dogfood milestone does not wait for the
  rewrite (see §6.4).
- **Shared packages** — `@openagentsinc/ui` evolves to React components
  on the token system; `khala-sync-db-collection`, schemas, contracts,
  and Effect services shared verbatim across all three.

### 6.3 The Expo mandate reversal, precisely

The 2026-06-26 mandate ("NO Expo/EAS cloud; native SwiftUI only") is
**reversed for the Expo framework by owner direction (2026-07-04)**. Two
nuances preserved deliberately:

- **Build/ship path**: `expo prebuild` + local Xcode/Gradle + the proven
  `altool` TestFlight lane keeps working with Expo — local-first
  building remains the default posture. **EAS cloud builds/updates
  remain owner-gated** until the owner explicitly re-enables them (the
  original mandate's sharpest edge was EAS cloud specifically; "yes
  expo" re-admits the framework, and EAS can be a separate convenience
  decision).
- **OTA**: the own-OTA publisher built for the previous Expo era
  (`apps/oa-updates` + `publish-ota.sh`, `updates.openagents.com`)
  becomes relevant again — we can ship JS updates over our own feed
  without EAS.

### 6.4 What this changes in yesterday's mobile report

The mobile report's Option A (SwiftUI + Swift protocol port) is
**superseded as the destination** — Option B wins by owner decision, with
the calculus improved by §5 (the RN app inherits the entire TanStack DB
data layer rather than hand-rolling anything). What survives: the
SwiftUI app as interim companion + native-module reference; the
chat-sync dogfood milestone (KS-1/KS-3/KS-5) unchanged; KS-2 superseded
by khala-sync-db-collection (TS-3); **KS-4 (Swift protocol port)
canceled** — the Expo app consumes the TS client directly, and the
conformance-fixture machinery shrinks to whatever non-JS consumers ever
exist. The near-term test can still run on the SwiftUI app via the
simplest possible path (direct mutation HTTP calls + WS refetch) without
building the full Swift client that KS-4 described.

## 7. Dogfood loop (why this compounds)

The full loop the owner is pointing at: we migrate our site to Start →
we learn the stack's paper cuts as customer number one → the lessons
become the Sites rules pack + behavior contracts → Autopilot Sites
generates Start apps our fleet can actually maintain (typed routes and
server-fn boundaries are also guardrails for *our* coding agents, per the
harness-evolution thesis: deterministic structure beats prompt cleverness)
→ generated sites are agent-ready and QA-Swarm-verified by construction →
site quality becomes a sellable, receipted claim → and the same TanStack
DB adapter that powers our chat sync powers real-time features in
customer sites on the same Khala Sync spine. Every layer of that loop is
billable somewhere in the current revenue plan.

## 8. Risks and mitigations

1. **Framework churn** (Start 1.x velocity): pin + vendor a parity
   contract (upstream-pin discipline already proven on Codex).
2. **Bundle regression in React**: per-route splitting is Start's
   default and `importProtection` + budget tests (site-speed lane
   budgets) hold the line; the current 4.1 MB monolith is the low bar.
3. **Dual-framework drift**: time-box the Foldkit-in-CSR bridge; every
   migrated route deletes Foldkit code in the same PR.
4. **TanStack DB is beta** (0.6): the adapter isolates us — worst case we
   own the store behind the same `khalaSyncCollectionOptions` surface;
   the SyncConfig contract is small enough to reimplement.
5. **Working-rule conflict**: CLAUDE.md currently mandates Foldkit for
   this app; nothing ships until the owner signs the default change and
   the rule text updates in the same PR.
6. **WfP build costs** for user sites (container builds): metered tier
   exists; receipts per build; budget caps per site.

## 9. Owner decision points — status after the 2026-07-04 direction

1. ~~Approve the direction~~ **DECIDED**: one UI ecosystem — React +
   Tailwind across web/desktop/mobile; TanStack Start for the web surface
   and as Autopilot Sites' canonical output; Expo React Native for
   mobile. CLAUDE.md rule text updates ride this change set.
2. Sequencing: funnel pages first (recommended) vs Sites template first —
   still open; recommendation stands.
3. ~~Desktop-shell question~~ **DECIDED**: Khala Code shell rewrite goes
   to React + Tailwind (Foldkit shell migration superseded before it
   started); Electrobun runtime unchanged.
4. ~~Mobile~~ **DECIDED**: Expo RN app is the destination (one codebase,
   no separate Swift + Kotlin apps); SwiftUI app is interim + native-
   module reference; **EAS cloud remains owner-gated** as a separate
   convenience decision (§6.3); chat-sync dogfood milestone proceeds now
   on the interim app.

## 10. Proposed workstream map (TS; not filed — follow-up filing pass next)

| Task | Description | Depends |
| --- | --- | --- |
| TS-1 | `effect-start` bridge helper (managed Effect runtime + per-request context in Start server fns/loaders) + pinned-version parity contract | — |
| TS-2 | `apps/start` Worker: funnel pages (landing, /business, /blog, /docs, /code/download) SSR'd on the tanstack.com pattern; well-known agent surfaces served from the shell; site-speed budgets as merge gates | TS-1 |
| TS-3 | `@openagentsinc/khala-sync-db-collection` (SyncConfig adapter over scope rooms + cursors + mutation-id matching) — supersedes KS-2; first consumer = a live surface in TS-2 or the chat-sync milestone | — |
| TS-4 | Autopilot Sites Start template v1 + containerized build lane (version → build → WfP module → existing gates), agent-ready surfaces baked in | — |
| TS-5 | Sites rules pack + per-site behavior contracts (the Lovable curated-rules lesson, enforced our way) | TS-4 |
| TS-6 | Web app-shell panel migration (Foldkit-in-CSR bridge, route-by-route, delete-as-you-go) | TS-2 |
| TS-7 | Khala Code desktop shell rewrite in React + Tailwind (replaces the planned Foldkit shell migration; Electrobun + RPC/fleet machinery untouched; existing UX behavior contracts as the regression net) | TS-2 patterns helpful, not blocking |
| TS-8 | Expo RN companion app v0: Expo Router shell, NativeWind + tokens, khala-sync-db-collection + expo-sqlite persistence, expo-modules ports of the Swift voice/Apple-FM pieces; local prebuild + Xcode/Gradle builds; own-OTA feed wiring | TS-3; KS-1 server work |
| TS-9 | `@openagentsinc/ui` React edition on the shared token system (StarCraft theme), consumed by TS-2/TS-6/TS-7 and (via NativeWind parallel) TS-8 | TS-1 |
| TS-10 | Fleet UI velocity measurement: cycle-time and review-minutes per merged UI PR, Foldkit-era baseline vs React-era, on the BF-7.2 metric shapes — the §6.1 claim gets a receipt | TS-2 |

Start-now set: TS-1, TS-3, TS-4 scaffolding, TS-9; TS-7 before any
further Foldkit-shell effort is spent in the desktop app.
