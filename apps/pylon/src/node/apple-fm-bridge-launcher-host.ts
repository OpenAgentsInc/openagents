/**
 * Host assembly for the supervised local Apple FM bridge launcher.
 *
 * The deterministic core already exists:
 *   - `discoverAppleFmBridgeHelper`        → *where* the helper binary lives.
 *   - `reduceAppleFmBridgeSupervisor`      → *when* to (re)start / give up (pure).
 *   - `createAppleFmBridgeSupervisorDriver`→ holds that state + self-fires backoff.
 *   - `createAppleFmBridgeLauncher`        → glues the driver to a real child via
 *                                            an injected process spawner + timer.
 *
 * What was still missing is the *host wiring*: nothing constructed the launcher
 * with the production defaults the named remaining step calls for — a
 * `setTimeout`-backed `AppleFmBridgeSupervisorTimer`, `Date.now`, the
 * `Bun.spawn`-backed process spawner, and the result of
 * `discoverAppleFmBridgeHelper()` — while still returning a clear `null` when no
 * helper is present (so a non-Apple host degrades gracefully rather than
 * throwing). This module supplies exactly that assembly seam.
 *
 * Every real dependency (helper discovery, the process spawner, the timer, and
 * the clock) is still injectable so the assembly stays deterministic in tests;
 * the defaults are the live implementations. The module reads no wall clock of
 * its own and introduces no prompts, file contents, tokens, URLs, or bearer
 * material.
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import {
  discoverAppleFmBridgeHelper,
  type DiscoverAppleFmBridgeHelperOptions,
  type DiscoveredAppleFmBridgeHelper,
} from "./apple-fm-bridge-helper.js"
import {
  createAppleFmBridgeLauncher,
  createBunAppleFmBridgeProcessSpawner,
  type AppleFmBridgeLauncher,
  type AppleFmBridgeProcessSpawner,
} from "./apple-fm-bridge-launcher.js"
import type { AppleFmBridgeSupervisorConfig } from "./apple-fm-bridge-supervisor.js"
import type { AppleFmBridgeSupervisorTimer } from "./apple-fm-bridge-supervisor-driver.js"

/**
 * Production timer backed by `setTimeout` / `clearTimeout`. The driver only ever
 * stores the opaque handle it returns and hands it straight back to `clear`, so
 * the narrow cast below is sound: every handle that reaches `clear` originated
 * from this `set`.
 */
export function createSetTimeoutSupervisorTimer(): AppleFmBridgeSupervisorTimer {
  return {
    set(callback, delayMs) {
      return setTimeout(callback, delayMs)
    },
    clear(handle) {
      clearTimeout(handle as ReturnType<typeof setTimeout>)
    },
  }
}

export type CreateDefaultAppleFmBridgeLauncherOptions = {
  /** Options forwarded to `discoverAppleFmBridgeHelper` (cwd/env/fileExists). */
  readonly discover?: DiscoverAppleFmBridgeHelperOptions
  /** Override helper discovery entirely (tests inject a stub). */
  readonly discoverHelper?: (
    options: DiscoverAppleFmBridgeHelperOptions,
  ) => DiscoveredAppleFmBridgeHelper | null
  /** Process-spawning seam; defaults to the `Bun.spawn`-backed spawner. */
  readonly spawnProcess?: AppleFmBridgeProcessSpawner
  /** Backoff timer; defaults to the `setTimeout`-backed timer. */
  readonly timer?: AppleFmBridgeSupervisorTimer
  /** Clock; defaults to `Date.now`. */
  readonly now?: () => number
  /** Optional supervision policy overrides. */
  readonly config?: Partial<AppleFmBridgeSupervisorConfig>
  /** Local loopback port the helper should bind. */
  readonly port?: number
  /** Extra args appended after the port flag, if a deployment needs them. */
  readonly extraArgs?: ReadonlyArray<string>
}

/**
 * A successfully-assembled launcher together with the helper it will supervise.
 * Callers that need the discovery source (env / source / packaged) for
 * diagnostics can read it off `helper`.
 */
export type DefaultAppleFmBridgeLauncher = {
  readonly helper: DiscoveredAppleFmBridgeHelper
  readonly launcher: AppleFmBridgeLauncher
}

/**
 * Discover the Foundation Models bridge helper and, if present, assemble a
 * fully-wired supervised launcher around it using production defaults.
 *
 * Returns `null` (never throws) when no helper can be discovered — the expected
 * outcome on a host that does not ship the bridge — so the caller can fall back
 * to the existing unsupervised `apple_fm.status` projection.
 */
export function createDefaultAppleFmBridgeLauncher(
  options: CreateDefaultAppleFmBridgeLauncherOptions = {},
): DefaultAppleFmBridgeLauncher | null {
  const discoverHelper = options.discoverHelper ?? discoverAppleFmBridgeHelper
  const helper = discoverHelper(options.discover ?? {})
  if (helper === null) {
    return null
  }

  const launcher = createAppleFmBridgeLauncher({
    helper,
    spawnProcess: options.spawnProcess ?? createBunAppleFmBridgeProcessSpawner(),
    now: options.now ?? Date.now,
    timer: options.timer ?? createSetTimeoutSupervisorTimer(),
    ...(options.config === undefined ? {} : { config: options.config }),
    ...(options.port === undefined ? {} : { port: options.port }),
    ...(options.extraArgs === undefined ? {} : { extraArgs: options.extraArgs }),
  })

  return { helper, launcher }
}
