# Site Speed Lane — Spec (openagents.com, landing page first)

Date: 2026-07-02
Status: **specification for a new, standing performance lane** on the
deployed website, starting with the landing page at `https://openagents.com`.
Owner-directed 2026-07-02. This lane is deliberately **separate from
ROADMAP_QA / QA Swarm** (those target Khala Code Desktop and the QA product;
this targets the production website with a deploy-driven cadence) — it shares
only the honest-evidence discipline: exact measurements, versioned report
schemas, budgets as data, `not_measured` over guesses. This doc flips no
promise state and broadens no public copy.

## 0. Mission

Know **exactly how long every part of the landing page takes to load** — the
document, the CSS, the JS bundle, fonts, the hero scene, the token-total
counters, images, API calls, the live WebSocket — on real connection
profiles, cold and warm; know **which parts block or slow the others** (the
owner's explicit question: do the token totals slow anything down?); encode
the answers as **named budgets**; and keep the measurements running so every
deploy and every optimization is judged by the same numbers.

## 1. Ground Truth — What The Landing Page Is Today

Verified against prod and the repo on 2026-07-02:

### 1.1 The serving shape

- **A ~2.7 KB SPA shell.** `GET /` returns a nearly empty HTML document
  (edge-cached, `cf-cache-status: HIT`, TTFB ~188 ms from this network).
  There is **no server-rendered content**: nothing meaningful can paint
  until the JS bundle downloads, parses, and renders. First paint is
  structurally gated on the bundle.
- **One monolithic JS bundle**: `/assets/index-<hash>.js` — **4.10 MB raw,
  1.07 MB brotli over the wire** (~600 ms fetch on a fast pipe; multiple
  seconds on 4G before parse even begins). No route-level code splitting:
  the landing page ships the code for every page, panel, and scene in
  `apps/web`.
- **One CSS file**: `/assets/openagents.css` — 376 KB raw / ~46 KB brotli,
  render-blocking stylesheet.
- **Third-party requests in the shell**: Google Fonts (Inter, `display=swap`,
  css2 + gstatic preconnects) and **Fathom analytics**
  (`cdn.usefathom.com/script.js`). Both are external origins on the critical
  page load; policy prefers Cloudflare primitives, so both are review
  candidates for this lane.
- **`<link href="/api/public/home">`** in the shell (~102 ms TTFB, ~2 KB) —
  home data reference; whether it is actually preloaded vs merely linked is a
  P0 measurement item.
- **Heavy static assets in `/assets`**: a 5.5 MB GLB model (`UEPerson`),
  multiple 200–540 KB JPEGs, more scene textures. Which of these the landing
  route actually requests (vs other routes reached through the same bundle)
  is a P0 waterfall question.

### 1.2 The token totals (the owner's named suspect)

The tokens-served counter has three cost surfaces, each measured separately:

1. **Initial value**: `GET /api/public/khala-tokens-served` re-sums the
   ledger **live-at-read** (measured ~200–207 ms TTFB). The countup
   controller (`khala-tokens-served-countup-controller.ts`) targets
   `[data-counter-display]` nodes.
2. **Live updates**: a **WebSocket** stream (`khalaTokensServedStream` in
   `apps/web/src/subscriptions.ts`) with cursor replay; the old poll is
   explicitly no longer the live path but remains as fallback/reconcile.
3. **Render cost**: the countup animation itself (rAF ticks re-rendering
   number text), potential **layout shift** as digit counts grow, and the
   per-message handling cost of the WS feed while the page is otherwise
   idle.

### 1.3 The hero scene

Foldkit custom elements (`landingSquaresElement`, `lightBeamsElement`, the
persistent scene family) render the animated hero. Costs to isolate: WebGL/
canvas context init, first scene frame, per-frame cost after settle (fps,
long-task profile), and whether scene startup contends with the counter and
hydration on the main thread.

### 1.4 Prior instrumentation

There is **no RUM** on the site today (Fathom is pageview analytics, not
performance), no `Server-Timing` headers from the Worker, no User Timing
marks in the page, and no lab harness. The desktop app's `qa-metrics`
budget-as-data pattern is the design precedent; none of it points at the
website yet.

## 2. The Questions This Lane Answers (ranked)

1. **What is the exact phase timeline** of a landing-page load —
   TTFB → CSS → JS downloaded → JS parsed/executed → first render (FCP) →
   hero rendered (LCP) → scene first frame → counter first value → counter
   first live WS update → interactive (INP-ready) → settled?
2. **Do the token totals slow anything down?** Specifically: (a) does the
   initial counter fetch block or delay any paint, (b) does the ~200 ms
   live-at-read API sit on any critical path, (c) does the countup animation
   or WS message handling produce long tasks / jank / CLS, (d) what does the
   counter cost per-frame while idle?
3. **What share of the cost is the monolithic bundle**, and what would the
   landing route cost if it shipped only its own code (code-splitting
   headroom)?
4. **What does the scene cost**, at init and per-frame, and does deferring
   it change FCP/LCP/INP?
5. **What do the third-party origins cost** (Google Fonts, Fathom), including
   connection setup and font-swap layout shift?
6. **Which large assets does the landing route actually pull** (GLB?
   JPEGs?), and are any fetched without being visible?
7. **How does all of this look on a mid-tier phone on 4G**, not just a fast
   desktop — and cold vs warm cache?
8. **Does any of it regress on a deploy** — and would we notice within a day?

## 3. Metrics And Mark Taxonomy

### 3.1 Standard vitals (per run, per profile)

`TTFB`, `FCP`, `LCP` (+ LCP element identity), `CLS` (+ worst shift
sources), `INP`/interaction latency probes, `TBT`, long tasks (count, total,
max), main-thread breakdown (parse/compile/execute/layout/paint), JS heap
after settle, transferred bytes and request count **by asset class** (doc /
js / css / font / image / model / api / ws / third-party).

### 3.2 Page-part marks (the User Timing schema, added in P2)

All in-page instrumentation uses one namespace so lab and field agree —
schema `openagents.site_speed.marks.v1`:

| Mark / measure | Meaning |
| --- | --- |
| `oa:boot:start` → `oa:boot:first-render` | bundle execute start → first Foldkit render commit |
| `oa:hero:first-frame` | first painted frame of the hero scene canvas |
| `oa:hero:settled` | scene reaches steady-state fps |
| `oa:counter:value-rendered` | tokens-served counter shows a real value (not placeholder) |
| `oa:counter:first-live-update` | first WS-driven counter change applied |
| `oa:counter:countup-done` | countup animation settled |
| `oa:fonts:swapped` | web font applied (Inter swap complete) |
| `oa:route:interactive` | route handlers attached; first interaction serviceable |

Per-frame counters (sampled, not per-tick): countup rAF cost, WS
message-handler duration, scene frame time distribution.

### 3.3 Edge/server metrics

`Server-Timing` headers from the Worker (added in P2) for: total worker CPU,
D1 query time on `/api/public/khala-tokens-served` and `/api/public/home`,
cache hit/miss per asset. Plus lab-side per-request timing already captured
by the harness.

## 4. Methodology — Three Modes

### Mode L — Lab harness (the core, built first)

A committed script (Bun + Playwright/CDP, same stack as every smoke in this
repo — no new tooling) that loads `https://openagents.com/` under a matrix
and emits a versioned JSON report:

- **Profiles**: `desktop-fast` (no throttle), `mobile-mid` (4× CPU throttle,
  ~4G network: 9 Mbps down / 170 ms RTT), `mobile-slow` (6× CPU, ~3G) —
  each **cold** (no cache) and **warm** (second view).
- **Runs**: N=5 per cell; report medians and p75; every value carries its
  spread. A run collects: CDP trace (Performance + Network domains), the
  §3.1 vitals via injected PerformanceObserver, the full resource waterfall,
  long tasks, a filmstrip of screenshots, and (once P2 lands) the §3.2 marks.
- **Isolation experiments** (the causal answers, not just observation) — each
  is a page load with one factor removed via request interception, diffed
  against baseline on the same profile:
  - `block:counter-api` (counter endpoints stalled/failed) — does anything
    else slow down or shift?
  - `block:websocket` — page without the live feed.
  - `block:scene` (scene canvas elements stubbed) — FCP/LCP/INP delta.
  - `block:fonts` / `block:fathom` — third-party cost.
  - `freeze:counter` (value renders once, countup disabled) — animation cost.
- **Read-only invariant**: the harness only issues GETs a normal visitor
  would; no auth, no mutations, no synthetic load loops against APIs.
  (Landing-page loads do not move public counters; N=5×6 cells is negligible
  traffic.)
- **Report schema**: `openagents.site_speed.run_report.v1` — commit sha /
  deploy version, profile, cell, vitals, marks, waterfall summary by class,
  isolation deltas, budget evaluations. Artifacts under ignored `var/`;
  the JSON is the artifact of record.

### Mode E — Edge probes (cheap, continuous)

The curl-class probes proven in this spec's own baseline (§1), scripted:
doc/CSS/bundle/API TTFB + wire size + cache status, run per-deploy and
nightly, appended to the trend series. Catches CDN regressions (cache-miss
storms, bundle growth) without a browser.

### Mode F — Field RUM (real users, aggregate-only)

A tiny beacon in the page (web-vitals: LCP/CLS/INP/TTFB + the §3.2 marks)
posting to a Worker endpoint that writes **Cloudflare Analytics Engine**
rows (per standing policy: Cloudflare primitives, no third-party perf SaaS).
Aggregate-only by design: no user identifiers, no per-user timelines, coarse
device/connection class only, sampled (e.g. 10%). Public-safe aggregates can
later surface on the QA status page. RUM is the truth that keeps the lab
honest — lab profiles get recalibrated against field p75s.

## 5. Hypotheses (ranked, falsifiable — P0 tests all of these)

- **H1 — The bundle dominates.** 1.07 MB br / 4.1 MB parse on the critical
  path of an empty shell puts FCP/LCP far behind TTFB; on `mobile-mid` the
  parse/execute alone likely exceeds every other cost combined. Test: phase
  timeline + main-thread breakdown. Fix direction (P5): route-level code
  splitting; landing route ships a fraction of the code.
- **H2 — Nothing paints without JS.** The 2.7 KB shell means LCP element is
  client-rendered; a static/SSR hero (even inlined critical HTML for the
  fold) would move FCP/LCP dramatically. Test: `block:scene` +
  bundle-throttled loads showing paint waits on execute.
- **H3 — The token totals are a minor but nonzero cost, in specific ways.**
  Prediction: the initial fetch (~200 ms) is off the paint path (async after
  render) and does NOT slow first paint; the real candidates are (a) CLS if
  the counter placeholder→value changes width, (b) countup rAF work
  contending during scene start, (c) WS connect during the busy window, and
  (d) the live-at-read D1 re-sum making the API's p75 worse than its p50
  under load. Test: `block:counter-api`, `freeze:counter`, API latency
  distribution (Mode E), CLS source attribution. This answers the owner's
  question with numbers either way.
- **H4 — Scene init is a long-task source.** WebGL/canvas init + first
  layout of animated elements lands as one or more >100 ms tasks near FCP.
  Test: `block:scene` delta on TBT/INP.
- **H5 — Third-party origins cost more than they return on this page.**
  Google Fonts (extra origin, swap shift) and Fathom (external script) each
  add connection setup on the critical window; self-hosting the font and
  deferring/replacing analytics are cheap wins. Test: `block:*` deltas.
- **H6 — Some heavy assets load without being needed.** The 5.5 MB GLB
  and/or large JPEGs may be fetched by the landing route (bundle-referenced)
  without being visible at the fold. Test: waterfall class audit.

## 6. Budgets (initial, data-backed after P0)

Encoded as data (`openagents.site_speed.budget.v1`), evaluated by every lab
run and the nightly; initial targets set from the baseline and tightened
after P0 evidence. Two tiers: `desktop-fast` / `mobile-mid`.

| Budget id | desktop-fast | mobile-mid |
| --- | --- | --- |
| `ttfb.doc` | < 250 ms | < 400 ms |
| `fcp` | < 1.0 s | < 2.5 s |
| `lcp` | < 1.8 s | < 3.5 s |
| `cls` | < 0.05 | < 0.05 |
| `inp.probe` | < 200 ms | < 300 ms |
| `tbt` | < 200 ms | < 600 ms |
| `long_task.max` | < 200 ms | < 350 ms |
| `js.wire` (landing route) | < 350 KB br (post-split target; today 1.07 MB) | same |
| `counter.value_rendered` | < 1.2 s | < 3.0 s |
| `counter.cls_contribution` | 0 | 0 |
| `hero.first_frame` | < 1.5 s | < 3.5 s |
| `hero.settled_fps` | ≥ 55 | ≥ 30 |
| `api.tokens_served.p75` | < 300 ms | (edge metric, profile-independent) |

Today's single-bundle reality will fail `js.wire` (and likely several paint
budgets on `mobile-mid`) **by design** — budgets state the target, the P0
report states the truth, and the gap is the optimization backlog.

## 7. Deliverables And Phases

- **P0 — Baseline report (first build step, no product code changes).**
  Build the Mode L harness + Mode E probe script; run the full matrix +
  isolation experiments against prod; publish
  `docs/perf/2026-07-XX-landing-page-baseline.md` with the phase timeline,
  the H1–H6 verdicts (including the definitive token-totals answer), and the
  ranked offender list with estimated wins. **This is the very next build
  step of the lane.**
- **P1 — Harness committed + repeatable.** `site-speed` scripts live in the
  repo (`apps/openagents.com` scripts dir), `bun run perf:landing` runs a
  cell locally; report schema + budget evaluation included; runbook doc.
- **P2 — Instrumentation.** The §3.2 User Timing marks in `apps/web` (guarded,
  ~zero-cost), `Server-Timing` on the Worker's public endpoints, and a
  `?perf=1` debug overlay rendering the marks locally. No copy changes; no
  behavior changes.
- **P3 — RUM.** The web-vitals + marks beacon → Worker → Analytics Engine
  dataset; aggregate queries; lab-vs-field calibration note.
- **P4 — The loop.** Mode E on every deploy (post-deploy check per
  `docs/DEPLOYMENT.md` flow) + the Mode L matrix nightly on the owned runner;
  trend series; budget regression ⇒ auto-filed issue (same pattern as
  ROADMAP_QA Q2.5, separate lane and separate dataset).
- **P5 — The optimization backlog, executed.** Fix in evidence order from P0
  (expected shape: route-level code splitting → static/inline first-paint
  hero → defer scene start until first paint → self-host fonts → counter
  CLS/rAF fixes if H3 confirms → image/GLB loading discipline), each landing
  with before/after harness runs in the PR body, budgets ratcheting as they
  turn green.

Extension after the landing page proves the loop: `/stats`, `/khala`,
`/business`, `/forum`, `/blog`, `/docs` — one page at a time, same harness,
same schema, per-route budgets.

## 8. Invariants

- **Prod is measured, never mutated**: visitor-shaped GETs only; no
  synthetic API load; sampling volumes negligible.
- **Exact-only, honest states**: every number from a real trace or a real
  header; `not_measured` where a mode hasn't run; medians with spread, never
  single anecdotes.
- **Cloudflare primitives for RUM** (Analytics Engine via our own Worker);
  no third-party perf SaaS; aggregate-only, no user identifiers.
- **Instrumentation is invisible**: marks and beacons must not measurably
  change what they measure (guard: harness diff with `block:beacon`).
- **No copy or visual changes ride this lane** — optimization PRs change
  loading behavior, not content; anything user-visible (e.g. a static hero)
  gets its own explicit review.
- **Budgets are data**, versioned next to the report schema; a budget change
  is a reviewed diff, never a silent edit to make a run pass.
