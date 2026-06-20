# autopilot.local_apple_fm_tool_chat.v1 — helper supervision policy

Date: 2026-06-20

Promise: `autopilot.local_apple_fm_tool_chat.v1` (state: **yellow**, unchanged)

## Blocker advanced

`blocker.product_promises.local_apple_fm_helper_supervision_missing`

This run advances — but does **not** clear — the helper-supervision blocker. It
remains listed in the registry `blockerRefs`.

## What was missing

`apps/pylon/src/node/apple-fm-bridge-helper.ts` only *discovers* where the
Foundation Models bridge binary lives (`discoverAppleFmBridgeHelper`). Nothing
decided *when* to launch it, *when* to restart it after a crash, or *when to
stop trying* during a crash loop. A signed-installer recut that "bundles or
supervises the helper" (per the promise verification text) needs that decision
layer, and it needs to be deterministic enough to test without a real Mac.

## What was built

- `apps/pylon/src/node/apple-fm-bridge-supervisor.ts` — a pure, side-effect-free
  reducer (`reduceAppleFmBridgeSupervisor`) implementing a bounded
  restart/backoff supervision policy:
  - phases: `idle → starting → running → backoff → given_up`;
  - exponential backoff (`baseBackoffMs · backoffFactor^(attempt-1)`) clamped to
    `maxBackoffMs`;
  - crash-loop give-up after `maxRestartsInWindow` restarts inside
    `restartWindowMs`, emitting the stable blocker ref
    `blocker.pylon.apple_fm.bridge_supervisor.crash_loop`;
  - stale restart attempts outside the window are pruned;
  - a helper that stays healthy past `stableUptimeResetMs` forgives old crashes
    (resets the counter and backoff escalation);
  - no double-spawn while `starting`/`running`; no spawn after `given_up`.
  - The caller injects `nowMs` on every event — the policy never reads the clock,
    spawns processes, opens sockets, or emits prompts, paths, or secrets.
- `apps/pylon/tests/apple-fm-bridge-supervisor.test.ts` — 8 focused tests
  covering spawn-on-start, no double-spawn, backoff scheduling/growth/cap, tick
  gating, crash-loop give-up + refusal-after-give-up, stable-uptime reset, and
  window pruning. `bun test` green (8 pass / 32 assertions).

## Validation

- `apps/pylon`: `bunx tsc -p tsconfig.json --noEmit` → 0 errors.
- `apps/openagents.com/workers/api`: `bunx tsc -p tsconfig.json --noEmit` → 0 errors.
- `apps/pylon`: `bun test tests/apple-fm-bridge-supervisor.test.ts` → 8 pass.

## Follow-up run (2026-06-20): supervisor phase is now observable

- `apps/pylon/src/node/apple-fm-bridge-supervisor-status.ts` —
  `summarizeAppleFmBridgeSupervisor(state, nowMs)` projects the internal reducer
  state into a stable, public-safe summary
  (`openagents.pylon.apple_fm.supervisor.v0.1`): coarse `health`
  (`idle`/`starting`/`running`/`recovering`/`stopped`), `supervised`,
  `consecutiveRestarts`, window-pruned `restartsInWindow`, clamped
  `backoffRemainingMs`, the crash-loop `blockerRefs`, and `contentRedacted`.
  `nowMs` is injected (no clock read); the summary carries no prompts, file
  contents, paths, tokens, URLs, or bearer material.
- `apps/pylon/tests/apple-fm-bridge-supervisor-status.test.ts` — 7 tests
  (idle/running/recovering health, non-negative backoff clamp, window pruning,
  crash-loop → stopped + blocker ref, and a no-sensitive-keys assertion).
  `bun test` green (7 pass / 27 assertions).
- This makes supervision *observable* by `apple_fm.status` / Autopilot Desktop,
  but the live launcher that actually feeds events and the `apple_fm.status`
  wiring still do not exist — the blocker stays open.

## Follow-up run (2026-06-20): signed-installer recut now has a helper-bundling gate

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_signed_installer_recut_missing`.

- What was missing: `notarize-macos.sh` would deep code-sign and notarize an
  Autopilot Desktop `.app` even if it shipped **no** Apple FM bridge helper, and
  electrobun's `build.copy` had no entry coupling the helper to the path Pylon's
  `discoverAppleFmBridgeHelper` packaged-resource lookup expects
  (`<Resources>/app/apple-fm-bridge/foundation-bridge`). A signed recut could
  therefore look green while being unable to start any local Apple FM session.
- `apps/autopilot-desktop/src/shared/apple-fm-packaging.ts` — pure packaging
  contract: the helper basename, the Resources sub-path, the electrobun
  `build.copy` dest that lands it there, macOS-arm64-only built-`.app` candidates,
  and `verifyPackagedAppleFmBridge({ probe })` — a side-effect-free verifier that
  checks the helper exists, is non-empty, and is executable *inside* the bundle
  root (so deep code-sign + notarization cover it). Returns structural facts
  only — no prompts, file contents, secrets, or paths beyond the bundle dir.
- `apps/autopilot-desktop/scripts/verify-packaged-apple-fm-bridge.ts` — a real
  filesystem-backed runner over the pure verifier (exit 0/1 with secret-free
  diagnostics), intended to run after `electrobun build` and before notarization.
- `apps/autopilot-desktop/scripts/notarize-macos.sh` — now runs that gate before
  `codesign` (opt out with `OA_SKIP_APPLE_FM_BRIDGE_CHECK=1` for intentionally
  Apple-FM-less builds).
- `apps/autopilot-desktop/tests/apple-fm-packaging.test.ts` — 8 tests
  (copy-dest↔discovery-path coupling, bundle path derivation, accept/first-match,
  missing/empty/non-executable rejection, and a no-secret-leak assertion).
  `bun test` green (8 pass / 21 assertions).
- Still open: the helper binary is built by the Swift `foundation-bridge` package
  only on a macOS build host, so the electrobun `build.copy` entry
  (`"<built helper>": APPLE_FM_BRIDGE_ELECTROBUN_COPY_DEST`) and a Swift release
  build step in the desktop build pipeline are NOT added here (adding a copy of a
  non-existent source would break non-Mac `bun run build`). Real signing,
  notarization, and a from-install smoke on admitted hardware remain.

## What genuinely remains (blocker NOT cleared)

- Wire a real launcher (`Bun.spawn`/`child_process`) on top of this policy that
  feeds `process_started`/`process_exited`/`health_ok` events from the live
  bridge and performs the emitted `spawn`/`schedule_restart`/`give_up` actions,
  then embed `summarizeAppleFmBridgeSupervisor(...)` output into the
  `apple_fm.status` projection so the supervisor phase reaches the surface.
- Repeat the admitted-Mac smoke with supervised launch (and the still-open
  `blocker.product_promises.local_apple_fm_signed_installer_recut_missing`:
  signed/notarized installer that bundles or supervises the helper, plus a
  from-install smoke).
