/**
 * Host glue: register a local bridge-supervisor `status()` with the Pylon
 * `apple_fm.status` action.
 *
 * The deterministic supervision core already exists end-to-end:
 *   - `reduceAppleFmBridgeSupervisor`        → *when* to (re)start / give up (pure).
 *   - `createAppleFmBridgeSupervisorDriver`  → holds that state + self-fires backoff.
 *   - `createAppleFmBridgeLauncher`          → glues the driver to a real child.
 *   - `createDefaultAppleFmBridgeLauncher`   → assembles the launcher with prod defaults.
 *   - `summarizeAppleFmBridgeSupervisor` / `withAppleFmSupervisorStatus`
 *                                            → project + carry the phase onto the
 *                                              `apple_fm.status` projection.
 *
 * What was still missing is the *action seam*: nothing combined the base
 * `collectPylonAppleFmStatus(...)` projection with a live launcher's `status()`
 * at request time, so the supervisor phase never actually reached the
 * `apple_fm.status` action that Autopilot Desktop calls. This module supplies
 * exactly that seam.
 *
 * `createSupervisedAppleFmStatusAction(...)` returns the `() => Promise<...>`
 * action the control server registers. On every call it:
 *   1. collects the base capability projection (the existing behaviour), then
 *   2. asks the injected supervisor-status provider for the current phase and,
 *      when one is present, merges it in via `withAppleFmSupervisorStatus`.
 *
 * When no provider is supplied (or it returns null/undefined — e.g. a host that
 * ships no helper, so `createDefaultAppleFmBridgeLauncher` returned null), the
 * action is byte-for-byte the previous unsupervised projection. Both the
 * collector and the provider are injectable so the seam is deterministic in
 * tests. This module reads no wall clock and introduces no prompts, file
 * contents, paths, tokens, URLs, or bearer material.
 *
 * It advances (does NOT clear):
 *   blocker.product_promises.local_apple_fm_helper_supervision_missing
 */

import type { PylonAppleFmSupervisorStatus } from "./apple-fm-bridge-supervisor-status.js"
import {
  collectPylonAppleFmStatus,
  withAppleFmSupervisorStatus,
  type CollectPylonAppleFmStatusInput,
  type PylonAppleFmStatusProjection,
} from "./apple-fm-status.js"

/**
 * Reads the current public-safe supervisor phase, or null/undefined when no
 * launcher is supervising a helper on this host. A live launcher's `status`
 * method satisfies this directly.
 */
export type AppleFmSupervisorStatusProvider = () =>
  | PylonAppleFmSupervisorStatus
  | null
  | undefined

export type CreateSupervisedAppleFmStatusActionOptions = {
  /**
   * Provider for the live supervision phase. Omit (or pass one that returns
   * null) when supervision is not wired on this host — the action then returns
   * the unsupervised projection unchanged.
   */
  readonly supervisorStatus?: AppleFmSupervisorStatusProvider
  /** Base capability collector; defaults to `collectPylonAppleFmStatus`. */
  readonly collect?: (
    input: CollectPylonAppleFmStatusInput,
  ) => Promise<PylonAppleFmStatusProjection>
}

/**
 * Build the `apple_fm.status` action, optionally attaching a live supervisor's
 * phase to every projection it returns.
 */
export function createSupervisedAppleFmStatusAction(
  baseInput: CollectPylonAppleFmStatusInput,
  options: CreateSupervisedAppleFmStatusActionOptions = {},
): () => Promise<PylonAppleFmStatusProjection> {
  const collect = options.collect ?? collectPylonAppleFmStatus
  const supervisorStatus = options.supervisorStatus

  return async () => {
    const projection = await collect(baseInput)
    if (supervisorStatus === undefined) {
      return projection
    }
    const supervisor = supervisorStatus()
    if (supervisor === null || supervisor === undefined) {
      return projection
    }
    return withAppleFmSupervisorStatus(projection, supervisor)
  }
}
