/**
 * Live launcher that runs the local Apple FM Foundation Models bridge helper
 * under the bounded supervision policy.
 *
 * The pieces this sits on already exist:
 *   - `discoverAppleFmBridgeHelper`  → *where* the helper binary lives.
 *   - `reduceAppleFmBridgeSupervisor`→ *when* to (re)start / give up (pure).
 *   - `createAppleFmBridgeSupervisorDriver` → holds that state across events and
 *     turns the emitted actions into injected effects + a self-firing backoff.
 *
 * What was still missing is the glue that turns the driver's abstract
 * `spawn: () => void` into an actual child process whose start/exit signals are
 * fed back into the driver, and exposes a public-safe `status()` for the
 * `apple_fm.status` surface. This module supplies exactly that.
 *
 * ALL real process I/O is injected via `AppleFmBridgeProcessSpawner`. A live
 * caller passes the `Bun.spawn`-backed default (`createBunAppleFmBridgeProcessSpawner`);
 * tests pass a fake spawner so the lifecycle stays deterministic with no real
 * processes. The launcher itself never reads the wall clock, never spawns a
 * process directly, and never emits prompts, file contents, tokens, URLs, or
 * bearer material.
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import {
  APPLE_FM_BRIDGE_DEFAULT_PORT,
  type DiscoveredAppleFmBridgeHelper,
} from "./apple-fm-bridge-helper.js"
import type { AppleFmBridgeSupervisorConfig } from "./apple-fm-bridge-supervisor.js"
import {
  createAppleFmBridgeSupervisorDriver,
  type AppleFmBridgeSupervisorTimer,
} from "./apple-fm-bridge-supervisor-driver.js"
import type { PylonAppleFmSupervisorStatus } from "./apple-fm-bridge-supervisor-status.js"

/**
 * Opaque handle to a running helper process. The launcher only ever asks it to
 * stop; it never reads stdio or process internals.
 */
export type AppleFmBridgeProcessHandle = {
  /** Request termination of the child process. Idempotent. */
  kill(): void
}

/**
 * Lifecycle callbacks the spawner invokes for the process it created. They are
 * the ONLY way the child's state re-enters the supervisor.
 */
export type AppleFmBridgeProcessCallbacks = {
  /** The process is up and accepting work. */
  readonly onStarted: () => void
  /** The process exited (crash or clean stop). Fired at most once. */
  readonly onExited: (exitCode: number | null, signal: string | null) => void
}

/** Fully-resolved command the launcher wants run, with no environment secrets. */
export type AppleFmBridgeSpawnSpec = {
  readonly command: string
  readonly args: ReadonlyArray<string>
}

/**
 * Injected process-spawning seam. A live caller backs this with `Bun.spawn`;
 * tests back it with a recording fake. The spawner is responsible for invoking
 * `callbacks.onStarted` once the child is up and `callbacks.onExited` once it
 * leaves, and for returning a handle that can stop the child.
 */
export type AppleFmBridgeProcessSpawner = (
  spec: AppleFmBridgeSpawnSpec,
  callbacks: AppleFmBridgeProcessCallbacks,
) => AppleFmBridgeProcessHandle

export type AppleFmBridgeLauncherOptions = {
  /** Resolved helper binary (from discoverAppleFmBridgeHelper). */
  readonly helper: DiscoveredAppleFmBridgeHelper
  /** Process-spawning seam (Bun.spawn-backed in production, fake in tests). */
  readonly spawnProcess: AppleFmBridgeProcessSpawner
  /** Injected clock so the launcher stays reproducible (no wall-clock reads). */
  readonly now: () => number
  /** Schedule/cancel the backoff restart timer (setTimeout-backed live). */
  readonly timer: AppleFmBridgeSupervisorTimer
  /** Optional supervision policy overrides. */
  readonly config?: Partial<AppleFmBridgeSupervisorConfig>
  /** Local loopback port the helper should bind. */
  readonly port?: number
  /** Extra args appended after the port flag, if a deployment needs them. */
  readonly extraArgs?: ReadonlyArray<string>
}

/**
 * A running, supervised helper. The caller drives it with `start`/`stop` and
 * forwards heartbeat signals via `notifyHealthy`; everything else (respawn,
 * backoff, give-up) is handled internally by the supervision policy.
 */
export type AppleFmBridgeLauncher = {
  /** Begin supervising the helper (idempotent while already up). */
  start(): void
  /** Forward a healthy heartbeat from the helper into the policy. */
  notifyHealthy(): void
  /** Public-safe supervision status for the apple_fm.status surface. */
  status(): PylonAppleFmSupervisorStatus
  /** Stop supervision: cancel pending restart and kill any live process. */
  stop(): void
}

/** Build the helper invocation. Kept pure + path-only (no env, no secrets). */
export function buildAppleFmBridgeSpawnSpec(
  helper: DiscoveredAppleFmBridgeHelper,
  port: number = APPLE_FM_BRIDGE_DEFAULT_PORT,
  extraArgs: ReadonlyArray<string> = [],
): AppleFmBridgeSpawnSpec {
  return {
    command: helper.path,
    args: ["--port", String(port), ...extraArgs],
  }
}

export function createAppleFmBridgeLauncher(
  options: AppleFmBridgeLauncherOptions,
): AppleFmBridgeLauncher {
  const spec = buildAppleFmBridgeSpawnSpec(
    options.helper,
    options.port ?? APPLE_FM_BRIDGE_DEFAULT_PORT,
    options.extraArgs ?? [],
  )

  // The single live child handle, if any. Cleared on exit / stop.
  let processHandle: AppleFmBridgeProcessHandle | null = null
  // Guards against feeding two exits from one process into the policy.
  let exitDelivered = false

  const driver = createAppleFmBridgeSupervisorDriver({
    config: options.config,
    now: options.now,
    timer: options.timer,
    spawn: () => {
      // A fresh spawn supersedes any previous handle bookkeeping.
      exitDelivered = false
      processHandle = options.spawnProcess(spec, {
        onStarted: () => {
          driver.notifyStarted()
        },
        onExited: (exitCode, signal) => {
          if (exitDelivered) return
          exitDelivered = true
          processHandle = null
          driver.notifyExited(exitCode, signal)
        },
      })
    },
    giveUp: () => {
      // Policy has stopped trying; ensure no orphaned child lingers.
      killCurrent()
    },
  })

  function killCurrent(): void {
    if (processHandle !== null) {
      const handle = processHandle
      processHandle = null
      handle.kill()
    }
  }

  return {
    start() {
      driver.requestStart()
    },
    notifyHealthy() {
      driver.notifyHealthy()
    },
    status() {
      return driver.status()
    },
    stop() {
      driver.dispose()
      killCurrent()
    },
  }
}

/**
 * Production `Bun.spawn`-backed spawner. This is the thin I/O edge: it launches
 * the helper, reports started immediately, and wires `subprocess.exited` to the
 * exit callback. stdio is ignored so no prompt or model content is captured.
 */
export function createBunAppleFmBridgeProcessSpawner(): AppleFmBridgeProcessSpawner {
  return (spec, callbacks) => {
    const subprocess = Bun.spawn([spec.command, ...spec.args], {
      stdout: "ignore",
      stderr: "ignore",
      stdin: "ignore",
    })
    callbacks.onStarted()
    void subprocess.exited.then((exitCode) => {
      callbacks.onExited(exitCode, subprocess.signalCode ?? null)
    })
    return {
      kill() {
        subprocess.kill()
      },
    }
  }
}
