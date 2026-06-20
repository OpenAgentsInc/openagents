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

## What genuinely remains (blocker NOT cleared)

- Wire a real launcher (`Bun.spawn`/`child_process`) on top of this policy that
  feeds `process_started`/`process_exited`/`health_ok` events from the live
  bridge and performs the emitted `spawn`/`schedule_restart`/`give_up` actions,
  surfacing the supervisor phase through `apple_fm.status`.
- Repeat the admitted-Mac smoke with supervised launch (and the still-open
  `blocker.product_promises.local_apple_fm_signed_installer_recut_missing`:
  signed/notarized installer that bundles or supervises the helper, plus a
  from-install smoke).
