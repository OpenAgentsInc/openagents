/**
 * Host lifecycle glue that bridges the supervised-launcher *assembly* seam to
 * the `apple_fm.status` *action* seam.
 *
 * Everything on both sides already exists:
 *   - `createDefaultAppleFmBridgeLauncher(...)` → discovers the Foundation Models
 *     bridge helper and, when present, assembles a fully-wired supervised
 *     launcher with production defaults (or returns `null` on a host that ships
 *     no helper).
 *   - `createSupervisedAppleFmStatusAction(base, { supervisorStatus })` → builds
 *     the `apple_fm.status` action, attaching a live supervisor phase to every
 *     projection when a `supervisorStatus` provider is supplied.
 *
 * What was still missing is the piece that *owns the launcher's lifecycle*:
 * nothing constructed the launcher, called `start()` to begin supervision, and
 * surfaced its `status()` as the provider the action consumes — plus a single
 * `stop()` the host can call on shutdown. The previous step's index.ts comment
 * said the provider gets wired "once a live launcher is constructed on this
 * host"; this module is that construction + lifecycle owner.
 *
 * `createAppleFmSupervisedLaunch(...)`:
 *   - assembles the launcher (injectable; defaults to the production factory),
 *   - when present, calls `start()` and returns `supervised: true` with a
 *     `supervisorStatus` provider (the launcher's `status`), a `notifyHealthy`
 *     pass-through for the bridge heartbeat, and an idempotent `stop`,
 *   - when absent (no helper / non-Apple host), returns a fully inert handle
 *     (`supervised: false`, `supervisorStatus: undefined`, no-op `notifyHealthy`
 *     / `stop`) so the caller falls back to the unsupervised projection with
 *     byte-identical behaviour.
 *
 * The assembly factory is injectable so this stays deterministic in tests with
 * no real process. This module reads no wall clock and introduces no prompts,
 * file contents, paths, tokens, URLs, or bearer material.
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import type { DiscoveredAppleFmBridgeHelper } from "./apple-fm-bridge-helper.js"
import {
  createDefaultAppleFmBridgeLauncher,
  type CreateDefaultAppleFmBridgeLauncherOptions,
  type DefaultAppleFmBridgeLauncher,
} from "./apple-fm-bridge-launcher-host.js"
import type { AppleFmSupervisorStatusProvider } from "./apple-fm-supervised-status.js"

export type CreateAppleFmSupervisedLaunchOptions =
  CreateDefaultAppleFmBridgeLauncherOptions & {
    /**
     * Override the launcher assembly entirely (tests inject a stub). Defaults to
     * the production `createDefaultAppleFmBridgeLauncher`.
     */
    readonly assemble?: (
      options: CreateDefaultAppleFmBridgeLauncherOptions,
    ) => DefaultAppleFmBridgeLauncher | null
  }

/**
 * The lifecycle handle the host holds onto. `supervisorStatus` is passed
 * straight to `createSupervisedAppleFmStatusAction`; `stop` is called on
 * shutdown; `notifyHealthy` forwards the live bridge heartbeat.
 */
export type AppleFmSupervisedLaunch = {
  /** True when a helper was found and supervision was started. */
  readonly supervised: boolean
  /** The discovered helper, or null when none was found. */
  readonly helper: DiscoveredAppleFmBridgeHelper | null
  /**
   * Provider to hand to `createSupervisedAppleFmStatusAction`. `undefined` when
   * unsupervised, so the action returns the unsupervised projection unchanged.
   */
  readonly supervisorStatus: AppleFmSupervisorStatusProvider | undefined
  /** Forward a healthy heartbeat from the bridge into the policy. No-op when unsupervised. */
  notifyHealthy(): void
  /** Stop supervision and kill any live child. Idempotent; no-op when unsupervised. */
  stop(): void
}

/** Inert lifecycle handle for a host that ships no helper (non-Apple, etc.). */
function unsupervised(): AppleFmSupervisedLaunch {
  return {
    supervised: false,
    helper: null,
    supervisorStatus: undefined,
    notifyHealthy() {},
    stop() {},
  }
}

/**
 * Construct, start, and take ownership of the supervised Apple FM bridge
 * launcher on this host. Returns an inert handle when no helper is present.
 */
export function createAppleFmSupervisedLaunch(
  options: CreateAppleFmSupervisedLaunchOptions = {},
): AppleFmSupervisedLaunch {
  const { assemble, ...launcherOptions } = options
  const factory = assemble ?? createDefaultAppleFmBridgeLauncher
  const assembled = factory(launcherOptions)
  if (assembled === null) {
    return unsupervised()
  }

  const { helper, launcher } = assembled
  launcher.start()

  let stopped = false
  return {
    supervised: true,
    helper,
    supervisorStatus: () => launcher.status(),
    notifyHealthy() {
      if (stopped) return
      launcher.notifyHealthy()
    },
    stop() {
      if (stopped) return
      stopped = true
      launcher.stop()
    },
  }
}
