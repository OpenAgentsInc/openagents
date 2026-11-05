# Aethyrforge desktop frame pacing, 2026-11-04

Measured the installed native AppKit/WKWebView app on the owner's built-in
120 Hz Retina display, with a 1280 × 800 point window and the saved Everglade
world (version 24, including the individually enlarged middle crate). The
baseline source was `e6086f93c5`. Low Power Mode was off. The room's observed
95th-percentile tick cost was about 0.2 ms; its queues were empty. No compiler
or build-load changes are part of this work.

## Findings and changes

- WebKit's `PreferPageRenderingUpdatesNear60FPSEnabled` feature was enabled.
  The original native run stayed at 60 animation callbacks per second despite
  CPU work below 4 ms. The shell now disables that preference on displays above
  60 Hz, checking that the feature and selectors exist first. An unavailable
  feature retains 60 Hz. This uses WebKit's private feature API in this
  experimental ad-hoc-signed app; it is not a public App Store API. Sources:
  [WKPreferencesPrivate.h](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/API/Cocoa/WKPreferencesPrivate.h),
  [WebKit's animation frame-rate explainer](https://webkit.github.io/explainers/animation-frame-rate/).
- Coax's AUTO governor assumed a 60 FPS target and could accept a 30 FPS
  quality budget. Native page facts now supply the display rate (60–240 Hz)
  and a 60 FPS fallback budget. Web/phone facts with no override retain their
  existing defaults. This is a quality target, not a guarantee of throughput.
- The desktop uses Everglade's legacy GPU host, not `RenderHost`. With high
  refresh unlocked and no queue bound, GPU completion round trips reached
  hundreds of milliseconds while CPU work stayed below 2 ms under load.
  `aethyr/frame-pacing.coax` now admits at most two unfinished frames. The
  browser reports queue fences and skips work before simulation, resizing,
  writes or encoding while both are outstanding. Elapsed simulation time is
  retained for the next admitted frame. The count belongs to the GPUDevice;
  old-device completions cannot decrement a replacement device's count.
  Shared-renderer validation and resource-ledger rules are unchanged.
- The floating HUD allocated/zeroed an 8 MB Swift array on each draw. Its
  Coax-generated vertex storage is retained, and the Metal upload reads it
  without requesting a mutable Swift array copy.

## Native measurements

`--frame-profile PATH` runs the normal visible app with its real animation
callbacks. After scene readiness and a three-second warmup, it records six
five-second windows. There is no hidden-window frame pump in these runs.
`frame-profile.coax` computes statistics from raw browser/platform observations.

| Observation | Original | Fixed, stationary | Fixed, W/S movement |
| --- | ---: | ---: | ---: |
| Animation callback rate | 60/s | 120/s | 120/s |
| GPU submissions | not instrumented | 97.7/s | 116.9/s |
| Submitted interval p50 / p95 / max | not instrumented | 8 / 17 / 27 ms | 8 / 10 / 19 ms |
| CPU frame work p95 / max | 3.34 / 3.90 ms | 1.84 / 2.66 ms | 0.94 / 1.68 ms |
| GPU completion round trip p95 / max | not instrumented | 20.16 / 33.04 ms | 15.94 / 19.02 ms |
| Maximum unfinished frames | not instrumented | 2 | 2 |
| Visible windows / samples | 6 / 1872 | 6 / 3600 | 6 / 3600 |
| Last canvas size | 2560 × 1600 | 1920 × 1200 | 2560 × 1600 |

The movement run alternated actual W/S key events every 2.5 seconds for 30
seconds. AUTO adjusted its rung during both fixed runs. The measurements are
not a controlled GPU benchmark at identical quality, and submissions are not
display presentation timestamps. They show the 60 Hz preference is removed,
the queue is bounded and these runs exceed the 60 FPS frame-interval target.
They do not establish every scene or concurrent GPU workload sustains 120 FPS.

An intermediate unbounded high-refresh run had a 111 ms median, 262 ms p95
and 523 ms maximum queue-completion round trip. A hidden input smoke was
running during part of that capture, so it is load evidence rather than a
controlled comparison against the stationary run. One earlier file named
`wk-bounded.json` contained an old Wasm without the export; it was discarded.

Raw observations and check logs are retained locally under
`/tmp/forge-frame-pacing/`: `wk-baseline.json`, `wk-after.json`,
`wk-bounded-final.json`, `wk-walking.json` and `checks/`. Ordinary launches
do not collect traces. Round trips include browser scheduling and queue
latency, not solely GPU execution. The legacy host supplies the completion
measurements; zero fields on other rendering paths mean unobserved work.

## Verification

- `coax test aethyr/quality.coax`: 8/8, including 120 Hz target, 60 FPS floor,
  recovery, invalid rates, and unchanged web/phone defaults.
- Existing Everglade graphics-quality checks: 4/4. New native page-facts
  integration check: 1/1, including removal of overrides on a later facts event.
- Desktop high-refresh selection: 1/1. Profile fractional-time conversion:
  1/1. Frame admission authority: all 5 mutants killed, zero survivors.
- Headless muted Chrome held queue completions: exactly two submissions,
  no third submission through a second hold period. Releasing the fence
  resumed frames. Forced device loss recovered; releasing old-device
  completions left the fresh queue count valid and frames resumed (79 total).
- Native AppKit input smoke passed Return/keypad Enter, Shift-Enter, retained
  drafts, transparent Aethyr message vertices (3912), actual offline Coder CLI
  reply, no region-chat Enter leakage and a 5.023-second reply lifetime.
  Its Echo fixture omits the scene tool manual: otherwise Echo slowly repeats
  that unrelated manual and exceeds the input test's deadline.
- Wasm build: 96 seconds, 2,025,830 bytes, within the 2,040,000-byte budget.
  Optimized native build and ad-hoc signing passed. Existing optional-value
  interpolation warnings remain in older scene-smoke diagnostics.
- `coax/web/tool.sh tools-check` ran: 29 existing files outside this change
  lack reason headers. No new non-Coax tools were added.

Reproduce using the desktop README's frame-profile command. Keep native
measurement runs separate from browser/GPU smoke checks.
