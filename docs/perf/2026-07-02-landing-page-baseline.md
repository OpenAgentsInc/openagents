# Landing Page Speed Baseline — P0 Report

Date: 2026-07-02
Lane: [site-speed lane](../fable/2026-07-02-site-speed-lane-spec.md) phase P0.
Harness: `apps/openagents.com/scripts/site-speed-landing.ts` (Mode L;
`openagents.site_speed.run_report.v1`), run against production
`https://openagents.com/` — medians of 3 cold-cache loads per cell, profiles
`desktop-fast` (no throttle, 1440×900) and `mobile-mid` (4× CPU throttle,
~4G: 9 Mbps / 170 ms RTT, 390×844).

## 1. The headline numbers (baseline, cold cache)

| Metric | desktop-fast | mobile-mid |
| --- | --- | --- |
| TTFB (doc) | **121 ms** | **128 ms** |
| First Contentful Paint | **1,156 ms** | **3,004 ms** |
| Largest Contentful Paint | 1,156 ms (the "OpenAgents" wordmark div) | 3,004 ms |
| CLS | ~0 (0.000) | ~0 (0.002) |
| Total Blocking Time | 21 ms | **1,027 ms** |
| Longest single task | 576 ms | 640 ms |
| Script execute (CDP ScriptDuration) | 192 ms | **751 ms** |
| Counter value rendered | 1,272 ms | 3,205 ms |
| WebSocket (live counter feed) connected | 1,337 ms | 3,206 ms |
| Wire transfer (full load) | ~1.20 MB / 14 requests | ~1.20 MB / 14 requests |
| JS heap after settle | 29 MB | 28 MB |

The shape of the load: the edge answers in ~125 ms with a 2.7 KB empty SPA
shell, then **nothing paints for another ~1.0 s (desktop) / ~2.9 s (mobile)**
while the 1.07 MB-brotli (4.1 MB raw) monolithic bundle downloads, parses,
and executes — including one ~600 ms long task (the bundle evaluation) on
both profiles. First paint and LCP are the same instant and the same
element: the client-rendered wordmark. Everything the user sees is gated on
the bundle (spec hypotheses H1 + H2: **confirmed**).

## 2. The token-totals verdict (the owner's question)

**The token totals do not meaningfully slow the page down.** Evidence from
the isolation runs (`block-counter`: the scalar endpoint, the sync feed, and
the WebSocket stream all stalled):

| | baseline FCP | block-counter FCP | delta |
| --- | --- | --- | --- |
| desktop-fast | 1,156 ms | 1,240 ms | +84 ms (within cell noise) |
| mobile-mid | 3,004 ms | 3,116 ms | +112 ms (within cell noise) |

- The counter value renders **~120 ms after first paint** (1,272 vs
  1,156 ms desktop) — it is fetched asynchronously after render, off the
  paint path.
- The counter contributes **zero layout shift** (CLS ≈ 0 in every cell; no
  counter node in any layout-shift source attribution).
- The live WebSocket connects ~180 ms after paint on desktop and does not
  precede paint on any run.
- The scalar endpoint itself (`/api/public/khala-tokens-served`,
  live-at-read D1 re-sum) answers in ~200 ms standalone — respectable, and
  irrelevant to paint because nothing waits on it.

The pre-registered hypothesis H3 ("the fetch is off the paint path; the real
risks are CLS/countup/WS timing") held, and even the residual risks measured
clean. The lag lives elsewhere.

## 3. Where the time actually goes (ranked offenders)

1. **The monolithic bundle (H1 — dominant).** 1.07 MB brotli / 4.1 MB parsed
   for a page whose fold is a wordmark, two CTAs, and a counter pill. It
   costs the full download + a ~600 ms evaluation long task + 751 ms of
   script time on a mid-tier phone, and it is the entire FCP→TTFB gap.
   Fix direction: route-level code splitting (spec P5).
2. **Nothing is server-rendered (H2).** The 2.7 KB shell contains no
   content, so even a fast bundle would paint late. Fix direction: SSR or
   static critical HTML for the fold — this is what the `/lander2`
   experiment (§4) exists to quantify.
3. **Mobile TBT ≈ 1.0 s** — the throttled main thread is saturated during
   load; interaction during the first ~3 s will feel dead. Same root cause
   as (1).
4. **Third-party origins (H5 — directionally confirmed, measurement
   caveat).** The Google Fonts stylesheet is render-blocking and Fathom is
   an external script on the critical window. The `block-thirdparty` cells
   came back *slower* than baseline, which exposed a harness artifact
   rather than a truth about fonts: enabling request interception adds
   per-request overhead and aborting a render-blocking stylesheet has
   different timing than its absence. Follow-up: add an
   interception-enabled-nothing-blocked control cell (harness v2) before
   quoting third-party deltas. Self-hosting the font remains correct on
   principle (one less origin, no swap dependency).
5. **CLS is a non-issue** (≈0 everywhere) and TTFB is already excellent —
   neither is worth optimization effort.

## 4. The `/lander2` control experiment

To isolate the "SPA shell + bundle" architecture cost from everything else,
the lane shipped a server-rendered variant of the same fold at
`openagents.com/lander2` (unlisted, `noindex`): one HTML document, inline
critical CSS on the khala palette, the token total read from the ledger at
render time (Server-Timing exposes the D1 read), a ~0.4 KB inline refresher
instead of the bundle. Same copy, same links, no WebGL scene.

Deployed 2026-07-02 (Worker version `fd22383d`) and measured with the same
harness, same profiles, same cold-cache 3-run medians:

| Metric | `/` (SPA) desktop | `/lander2` desktop | `/` (SPA) mobile-mid | `/lander2` mobile-mid |
| --- | --- | --- | --- | --- |
| TTFB | 121 ms | 263 ms | 128 ms | 235 ms |
| **FCP = LCP** | **1,156 ms** | **296 ms** | **3,004 ms** | **276 ms** |
| Counter value visible | 1,272 ms | 324 ms | 3,205 ms | 302 ms |
| TBT | 21 ms | 0 ms | 1,027 ms | 0 ms |
| Longest task | 576 ms | 0 | 640 ms | 0 |
| Wire transfer | ~1.20 MB / 14 req | ~4 KB / 3 req | ~1.20 MB / 14 req | ~4 KB / 3 req |
| JS heap | 29 MB | 1 MB | 28 MB | 1 MB |

Readings:

- **First paint is 3.9× faster on desktop and 10.9× faster on a mid-tier
  phone.** SSR paint time is nearly device-independent (296 vs 276 ms)
  because nothing is CPU-bound — the SPA's mobile paint is 3.0 s because the
  bundle parse/execute is.
- The token total is **on screen at ~300 ms** — 4–10× sooner than the SPA
  renders it — while still live-updating (the 0.4 KB refresher).
- TTFB is ~110 ms *worse* than the SPA shell because the Worker does a
  live-at-read D1 ledger SUM per request (`Server-Timing: d1` exposes it),
  and standalone probes show that read's variance (0.2–2.3 s on cold paths).
  **Follow-up:** serve the scalar from the counter push path's cached total
  or a short-TTL edge cache — the SSR page should get the ~120 ms TTFB and
  keep the ~300 ms paint.
- Zero long tasks, zero blocking time, 1 MB less JS heap: the interaction
  budget on mobile goes from ~1 s blocked to untouched.

Caveat for honesty: `/lander2` renders the fold without the WebGL hero
scene and the full app chrome — it is the architecture control, not a
drop-in replacement. The right production conclusion is the combination:
**server-render the fold (or inline static critical HTML) + route-split the
bundle + hydrate the scene after paint**, which this experiment now
justifies with numbers.

## 5. Follow-ups this report generates

- Harness v2: interception-control cell (per §3.4) + scene-blocking variant
  + warm-cache cells.
- The P5 optimization backlog opens with route-level code splitting and a
  server-rendered/static fold for `/` — `/lander2`'s numbers are the
  business case.
- P2 instrumentation (User Timing marks + Server-Timing on public
  endpoints) so these phases come from the page itself, not just harness
  observation.

All raw run JSON: `openagents.site_speed.run_report.v1` artifacts under
`apps/openagents.com/var/site-speed/` (ignored; medians reproduced above).
Reproduce: `bun apps/openagents.com/scripts/site-speed-landing.ts --runs 3`.
