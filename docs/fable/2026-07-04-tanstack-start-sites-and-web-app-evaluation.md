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

**Recommended architecture — sharpened by owner directive (2026-07-04):
clone tanstack.com's setup as exactly as practical.** The local clone at
`projects/tanstack/repos/tanstack.com` is the template, not just an
inspiration. "Stick very close to tanstack.com generally":

- A new **`apps/openagents.com/apps/start`** package whose skeleton is
  copied structurally from the tanstack.com repo: the same
  `vite.config.ts` plugin order (`cloudflare({viteEnvironment:{name:
  'ssr'}})` → `tanstackStart()` with `importProtection` + router
  code-splitting defaults + manual vendor chunks → `viteReact()` →
  `@tailwindcss/vite`), the same `tsr.config.json` shape
  (`src/routes` → `routeTree.gen.ts`), the same custom `src/server.ts`
  Worker entry wrapping the Start `server-entry` handler (security
  headers, content negotiation, `scheduled` crons, per-request
  AsyncLocalStorage env/context), the same `src/router.tsx` pattern
  (QueryClient in context, `defaultPreload: 'intent'`,
  ssr-query integration), the same `src/start.ts` global-middleware
  shape, and the same `wrangler.jsonc` structure (custom `main`, Workers
  Assets binding, `nodejs_compat`). Their `.agents/tanstack-patterns.md`
  seeds our rules pack. Deviations from the template are the exception
  and get a one-line justification in the PR.
- **Same component/design/Tailwind setup as tanstack.com** — adopt their
  Tailwind 4 configuration and component conventions as the baseline,
  then swap the palette to our StarCraft-blue token system (tokens are
  the theme; the setup is theirs). `@openagentsinc/ui` React edition
  (TS-9) grows out of this baseline rather than being invented parallel
  to it. Drop what we don't need (their Sentry/content-collections/
  drizzle wiring) rather than porting it unexamined.
- **Deployed to a NEW Worker with a visible staging URL ASAP — never
  replacing the live openagents.com Worker during bring-up.** First
  deploy goes out the day the scaffold builds: the Worker's
  `*.workers.dev` URL immediately, plus a `start.openagents.com` (or
  `staging.openagents.com`) custom domain when convenient. The existing
  `workers/api` Worker stays untouched as the authority (service
  binding/API calls cross to it); production route cutover happens only
  later, per-route, after the staging surface proves out.
- **The current `apps/web` Foldkit app is hereby deprecated**: no new
  pages land there; its pages move into the new system **funnel first
  and ASAP** — landing, `/business`, `/blog`, `/docs`, `/code/download`,
  vertical pages (where SSR/SEO/agent-readability pay immediately and
  the StarCraft theme reset already zero-based the content) — then the
  rest; heavy logged-in panels migrate last or stay client-rendered.
  Each ported page deletes its Foldkit counterpart at production
  cutover.

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

- **Builds**: local for now — `expo prebuild` + local Xcode/Gradle + the
  proven `altool` TestFlight lane. `eas build`/`eas submit` stay unused
  unless the owner explicitly changes that later.
- **Updates: we already built the drop-in EAS Updates replacement, and we
  preserve it** (owner clarification, 2026-07-04). The **OpenAgents
  Updates server** (`apps/oa-updates`) implements the expo-updates
  protocol (`expo-protocol-version: 1`), signs manifests with
  `expo-signature` code signing, stores assets, and models update
  channels/branches + runtime fingerprints; it runs on OpenAgents cloud
  behind `updates.openagents.com`, with the publish pipeline in
  `scripts/publish-ota.sh` (compute runtime fingerprint → `expo export` →
  bake as seed → deploy) — fully off Expo's CDN, and it also carries the
  desktop and Pylon signed-release feeds. The TS-8 Expo app embeds
  `updates.url` → our server and ships OTA through it; `eas update` is
  not part of the stack at all. One repair item for TS-8: `publish-ota.sh`
  still points at the retired `AutopilotRemoteControl` mobile path and
  gets repointed to the new app.

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
   module reference; **updates ship via our own EAS-Updates-replacement
   server (`apps/oa-updates` → `updates.openagents.com`), builds stay
   local for now** (§6.3); chat-sync dogfood milestone proceeds now on
   the interim app.

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

## 11. The transition plan (added 2026-07-04, post-decision — ordered and specific)

This section turns the decision into a sequenced execution plan, factoring
in the **live Khala Sync epic ([#8282](https://github.com/OpenAgentsInc/openagents/issues/8282))**,
whose state materially changed the TS map's assumptions since §5 was
written:

**Khala Sync ground truth (verified against the issue tracker 2026-07-04):**

- **The engine is DONE.** KS-0 (Cloud SQL + Hyperdrive + migration runner),
  KS-1 (contracts + conformance fixtures), KS-2 (outbox writer, version
  allocator, bootstrap/log reads, compaction/MustRefetch), KS-3 (mutator
  engine + `/api/sync/push` + registry), KS-4 (capture worker +
  `KhalaSyncHubDO` + offset-resumable catch-up), **KS-5 (client engine:
  bun:sqlite local store #8298, optimistic overlay + rebase #8299, session
  state machine + transport #8300, web SQLite-WASM opfs lane #8301)**,
  KS-7 (scope auth + access-change refetch #8305, CVR v2 #8306), and KS-9
  (load test, behavior contracts, INVARIANTS + runbook) are all closed.
- **First consumers are live**: fleet-cockpit scope projection (#8302),
  Khala Code desktop consuming the `fleet_run` scope instead of polling
  (#8303 — the desktop already consumes Khala Sync), and the public
  tokens-served projection (#8304).
- **What's open is the KS-8 domain-migration queue** (D1 → Cloud SQL,
  dual-write + backfill per domain): 8.7 billing/credits #8318, 8.8
  treasury/payouts #8319, 8.9 inference entitlements #8320, 8.10 forum
  #8321, 8.11 CRM/email #8322, 8.12 sites #8323, **8.13 Khala Code
  product state (threads/teams/workspaces → Cloud SQL + Khala Sync
  scopes) #8324**, 8.14 business funnel #8325, 8.15 training/gym #8326,
  8.16 forge #8327, 8.17 supervision tail #8328, 8.18 identity/auth
  #8329, 8.19 cron sweep + D1 retirement #8330, plus decommission
  follow-ups (#8331, #8333, #8334, #8335).

**Consequences for the TS map:** §5's "khala-sync-db-collection is the
client half" is now stated more precisely — the client half *exists*
(khala-sync-client engine, KS-5); **TS-3 is a TanStack DB `SyncConfig`
adapter wrapped around the shipped client engine**, not a from-scratch
store. And the mobile report's KS-1..KS-5 labels collide with the epic's
KS-* namespace — the chat-milestone items are hereby relabeled **MC-\***
(MC-1 chat collection + mutators, MC-3 desktop/web sidebar consumers,
MC-5 cross-device dogfood; MC-2/MC-4 were superseded/canceled in §6.4).

### 11.1 The ordering principles

1. **Don't couple a domain's Postgres migration to its UI rewrite** —
   except where the UI rewrite *is* the consumer proof (chat, by design).
2. **Revenue surfaces first** (funnel pages need zero sync work — ship
   them immediately); flagship demo second (cross-device chat); Sites
   third (it needs the build lane anyway); long-tail panels last.
3. **Baseline before the first React PR** — the §6.1 velocity claim needs
   the Foldkit-era measurement captured first, or the receipt is lost.
4. **One reordering request into the KS-8 queue**: pull **KS-8.13
   (#8324) ahead of 8.10–8.12** — it is the only KS-8 item that gates the
   one-UI plan (chat scopes = the cross-device dogfood, the desktop
   shell's first React surface, and the sync engine's flagship demo);
   forum/CRM/sites migrations gate nothing in this plan.
5. **Every React PR deletes the Foldkit/vanilla-DOM code it replaces** —
   no parallel implementations left standing.

### 11.2 The waves

**Wave 0 — foundations (start now; all parallel; no KS-8 dependency):**

| # | Work | Notes |
| --- | --- | --- |
| 0.1 | File the ONE-UI epic + TS-1..TS-10 issues (per EXECUTION.md conventions) | The filing pass this doc has been deferring |
| 0.2 | **TS-10a: capture the Foldkit-era velocity baseline** (cycle time, review-minutes per merged UI PR, from existing ledgers) | Must precede the first React merge |
| 0.3 | **TS-1: `effect-start` bridge** + pinned Start/Router/DB versions + parity-contract file | Unblocks all web work |
| 0.4 | **TS-9: `@openagentsinc/ui` React edition** on the shared tokens (StarCraft theme; NativeWind-compatible token export) | Unblocks TS-2/6/7/8 |
| 0.5 | **TS-3: `khala-sync-db-collection`** — TanStack DB `SyncConfig` adapter over the shipped khala-sync-client session engine; mutation-id matching via the KS-3 push route; **first consumer = the already-live `fleet_run` scope** (testable today, no chat dependency) | Proves the adapter against production sync before chat exists |
| 0.6 | KS-8 queue continues as scheduled: 8.7 billing → 8.8 treasury → 8.9 entitlements (the money-truth migrations stay first — they gate nothing here and matter most) | No change requested |
| 0.7 | **TS-2a: staging scaffold, day one** — clone the tanstack.com shape into `apps/openagents.com/apps/start` (vite config, server entry, router, wrangler per §4), StarCraft tokens over their Tailwind setup, one real page ported (the landing), **deployed to a NEW Worker with a visible staging URL immediately** (`*.workers.dev` first; `start.openagents.com` custom domain when convenient). Does not touch the live openagents.com Worker | Owner directive: a staging URL ASAP; only needs TS-1 stubs, not the full bridge |

**Wave 1 — the web funnel on Start (revenue-visible; ~immediately after 0.3/0.4):**

| # | Work | Notes |
| --- | --- | --- |
| 1.1 | **TS-2: port the funnel into the staging Worker** — `/business`, `/blog`, `/docs`, `/code/download`, vertical pages onto the 0.7 scaffold; well-known agent surfaces served from the shell; site-speed budgets as merge gates; owner reviews everything on the staging URL | Closes the site-speed P5 prescription and the agent-readability gap; zero sync coupling |
| 1.2 | Production cutover route-by-route onto the real domain only after staging sign-off (new Worker takes the route; API Worker untouched); the deprecated `apps/web` Foldkit page is deleted per cut-over route | The dual-framework window starts and stays bounded; openagents.com is never replaced wholesale |
| 1.3 | KS-8.14 (business funnel D1→Postgres) lands independently underneath — do not couple to 1.1/1.2 | Principle 1 |

**Wave 2 — chat scopes + the flagship demo (gated on the KS-8.13 pull-forward):**

| # | Work | Notes |
| --- | --- | --- |
| 2.1 | **KS-8.13 (#8324), reordered next in the KS-8 queue**: threads/teams/workspaces to Cloud SQL with Khala Sync scopes | The single blocking migration for everything below |
| 2.2 | **MC-1: chat collection + named mutators** (`chat.createThread` / `appendMessage` / `renameThread`) on the KS-3 mutator registry, owner-scoped | Small — the mutator engine and authoring guide (#8293) exist |
| 2.3 | **TS-7 begins with the chat sidebar**: Khala Code desktop's first React + Tailwind surface consumes thread scopes via TS-3; UX behavior contracts as the regression net; vanilla-DOM sidebar deleted | The shell rewrite and the sync consumer are the same PR series |
| 2.4 | **MC-3/MC-5: cross-device chat dogfood** — web (Start CSR or panel) + desktop sidebars live; interim SwiftUI app does mutation-HTTP + WS refetch; owner runs the phone↔desktop↔web round-trip; public-safe evidence bundle | The Khala Sync flagship receipt and the mobile milestone, unchanged in substance |

**Wave 3 — Sites on Start (after TS-4 scaffolding matures; overlaps Wave 2):**

| # | Work | Notes |
| --- | --- | --- |
| 3.1 | KS-8.12 (#8323) sites domain → Cloud SQL lands **before** template GA (don't build the new product on tables mid-migration) | Sequencing only; scaffolding can start earlier |
| 3.2 | **TS-4: Start site template v1 + containerized build lane** (version → build → WfP module → existing deploy gates); agent-ready surfaces baked in | The genuinely new machine |
| 3.3 | **TS-5: Sites rules pack + per-site behavior contracts**; first dogfood site generated end-to-end (a vertical landing page for our own funnel = the eat-our-own-output proof) | Lovable's curated-rules lesson |

2026-07-04 TS-4 landing note: `autopilot_sites.tanstack_start.v1` now
materializes a Start template, classifies `wrangler.jsonc` Start projects as
WfP Worker modules, and produces a dogfood OpenAgents funnel build-lane receipt
without live deployment or promise-state changes. See
`docs/fable/2026-07-04-ts-4-start-sites-template-build-lane.md`.

2026-07-04 TS-5 landing note: `sites_tanstack_rules.tanstack_start.v1.2026_07_04`
is injected into every Sites builder session, and generated Start sites now get
the starter behavior-contract set for dead controls, broken first-party nav,
LG-4 claim safety, and bundle budget before deploy review. See
`docs/fable/2026-07-04-ts-5-sites-tanstack-rules-and-contracts.md`.

**Wave 4 — the long tail (paced by capacity, after Waves 1–2 prove the stack):**

| # | Work | Notes |
| --- | --- | --- |
| 4.1 | **TS-6**: remaining web app-shell panels (Foldkit-in-CSR bridge, route-by-route, delete-as-you-go) | Paced; funnel + chat already migrated |
| 4.2 | **TS-7 remainder**: rest of the desktop shell to React (settings, history, fleet panels — fleet already syncs via #8303, so consumers move onto TS-3 as they're rewritten) | |
| 4.3 | **TS-8: Expo RN companion v0** — Expo Router shell, NativeWind tokens, TS-3 + expo-sqlite persistence, expo-modules ports of the Swift voice/Apple-FM pieces; local prebuild + Xcode/Gradle; own-OTA feed; replaces the interim SwiftUI app at parity | Needs TS-3 + chat scopes (Wave 2), nothing else |
| 4.4 | KS-8 remainder (8.10 forum, 8.11 CRM, 8.15–8.18) + **KS-8.19 cron sweep/D1 retirement + decommission follow-ups (#8331/#8333/#8334/#8335) close the era** | The D1-overload class dies here |
| 4.5 | **TS-10b: velocity receipt** — React-era metrics vs the 0.2 baseline, published internally | The §6.1 claim, measured |

2026-07-04 TS-6 slice note: the Start app now serves `/khala`, `/tassadar`,
`/gym`, `/activity`, `/business/kpi/$engagementRef`, `/clients-preview`,
`/components`, `/components/$family`, `/login`, `/preview/landing`, and `/run`
with route parity tests, keeping the existing Foldkit counterparts until a real
production route cutover allows delete-as-you-go. See
`docs/fable/2026-07-04-ts-6-start-khala-tassadar-route-slice.md`.

2026-07-04 TS-8 scaffold note: `clients/khala-mobile` now holds the Expo SDK 57
destination app with Expo Router, NativeWind over the shared token export, TS-3
Khala Sync read-model setup, an Expo SQLite persistence adapter, secure-store
key storage, delegation prompt validation, native module shells for STT and
Apple FM, and the OpenAgents Updates publish script repointed away from the
retired `AutopilotRemoteControl` path. The issue remains open for owner-gated
prebuild/Xcode/Gradle/TestFlight and signed OTA round-trip proof. See
`docs/fable/2026-07-04-ts-8-expo-mobile-scaffold.md`.

2026-07-04 TS-10b checkpoint note: the Foldkit-era velocity method is now
extracted as `bun run perf:ui-velocity`, and it reproduces the TS-10a baseline
numbers exactly. The React-era comparison remains time-gated: the later
dependency anchor is TS-7 phase 1 at `2026-07-04T21:36:04Z`, so the first
honest 30-day row is not eligible until `2026-08-03T21:36:04Z`; the full
30/60-day two-row table matures on `2026-09-02T21:36:04Z`. See
`docs/perf/2026-07-04-react-ui-velocity-receipt-checkpoint.md`.

### 11.3 Explicit dependency spine

```
0.2 baseline ─── (before any React merge)
0.3 TS-1 ──► 1.1 TS-2 ──► 1.2 cutover ──► 4.1 TS-6
0.4 TS-9 ──► (TS-2/TS-6/TS-7/TS-8 consume)
0.5 TS-3 (proves on fleet_run scope, live today)
        └──► 2.3 desktop chat sidebar ──► 4.2 TS-7 remainder
2.1 KS-8.13 ──► 2.2 MC-1 ──► 2.3/2.4 flagship demo ──► 4.3 TS-8 Expo app
3.1 KS-8.12 ──► 3.2 TS-4 GA ──► 3.3 TS-5
KS-8.7/8.8/8.9 (money) — independent, keep first in the KS-8 queue
KS-8.19 + decommissions — strictly last
```

### 11.4 The two actions that unlock everything

1. **File the ONE-UI epic** with TS-1..TS-10 (+MC-1/3/5) per this plan.
2. **Comment the KS-8.13 pull-forward on #8282/#8324** so the migration
   queue reorders 8.13 ahead of 8.10–8.12 — one sentence of sequencing
   that converts the chat demo from "mid-queue eventually" to "next."

Everything in Wave 0 is dependency-free and fleet-shaped; nothing waits
on anything the company hasn't already built.
