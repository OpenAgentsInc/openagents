# Desktop startup incident: ~5 s blank frame before the UI (2026-07-13)

Status: **FIXED, measured, contract-enforced.** Real-profile time to mounted
interactable shell went from **5.4–7.0 s** to **~0.44–0.72 s**; the window now
paints a branded boot frame instead of a blank frame; the history scan streams
in afterwards behind an honest "Scanning coding history…" state.

## Owner statement (verbatim)

> Opening the openagents app, via our new oa command or in dev, shows a
> blank/brown screen for ~5 seconds before opening the UI. This is
> unacceptable. I thought we had a UX contract somewhere about the need to
> show initial codex chats in <50 ms. That should be timed from startup. Go
> look up our ways of testing load times. Startup and everything else. Write
> full analysis of current situation in openagents/docs/fable/ new doc. This
> is an incident. Very bad. Need good bootup process. No brown screen. If any
> loading, show beautiful starcraft version of it, or something. Time to
> seeing stuff and then interactable elements on bootup is extremely
> important. Analyze, fix, update analysis, push.

## Impact and root cause (two sentences)

Every launch of the installed or dev desktop app on a machine with real Codex
history showed ~5–7 seconds of featureless dark frame because the renderer
**awaited the full `~/.codex` + `~/.claude` merged history-graph scan (measured
5.3 s against this machine's 22 GB, 1,543-rollout `~/.codex`) before mounting
the shell**, and the resulting pre-mount window displayed only the bare
`BrowserWindow` background. A second, latent blocker sat in Electron main: the
`whenReady` path **awaited a network session verification
(`recoverVerifiedDesktopSession` → `https://openagents.com/api/mobile/auth/session`,
no timeout) and a synchronous SQLite open before `createWindow()`** — it did
not fire on this profile's measurements (the vault state was not
`credential_present_unverified`), but on any profile with a stale session it
delays *any window at all* by an unbounded network round trip.

## Contract inventory — what we had actually promised

The owner's memory of "initial codex chats in <50 ms" maps to:

1. **`openagents_desktop.chat.thread_first_content_under_50ms.v1`**
   (`apps/openagents-desktop/GUARANTEES.md`): a **post-selection** bounded
   projection budget — after a thread is selected, the local first-content
   projection completes in <50 ms regardless of rollout size. It is *not* a
   time-from-startup promise and explicitly "does not claim that every machine
   will paint a complete window within 50 ms."
2. **Episode 248–249 product calibration** ("metadata-first startup / no blank
   startup", `docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md`):
   the directional product rule that startup shows metadata immediately and
   never a blank window — previously calibration prose, not an enforced
   contract.
3. **EP250 #8712 pre-boot background note** (`src/main.ts` `createWindow`):
   the BrowserWindow `backgroundColor` is pinned to the khala token background
   `#05070d` so the pre-boot frame is never off-palette — which is exactly the
   featureless dark frame the owner saw for ~5 s. On-palette, but blank.

So: **there was no enforced startup-time contract.** That gap is closed below
(`openagents_desktop.startup.window_first_no_blank_frame.v1`).

## Load-time measurement tooling — what exists

- `apps/openagents-desktop/scripts/startup-bench.ts` — runs the real Electron
  app N times in deterministic **fixture** mode
  (`OPENAGENTS_DESKTOP_STARTUP_MARKS`, temp userData, no real `~/.codex`),
  reports median/p95 milestone chains, writes timings-only receipts to
  `benchmarks/startup/`. Prior receipts
  (`2026-07-11-{baseline,after-minify,after-window-first,minify-ab}.json`)
  showed ~520–535 ms fixture medians — **the fixture bench structurally could
  not see this incident** because it never scans a real history root.
- **NEW: `OPENAGENTS_DESKTOP_STARTUP_TRACE`** (this incident) — the same
  milestone driver **without fixture substitution**: real userData, real
  session custody, real `~/.codex`, plus new per-step main-process marks
  (`sessionHardened`, `syncHostOpened`, `sessionVaultRecovered`,
  `sessionRecoverySettled`) and a renderer `historyHydrated` mark. Timings
  only; never deletes the measured profile; optional pixel receipts via
  `OPENAGENTS_DESKTOP_STARTUP_TRACE_SHOTS`.
- Electron smoke (`OPENAGENTS_DESKTOP_SMOKE=1`, screenshots via
  `OPENAGENTS_DESKTOP_SMOKE_SHOTS`), `scripts/release-preflight.ts`, and the
  root `perf:ui-velocity` / `test:ui-velocity-receipt` scripts (post-mount
  interaction latency, not startup).

## Measured reality (this machine, real profile copy, 22 GB `~/.codex`)

BEFORE (ms from process start; `OPENAGENTS_DESKTOP_STARTUP_TRACE` on a copy of
the real production profile):

| mark | run 1 (cold) | run 2 (warm) |
|---|---|---|
| appWhenReady | 233 | 172 |
| windowCreated | 382 | 315 |
| windowReadyToShow (blank frame visible) | 557 | 475 |
| firstPaint | **never recorded** | **never recorded** |
| shellMounted (first real UI) | **7017** | **5391** |
| capabilityReady | 7025 | 5404 |

The user saw the dark blank frame from ~0.5 s to ~5.4–7.0 s. `firstPaint`
never fired pre-mount because the document painted nothing (body background
resolves through `--en-*` variables that only exist after the renderer
stylesheet mounts).

Attribution micro-measurement: `buildMergedHistoryGraphs(~/.codex/sessions,
~/.claude)` — the scan behind the `codex.history.catalog` query the renderer
awaited before mount — took **5,291 ms warm** in isolation. That is the
incident, end to end. The renderer additionally awaited the coding catalog,
update projection, thread list, session view, and command handshake before
mounting; all were riders on the same pre-mount block.

## The fix

Three ordered principles, all landed in this change:

1. **Window first (main process).** `app.whenReady` now creates the window
   before any local database open, OS-keychain custody, or network work.
   `openDesktopSyncHost` (sync SQLite), the vault recover, and
   `runtimeGateway.start()` run immediately *after* `createWindow()`; the
   network session verification (`settleSessionRecovery`) is **fire-and-forget**
   — the renderer sees the honest typed `unverified` phase and the CUT-10
   converging chat facade re-admits operations once verified Sync connects.
   The windowless local-turn-restart probe keeps the fully settled ordering.
2. **Branded boot frame (first HTML parse).** `index.html` now carries a
   static boot frame — khala background, "OPENAGENTS" wordmark, a thin psi-bar
   shimmer in accent blue — that paints before the renderer bundle even
   evaluates. Every color literal in it is an exact `@effect-native/tokens`
   khalaTheme value, mechanically enforced (same rule as the BrowserWindow
   `backgroundColor`). Reduced-motion honored; removed the moment the shell
   mounts. No light variant.
3. **Mount before hydration (renderer).** `boot.ts` mounts the interactable
   shell (composer focusable, sidebar present) and only then runs
   `hydrateAfterMount`: history catalog + page restore, coding catalog, update
   projection, thread list, session view, deferred-command handshake, and the
   first-thread bounded open. While the scan runs the sidebar shows
   `Scanning coding history…` (new `history.hydrated` state) — the
   `No local Codex history found.` claim renders only after the scan settles,
   so the loading state never lies.

   Because the shell is now live during hydration, two interaction guards
   landed with it (both surfaced by the Electron smoke's reload pass):
   the persisted-focus workspace restore is skipped if the user navigated
   after mount (restore must never stomp an explicit selection), and a Files
   selection made before the coding catalog hydrated re-runs the same typed
   `DesktopWorkspaceSelected` path after hydration so workspace-editor
   recovery is never silently skipped.

## AFTER numbers

Real profile copy (same machine, same `~/.codex`):

| mark | run 2 | run 3 | run 4 |
|---|---|---|---|
| windowCreated | 291 | 291 | 500 |
| firstPaint (branded frame) | **390** | **390** | 677 |
| windowReadyToShow | 444 | 378 | 641 |
| shellMounted (interactable) | **490** | **443** | 724 |
| capabilityReady | 498 | 449 | 732 |
| historyHydrated (full sidebar history) | 5037 | 4601 | 5569 |

Time to seeing stuff: **~0.4 s**. Time to interactable shell: **~0.44–0.72 s**
(from 5.4–7.0 s — a 10×+ improvement). The 4.6–5.6 s history scan still runs
(unchanged cost) but now streams into a live, usable app behind an explicit
scanning row.

Fixture bench (`scripts/startup-bench.ts --runs 5`): on a quiet machine the
medians were windowCreated 350 → syncHostOpened 358 (ordering now provably
window-first), firstPaint 471, shellMounted 558, historyHydrated 672,
capabilityReady 575 — comparable to the 2026-07-11 `after-window-first`
receipt (firstPaint 535 / shellMounted 520), no fixture-mode regression, and
firstPaint now lands *before* shell mount because the boot frame paints. The
committed receipt (`benchmarks/startup/2026-07-13-window-first-boot-frame.json`)
was captured while concurrent agents held the machine at load average ~24
(everything ~3× slower: readyToShow 1459 / shellMounted 1593 medians) and the
budgets still held — honest evidence of budget headroom, not of typical
latency.

## Enforcement (new contract)

`openagents_desktop.startup.window_first_no_blank_frame.v1` — owner statement
recorded verbatim in `apps/openagents-desktop/src/contracts/ux-contracts.ts`
(registry v2026-07-13.6) and summarized in `GUARANTEES.md`. Oracles, all in
the normal test sweep (`apps/openagents-desktop/tests/startup-contract.test.ts`,
with falsifier fixtures proving each validator rejects the pre-incident
shape):

- `startup.window_first_ordering` — the production `whenReady` path creates
  the window before SQLite/keychain/network; the network settle is never
  awaited post-window; the network call stays confined to the settle helper.
- `startup.shell_mounts_before_hydration` — the history catalog fetch lives
  inside `hydrateAfterMount`, which runs after `renderer.mount`; the boot
  frame is removed after mount.
- `startup.boot_frame_token_sync` — the boot frame exists and every color in
  it is an exact khalaTheme value (no off-palette frame can ever paint).
- `startup.sidebar_scanning_honesty` — typed view-tree proof of the scanning
  row pre-hydration and the empty-history claim only post-hydration.
- `startup.bench_budgets` — `startup-bench.ts` now fails (exit 1) if fixture
  medians exceed windowReadyToShow 1500 ms / shellMounted 2500 ms; generous
  machine headroom, but an ordering regression blows these by seconds.

## Receipts

- Timings: `apps/openagents-desktop/benchmarks/startup/2026-07-13-window-first-boot-frame.json`
  (fixture, budget-asserted). Real-profile traces reproduced in the tables
  above (captured via `OPENAGENTS_DESKTOP_STARTUP_TRACE` against a copy of the
  production profile; raw files carry timings only).
- Pixels: `docs/receipts/2026-07-13-desktop-startup-incident/boot-frame.png`
  (the branded frame at ready-to-show, real profile) and
  `docs/receipts/2026-07-13-desktop-startup-incident/shell-mounted-smoke.png`
  (the mounted shell, deterministic smoke fixture).
- Full smoke: `OPENAGENTS_DESKTOP_SMOKE=1` green end-to-end after the
  restructure (composer flows, history restore, git panel, catalog
  persistence, teardown `{"ok":true,"active":0}`).

## Honest limits and follow-up

- The merged history scan itself is untouched: ~5 s of background utility-
  process work per cold launch on a 22 GB `~/.codex`. It is now off the
  critical path, but the right follow-up is an incremental/persisted catalog
  index (bound the scan, not just its placement). Tracked as follow-up, not
  claimed fixed.
- The background session verification remains unbounded (no fetch timeout);
  it can no longer block the window, but a hung request would leave the
  session phase `unverified` until process restart. Acceptable now; a bounded
  retry would be strictly better.
- Real-profile traces were captured against a **copy** of the production
  profile on this machine (dev Electron build), not the installed `oa`
  bundle; the installed app runs the same `dist/` code path. The ~5 s figure
  matched the owner's report before the fix and vanished after — but the
  installed-app relaunch itself was not re-run by this agent.
- In the measured profile the session vault state did not trigger the network
  await, so its pre-fix cost on this machine is inferred from code, not
  measured. The ordering fix removes the risk class either way.
- Unrelated environmental smoke flake observed while verifying: the
  `details-affordance` step fails when the machine's physical mouse cursor
  happens to rest over the visible smoke window's transcript (the CSS
  `:hover` reveal is real). Reproduced on the same build back-to-back with
  the cursor over/away from the window; not introduced by this change.
- The full desktop suite passed clean twice (1216 pass / 0 fail); a third run
  during a concurrent-agent load spike (load average ~24) timed out four
  git-subprocess tests (`git-github-host`, `git-review-corpus` — files this
  change does not touch) at their 5 s budgets. Load-induced, not a
  regression.
