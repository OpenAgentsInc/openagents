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

## Follow-up run (2026-06-20): supervisor phase now reaches the `apple_fm.status` surface

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: `summarizeAppleFmBridgeSupervisor(...)` produced a public-safe
  supervision summary, but nothing carried it onto the `apple_fm.status`
  projection that Autopilot Desktop already consumes. A crash-looped helper was
  therefore invisible to the surface — the capability probe could read `ready`
  while the supervisor had given up.
- `apps/pylon/src/node/apple-fm-status.ts` — added an optional `supervisor` field
  to `PylonAppleFmStatusProjection` and a pure, additive
  `withAppleFmSupervisorStatus(projection, supervisor)` that embeds the
  supervisor summary and merges its blocker refs into the top-level
  `blockerRefs` (deduped + sorted). The base projection (no supervisor) is
  unchanged, no clock is read, and no prompts/paths/tokens/URLs/bearer material
  are introduced.
- `apps/pylon/tests/apple-fm-status-supervisor.test.ts` — 5 tests: base
  projection has no supervisor; attaching a running supervisor surfaces health
  without adding blockers and does not mutate the base; a crash-looped
  supervisor merges its blocker even when health reads ready; blocker refs are
  deduped + sorted; the merged projection carries no sensitive content.
  `bun test` green (5 pass / 18 assertions).
- Still open: the live launcher that actually feeds
  `process_started`/`process_exited`/`health_ok` events and performs the emitted
  `spawn`/`schedule_restart`/`give_up` actions still does not exist, and the
  Pylon `apple_fm.status` action does not yet attach a real supervisor instance
  (no supervisor is live to attach). This wires the *projection plumbing* so the
  phase reaches the surface the moment a launcher exists.

## Follow-up run (2026-06-20): stateful supervisor driver glues policy to a launcher

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: `reduceAppleFmBridgeSupervisor(...)` is a pure reducer and
  `summarizeAppleFmBridgeSupervisor(...)` is a pure projection, but nothing held
  the supervisor state across events, translated the emitted actions into
  effects, or closed the backoff loop by re-feeding a `tick` when a restart
  timer fired. A live launcher had no deterministic core to build on.
- `apps/pylon/src/node/apple-fm-bridge-supervisor-driver.ts` —
  `createAppleFmBridgeSupervisorDriver({ spawn, markRunning?, giveUp?, now, timer })`
  returns a stateful driver (`requestStart` / `notifyStarted` / `notifyHealthy`
  / `notifyExited` / `phase` / `snapshot` / `status` / `dispose`). It folds each
  signal through the reducer, performs the single emitted action via the
  injected effects, and — on `schedule_restart` — sets a timer that, when it
  fires, re-feeds a `tick` so backoff actually transitions back to `spawn`. ALL
  real I/O is injected: a live launcher passes `Bun.spawn` + `setTimeout`; tests
  pass fakes. The driver never reads the wall clock, spawns a process, opens a
  socket, or emits prompts, paths, tokens, URLs, or bearer material.
- `apps/pylon/tests/apple-fm-bridge-supervisor-driver.test.ts` — 7 tests over a
  fake clock+timer harness: spawn-once / no-double-spawn, mark-running + running
  status, crash → self-firing backoff respawn, crash-loop give-up + blocker ref
  + `stopped` status, stable-uptime escalation reset, `dispose` cancels the
  pending restart, and a no-sensitive-keys status assertion. `bun test` green
  (7 pass / 28 assertions).
- Still open: the real launcher that constructs this driver with `Bun.spawn`,
  spawns/monitors the actual Foundation Models bridge process, feeds its real
  `process_started`/`process_exited`/heartbeat signals, and registers the live
  driver's `status()` with the Pylon `apple_fm.status` action does not exist yet.
  This run supplies the deterministic driver core that launcher will sit on.

## Follow-up run (2026-06-20): live launcher glues the driver to a real child process

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: `createAppleFmBridgeSupervisorDriver(...)` holds supervision
  state and turns actions into injected effects, but its `spawn: () => void` was
  abstract — nothing turned it into an actual child process, fed the process's
  start/exit back into the driver, or exposed a public-safe `status()` for the
  surface. There was no launcher to construct with `Bun.spawn`.
- `apps/pylon/src/node/apple-fm-bridge-launcher.ts` —
  `createAppleFmBridgeLauncher({ helper, spawnProcess, now, timer, ... })`
  returns a supervised launcher (`start` / `notifyHealthy` / `status` / `stop`).
  It builds the driver, and its injected `spawn` calls a process-spawning seam
  (`AppleFmBridgeProcessSpawner`) whose `onStarted`/`onExited` callbacks feed
  `notifyStarted`/`notifyExited` back through the policy — closing the
  spawn→run→crash→backoff→respawn loop against a real process. A duplicate exit
  is de-duped; `give_up` and `stop` kill any live child; `status()` is the
  existing public-safe supervisor summary. ALL process I/O is injected:
  `buildAppleFmBridgeSpawnSpec` is pure (helper path + `--port`), and
  `createBunAppleFmBridgeProcessSpawner()` is the thin `Bun.spawn` edge
  (stdio ignored so no prompt/model content is captured). The launcher reads no
  wall clock and emits no prompts, paths, tokens, URLs, or bearer material.
- `apps/pylon/tests/apple-fm-bridge-launcher.test.ts` — 8 tests over a fake
  clock+timer+spawner harness: spawn-spec shape, start-once/no-double-spawn +
  running status, crash→backoff respawn, duplicate-exit de-dup, crash-loop
  give-up + `stopped` + blocker ref, `stop` cancels pending restart, `stop`
  kills a live child, and a no-sensitive-content (incl. no helper path)
  assertion. `bun test` green (8 pass / 24 assertions).
- Still open: nothing constructs this launcher with the *real*
  `createBunAppleFmBridgeProcessSpawner()` + a `setTimeout`-backed `timer` +
  `Date.now`, wires `discoverAppleFmBridgeHelper()` into it, feeds the live
  bridge's heartbeat into `notifyHealthy()`, and registers the launcher's
  `status()` with the Pylon `apple_fm.status` action (via the already-built
  `withAppleFmSupervisorStatus`). That last integration step is the remaining
  supervision wiring; this run supplies the deterministic launcher it sits on.

## Follow-up run (2026-06-20): host-assembly factory wires the launcher's production defaults

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: `createAppleFmBridgeLauncher(...)` takes a `helper`, a
  `spawnProcess` seam, a `now` clock, and a `timer` — but nothing assembled those
  from the *production* defaults the remaining step calls for. In particular there
  was no `setTimeout`/`clearTimeout`-backed `AppleFmBridgeSupervisorTimer`
  anywhere, and no single seam that paired `discoverAppleFmBridgeHelper()` with
  `createBunAppleFmBridgeProcessSpawner()` + `Date.now` while degrading
  gracefully (returning `null`, not throwing) on a host that ships no helper.
- `apps/pylon/src/node/apple-fm-bridge-launcher-host.ts` —
  `createSetTimeoutSupervisorTimer()` (the production backoff timer) and
  `createDefaultAppleFmBridgeLauncher({ discover?, discoverHelper?, spawnProcess?,
  timer?, now?, config?, port?, extraArgs? })`, which discovers the helper and, if
  present, returns `{ helper, launcher }` assembled with the live defaults — or
  `null` when no helper is found. Every real dependency stays injectable so the
  assembly is deterministic in tests; the module reads no wall clock of its own
  and introduces no prompts, file contents, tokens, URLs, or bearer material.
- `apps/pylon/tests/apple-fm-bridge-launcher-host.test.ts` — 7 tests: null on
  no-helper, assembly + spawn-spec/running status, discover-options threading
  through real env discovery, port + extra-args forwarding, a no-sensitive-content
  (incl. no helper path) assertion, and the `setTimeout` timer fire/clear
  behaviour. `bun test` green (7 pass / 17 assertions).
- Still open: nothing in the Pylon node host (`src/index.ts` /
  `control-sessions.ts`) yet *calls* `createDefaultAppleFmBridgeLauncher()`,
  routes the live bridge heartbeat into `notifyHealthy()`, or attaches the
  launcher's `status()` to the `apple_fm.status` action via the already-built
  `withAppleFmSupervisorStatus`. This run supplies the production assembly seam
  that final host call will use.

## Follow-up run (2026-06-20): `apple_fm.status` action now accepts a live supervisor-status provider

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: every supervision piece existed in isolation
  (`createDefaultAppleFmBridgeLauncher` assembles a launcher whose `status()`
  returns the public-safe phase; `withAppleFmSupervisorStatus` merges that phase
  onto the projection), but the Pylon node host's `apple_fm.status` action
  (`src/index.ts`) still called bare `collectPylonAppleFmStatus(...)` — there was
  no seam at the action where a live launcher's `status()` could be attached, so
  the supervisor phase could never reach the surface even once a launcher exists.
- `apps/pylon/src/node/apple-fm-supervised-status.ts` —
  `createSupervisedAppleFmStatusAction(baseInput, { supervisorStatus?, collect? })`
  returns the `() => Promise<PylonAppleFmStatusProjection>` action the control
  server registers. On each call it collects the base capability projection, then
  — when a supervisor-status provider is supplied and returns a phase — merges it
  via `withAppleFmSupervisorStatus`. With no provider (or one returning
  null/undefined, e.g. a host where `createDefaultAppleFmBridgeLauncher` returned
  `null`), the action is byte-for-byte the previous unsupervised projection. Both
  the collector and the provider are injectable, so the seam is deterministic in
  tests; it reads no wall clock and introduces no prompts, file contents, paths,
  tokens, URLs, or bearer material.
- `apps/pylon/src/index.ts` — the `appleFmStatus` action is now built with
  `createSupervisedAppleFmStatusAction({ summary, env })` (behaviour-preserving:
  no provider is wired yet, so the projection is unchanged), creating the single
  point where a constructed launcher's `status` will plug in.
- `apps/pylon/tests/apple-fm-supervised-status.test.ts` — 6 tests: no-provider
  pass-through, null-provider pass-through, running supervisor attaches phase
  without adding blockers, crash-looped supervisor merges its blocker even when
  the capability probe reads ready, collector receives the base input on each
  call, and a no-sensitive-content assertion. `bun test` green (6 pass /
  15 assertions).
- Still open: the host does not yet *construct* a launcher
  (`createDefaultAppleFmBridgeLauncher()`), call `start()`, or route the live
  bridge heartbeat into `notifyHealthy()` — so no `supervisorStatus` provider is
  passed in production yet. That launcher-lifecycle wiring (which needs a real
  process and admitted-Mac validation) is the remaining step; this run supplies
  the action seam it will plug its `status()` into.

## Follow-up run (2026-06-20): launch lifecycle owner is constructed + (opt-in) wired into the Pylon host

Advances (does **not** clear)
`blocker.product_promises.local_apple_fm_helper_supervision_missing`.

- What was missing: every seam existed in isolation —
  `createDefaultAppleFmBridgeLauncher()` *assembles* a launcher, and
  `createSupervisedAppleFmStatusAction(base, { supervisorStatus })` *consumes* a
  provider — but nothing in the host *constructed* the launcher, called `start()`
  to begin supervision, surfaced its `status()` as the provider, or owned its
  `stop()` on shutdown. The previous index.ts comment said the provider gets
  wired "once a live launcher is constructed on this host"; that owner did not
  exist.
- `apps/pylon/src/node/apple-fm-supervised-launch.ts` —
  `createAppleFmSupervisedLaunch({ assemble?, ...launcherOptions })` is the
  lifecycle owner: it assembles the launcher (injectable; defaults to the
  production factory), and when one is present calls `start()` and returns
  `supervised: true` with a `supervisorStatus` provider (the launcher's
  `status`), a `notifyHealthy` heartbeat pass-through, and an idempotent `stop`.
  When no helper is found (non-Apple host) it returns a fully inert handle
  (`supervised: false`, `supervisorStatus: undefined`, no-op `notifyHealthy`/
  `stop`) so the caller falls back to the unsupervised projection byte-for-byte.
  `notifyHealthy` is also inert after `stop` (no late heartbeat reaches a
  torn-down launcher). The module reads no wall clock and emits no prompts,
  paths, tokens, URLs, or bearer material.
- `apps/pylon/src/index.ts` — the headless node now constructs the launch
  lifecycle owner behind a default-off env gate (`PYLON_APPLE_FM_SUPERVISE=1`):
  when the flag is set AND a helper is discovered, the launcher starts and its
  `status()` is passed as the `supervisorStatus` to the `apple_fm.status` action;
  the owner's `stop()` is called from the existing `requestShutdown` so a backoff
  timer / live child cannot outlive the node. With the flag off (the default) the
  launch is `null`, no provider is passed, and the action is byte-identical to
  before — so this introduces zero behaviour change until explicitly opted in.
- `apps/pylon/tests/apple-fm-supervised-launch.test.ts` — 7 tests over a
  recording fake launcher: inert handle on no-helper, start-once-before-status +
  status pass-through, heartbeat forwarding, idempotent stop, heartbeat-inert
  after stop, launcher-option threading, and a no-sensitive-content assertion.
  `bun test` green (7 pass / 17 assertions; full apple-fm suite 66 pass).

## What genuinely remains (blocker NOT cleared)

- Route a *real* bridge heartbeat into `appleFmSupervisedLaunch.notifyHealthy()`
  (the launch owner exposes the pass-through, but nothing yet feeds the live
  Foundation Models bridge's heartbeat into it) and validate the opt-in
  `PYLON_APPLE_FM_SUPERVISE=1` path end-to-end on admitted Apple hardware
  (construction, start, crash→backoff→respawn, crash-loop give-up surfacing on
  `apple_fm.status`). The construction/start/status-provider/stop lifecycle, the
  action seam, and the projection plumbing now all exist and are wired; only the
  live heartbeat source and the on-hardware smoke remain for this blocker.
- Repeat the admitted-Mac smoke with supervised launch (and the still-open
  `blocker.product_promises.local_apple_fm_signed_installer_recut_missing`:
  signed/notarized installer that bundles or supervises the helper, plus a
  from-install smoke).

Both blockers remain listed in the registry `blockerRefs`; nothing here flips
any promise state.
