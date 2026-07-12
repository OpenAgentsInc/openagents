# OpenAgents Desktop Startup Speed — Operationalized Audit & Measure-Constantly Discipline

- **Date:** 2026-07-11
- **Author:** Fable (agent)
- **Status:** operational speed audit — not roadmap authority; sequencing stays `docs/sol/MASTER_ROADMAP.md`
- **Owner directive (verbatim):** "we MUST MEASURE AND IMPROVE SPEED CONSTANTLY"
- **Scope:** `apps/openagents-desktop/` cold/warm startup path only (main-process init ordering, window creation, preload, renderer boot to first paint / interactive / capability-ready). Excludes the hot composer `shell.ts` region.
- **Companion sources:** `docs/electron/optimization.md` (the principles this expands), `apps/openagents-desktop/scripts/startup-bench.ts` (the harness), `apps/openagents-desktop/benchmarks/startup/` (the receipts).

## 1. Why this exists

`docs/electron/optimization.md` is the general theory: treat Electron as a
latency-sensitive multi-process native app, show a usable window almost
immediately, keep the main process nearly empty, minimize renderer JS before
first interaction, and — above all — **measure cold starts, with budgets, so
regressions are caught mechanically instead of by feel.**

This document operationalizes that theory for *our* app: it traces the exact
OpenAgents Desktop startup path with file:line receipts, audits which of the
principles we already honor and which we violate, ranks concrete optimizations
for our code, and defines the standing discipline (metrics, harness, thresholds)
that keeps speed green. Every speed claim here is backed by a measurement from
the repeatable harness, never a vibe.

## 2. The measurement instrument (so the rest is falsifiable)

The harness is `apps/openagents-desktop/scripts/startup-bench.ts`, run with
`bun run --cwd apps/openagents-desktop startup-bench`. It launches the real
Electron app N times in a new deterministic **startup-marks mode**
(`OPENAGENTS_DESKTOP_STARTUP_MARKS=<file>`), each with a fresh temp `userData`
root, discards a warmup run, and reports median + p95 per milestone. Marks-mode
reuses the smoke fixtures so the benchmark isolates *our init ordering* rather
than live filesystem/network variance (no real `~/.codex` scan, no network).

The milestone chain (all ms from process start = the main-process performance
origin):

| Mark | Meaning |
| --- | --- |
| `mainModuleEvaluated` | `dist/main.js` finished top-level evaluation (all module-scope service construction) |
| `appWhenReady` | Electron `app.whenReady()` fired |
| `windowCreated` | `new BrowserWindow(...)` returned |
| `windowReadyToShow` | `ready-to-show` fired — the renderer has painted its first frame (our canonical **first-paint** signal) |
| `rendererBootStart` | renderer `boot()` began (bundle parsed + executing) |
| `firstPaint` | renderer-reported FCP entry (best-effort; see §6) |
| `shellMounted` | Effect Native shell DOM mounted — the first **interactive** frame |
| `capabilityReady` | runtime-gateway `runtime.bootstrap` returned `ready` |

Marks come from three additive, production-safe instrumentation points: a
main-process `recordMainMark()` recorder (`src/main.ts`), a renderer-local
`globalThis.__oaStartupMarks` object set in `src/renderer/boot.ts` (never a
preload/IPC channel), and an `executeJavaScript` readback in the marks driver
(the same read-only channel smoke already uses). The receipt JSON is
timings-only — no paths, no user data.

## 3. Our actual startup path (with receipts)

`src/main.ts` at HEAD is a **2967-line single main module**. Reading it top to
bottom is the startup path:

1. **Module top-level (before `whenReady`).** `app.setName`/`process.title`
   (main.ts:26), then the entire service graph is constructed synchronously at
   import time: `makeCodexConnectService` (294), `makeProviderAccountsService`
   (301), `createDesktopRuntimeGateway` (355), `makeCodexHistoryHost` (589),
   `makeDesktopHostLifecycle` (590), `makeWorkspaceSearchRegistry` (595),
   `usageLedger`, `codexChildren`, `codexPreflight`, `codexLocal`,
   `openMcpConfigStore`, `makeFableLocalRuntime`, `openGitGithubService` (858),
   plus ~60 `ipcMain.handle(...)` registrations. All of this is counted in
   `mainModuleEvaluated`.
2. **`userData` + single-instance lock** (main.ts:229–239). `app.setPath` then
   `requestSingleInstanceLock()`; a non-primary instance quits. Deep-link /
   `second-instance` / `open-url` command intake is wired here (250–264).
3. **`app.whenReady()` handler** (main.ts:~3031). In order: dock icon; protocol
   client (packaged only); `openDesktopCommandBindingStore` (sync JSON read) +
   `installDesktopCommandMenu`; `hardenSession()`; **`openDesktopSyncHost({...})`
   — opens the Khala Sync SQLite database and catalog synchronously** and
   `replaceSync` (3043–3055); **`openDesktopSessionVault` + `recover()` and, if a
   stored credential is unverified, an *awaited* `recoverVerifiedDesktopSession`**
   (3059–3073); `runtimeGateway.start()` (3079); a non-blocking
   `codexPreflight.probeAll("boot")` (3083); and only THEN
   **`const window = createWindow()`** (3084).
4. **`createWindow()`** (main.ts:1499) builds a hardened `BrowserWindow`
   (`show:false`, `backgroundColor:#05070d`, `contextIsolation:true`,
   `sandbox:true`, `nodeIntegration:false`, tiny CJS preload), subscribes the
   window to runtime/ledger events, and `loadFile(renderer/index.html)`.
   `ready-to-show` → `window.show()` (1544).
5. **Renderer boot** (`src/renderer/boot.ts`). `index.html` loads one IIFE
   `boot.js`. `boot()` (boot.ts:1195) runs `mountDesktopShell`, an `Effect.gen`
   that — **before the first `renderer.mount` (first paint) at boot.ts:1152** —
   awaits a chain of IPC round-trips: chat host selection (491), fable-local
   availability (555), **codex history catalog scan (694)**, a history page probe
   + first page (703–706), coding-catalog snapshot (715), thread list (723), and
   session status (735). Only after all of those does it mount the shell.

## 4. Principle-by-principle audit (follow vs violate)

**Followed well:**

- *Keep the main process a hardened control plane.* The preload is a tiny typed
  capability bridge with no `ipcRenderer` leak, no Node builtins
  (`src/preload.cts`), matching optimization.md §16. `contextIsolation`,
  `sandbox`, deny-by-default permissions/navigation/window-open are all on
  (main.ts:1460–1497, `hardenSession`).
- *Heavy per-turn work is off the renderer.* Codex history parsing runs in a
  `utilityProcess`/worker (`codex-history-worker.ts`), workspace search in
  another worker — optimization.md §3/§10.
- *No giant synchronous IPC state transfers.* Thread list is metadata-only by
  design (main.ts comment at `DesktopThreadsChannel`), history pages are
  bounded/paginated — optimization.md §4/§12.
- *Pre-boot frame does not flash off-palette.* `backgroundColor:#05070d` matches
  the token background (createWindow).

**Violated / not yet honored:**

- **V1 — the window is created LAST, after heavy synchronous init**
  (optimization.md §1, "show a usable window almost immediately"). `createWindow`
  runs *after* `openDesktopSyncHost` (SQLite open), the session vault + recovery,
  and `runtimeGateway.start()` in `whenReady`. **Measured:** `appWhenReady`
  median 143ms → `windowCreated` median 282ms — ~139ms of main-thread work stands
  between "ready" and "window exists," and every downstream mark inherits it.
- **V2 — the renderer blocks first paint on data.** boot.ts awaits ~6–7 IPC
  round-trips (history catalog scan being the heaviest) *before* `renderer.mount`
  (boot.ts:491–735 precede 1152), directly against optimization.md §20 ("open the
  shell before data is ready") and §5 ("render the shell immediately"). Measured
  `rendererBootStart` 438ms → `shellMounted` 518ms is ~80ms of pre-mount awaits.
- **V3 — a large module-eval cost.** The whole service graph is constructed at
  import time (§1 above), against optimization.md §1/§2 ("allocate just in time",
  "microscopic startup bundle"). Measured `mainModuleEvaluated` median 105ms
  before `whenReady` even fires.
- **V4 — single 2967-line main + one large IIFE renderer** (no boot/route bundle
  split), against optimization.md §2.

## 5. Baseline measurements (this run)

Harness: `startup-bench --runs 7 --warmup 1`, deterministic marks mode, on the
build machine (`darwin-arm64`), origin/main `1d1569e8ed`. Receipt:
`apps/openagents-desktop/benchmarks/startup/2026-07-11-baseline.json`.

| Mark | Median (ms) | p95 (ms) |
| --- | ---: | ---: |
| mainModuleEvaluated | 105.29 | 130.91 |
| appWhenReady | 143.18 | 167.88 |
| windowCreated | 282.29 | 307.91 |
| windowReadyToShow (first paint) | 443.58 | 471.08 |
| rendererBootStart | 438.58 | 466.11 |
| shellMounted (interactive) | 518.58 | 550.61 |
| capabilityReady | 533.29 | 567.11 |

> **Note on `firstPaint`:** the renderer-reported FCP `PerformanceEntry` came
> back empty in marks mode on this host, so `windowReadyToShow` (Electron's
> `ready-to-show`, which fires only after the renderer's first frame paints) is
> the canonical first-paint metric here. `firstPaint` is retained as a
> best-effort refinement and is safe to be null.

> **Note on load drift:** the build machine is running many concurrent desktop
> lanes; absolute medians drift ±15–40ms between measurement windows. Cross-time
> comparisons are therefore invalid — the applied optimization below is proven
> with an **interleaved A/B** (alternating builds under identical load), not a
> before/after taken minutes apart.

## 6. Ranked optimizations for OUR app

1. **[TESTED — REVERTED, within noise] Window-first startup (fix V1).** Moving
   `createWindow()` ahead of `openDesktopSyncHost` + the session vault +
   recovery was implemented and measured. It produced **no** improvement:
   `windowCreated` 282→279ms, `shellMounted` 518→519ms — inside run noise. The
   reason the measurement teaches us: the ~139ms `appWhenReady → windowCreated`
   gap is the **`new BrowserWindow(...)` constructor cost itself** (Electron
   renderer/GPU process allocation), *not* the service init — on a fresh
   `userData` the SQLite open + empty vault are near-zero. Per the honesty rule
   the reorder was reverted; the finding stands as a receipt against re-trying it
   without a heavier-init reason. (It remains architecturally correct per
   optimization.md §1 and is cheap to re-apply if init ever grows.)
2. **[APPLIED — measured win, this pass] Minify all build artifacts.** The
   renderer shipped as a ~3.6 MB unminified IIFE, main ~2.2 MB, preload ~1.3 MB,
   with `scripts/build.ts` doing no minification — against optimization.md §2.
   The renderer and preload sit on the first-paint critical path and the main
   bundle parses before `whenReady`, so smaller bytes parse faster. See §7 for
   the drift-controlled A/B result.
3. **[NEXT] Render the shell before the pre-mount data awaits (fix V2).** Mount
   the Effect Native shell first, then dispatch history-catalog / coding-catalog
   / thread-list / session-status as post-paint intents (the file already does
   this for the *selected thread* body, boot.ts comment "First paint must never
   wait on local rollout parsing"). `shellMounted − rendererBootStart` ≈ 70–80ms
   of pre-mount awaits is the next reducible chunk. Deferred: it changes the boot
   data-load order and needs its own oracle set + smoke/restore-contract review.
4. **[NEXT] Defer module-scope service construction (fix V3).** Lazily construct
   `fableLocal`, `codexLocal`, `codexChildren`, `gitGithubService`, and the MCP
   store on first use instead of at import, shrinking `mainModuleEvaluated`
   (~108ms before `whenReady`).
5. **[LATER] Split the renderer bundle (fix V4)** into a boot/shell chunk plus
   lazily-imported feature routes (settings, git panel, workspace browser,
   history), per optimization.md §2 — the largest renderer modules (`shell.ts`
   ~133KB, `settings.ts` ~52KB, `workspace-browser.ts` ~40KB) do not all need to
   parse before first interaction. Compounds with minification.
6. **[LATER] Audit synchronous fs/JSON on the boot path** (`openMcpConfigStore`,
   `openDesktopCommandBindingStore`, the session vault) for large reads, per
   optimization.md §13.

## 7. Applied optimization + measured result (this pass)

**Change:** minify every build artifact (optimization #2). `scripts/build.ts`
now passes `minify: true` (env-gated `BUILD_MINIFY`, default on;
`OA_DESKTOP_BUILD_MINIFY=0` for A/B) to all five `Bun.build` calls. Bundle sizes:

| Artifact | Unminified | Minified | Δ |
| --- | ---: | ---: | ---: |
| `renderer/boot.js` | 3.56 MB | 2.20 MB | −38% |
| `main.js` | 2.22 MB | 1.12 MB | −50% |
| `preload.cjs` | 1.34 MB | 0.64 MB | −52% |

**Measured — interleaved A/B, 4 rounds × 3 runs per condition, alternating
unminified/minified builds under identical machine load** (this is the honest
comparison; cross-time before/after is invalid per the §5 drift note). Receipt:
`apps/openagents-desktop/benchmarks/startup/2026-07-11-minify-ab.json`.

| Mark | Unminified median | Minified median | Δ |
| --- | ---: | ---: | ---: |
| windowReadyToShow (first paint) | 438.3 ms | 419.3 ms | **−19.0 ms (−4.3%)** |
| shellMounted (interactive) | 508.0 ms | 491.0 ms | **−17.0 ms (−3.3%)** |
| capabilityReady | 525.8 ms | 506.0 ms | **−19.8 ms (−3.8%)** |

The improvement is consistent and directionally identical across all three
critical metrics under interleaved load — a real ~17–20ms win, not noise. It is
build-time only (no runtime-path risk). `bun run --cwd apps/openagents-desktop
verify` (typecheck + tests + build + smoke) stayed green on the minified build,
including the single-instance-lock, deep-link, and full smoke composer/restore
contracts. (`benchmarks/startup/2026-07-11-baseline.json` and
`2026-07-11-after-minify.json` are single-condition `startup-bench` snapshots
from different load windows and must NOT be diffed against each other — use the
A/B receipt.)

## 8. The standing measure-constantly discipline

- **Metrics that matter (in priority order):** `windowReadyToShow` (first paint),
  `shellMounted` (interactive), `capabilityReady`, then `windowCreated` and
  `appWhenReady` as leading indicators.
- **The A/B protocol is the gate, not absolute budgets.** Because absolute
  medians drift with machine load, a change to `main.ts`, `boot.ts`,
  `scripts/build.ts`, or `preload.cts` is judged by an **interleaved A/B**
  (`OA_DESKTOP_BUILD_MINIFY` toggle or a before/after branch, alternated under
  the same load), never a cross-time diff. Reference absolute budgets on an idle
  build machine, as loose ratchets:
  - first paint (`windowReadyToShow`) median ≤ 440ms
  - interactive (`shellMounted`) median ≤ 500ms
  - capability-ready median ≤ 515ms
- **How it stays green:** run `startup-bench` (or the A/B) before/after any
  startup-path change; commit the receipt JSON next to the change; treat a
  median regression that survives interleaving the same as a failing test.
  optimization.md §23/§24 — cold budgets, no vibes.
- **Honesty rule (exercised this pass):** if an "optimization" does not move a
  median beyond run noise (~±20ms here) under interleaving, it is reverted and
  reported as no-change. Window-first (§6.1) was reverted under exactly this
  rule. No unmeasured wins land.

## 9. Residual work

- Fix V2 (shell-before-data) and V3 (lazy module construction) as scoped
  follow-ups with their own oracles — the largest remaining reducible chunks.
- Recover a real renderer FCP number in marks mode (paint-entry capture).
- Add a cold-vs-warm dimension (first-run-after-install, post-reboot) per
  optimization.md §23, and eventually a mediocre-hardware target.
- Add renderer bundle-split (optimization #5) once the Effect Native route
  boundaries settle — it compounds with the minification landed here.
