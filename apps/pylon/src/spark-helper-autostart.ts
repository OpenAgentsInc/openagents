// Spark-helper autostart readiness — INERT, flag-gated capability.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.spark_helper_autostart_receipt_missing
//
// The video-core promise ("anybody plugs in consumer compute and gets paid
// Bitcoin") is blocked, in part, because there is no receipt proving that a
// NORMAL contributor's Spark backup helper reaches payout-readiness
// automatically on the self-serve path — without an operator hand-starting it.
//
// This module supplies that capability WITHOUT changing any live behavior:
//
//   - It is INERT by default. The autostart decision only fires when the
//     operator/contributor explicitly opts in via `PYLON_SPARK_AUTOSTART=1`
//     (or `{ enabled: true }`). With the flag off it returns a `disabled`
//     projection and recommends nothing — exactly today's behavior.
//   - It does NOT start a helper, spawn a process, move funds, or touch the
//     wallet. It is a pure classifier over an already-computed
//     `SparkBackupReceiveProjection`, plus a public-safe receipt builder.
//   - It never emits a raw Spark address, lightning address, balance, credential
//     material, or stderr. The receipt is redacted by construction.
//
// When the live self-serve path later wires actual helper autostart, it can call
// `classifySparkHelperAutostart` to gate that action and `buildSparkHelperAutostartReceipt`
// to emit the dereferenceable receipt that clears the blocker. Until then the
// flag stays off and nothing changes.

import type {
  SparkBackupReceiveProjection,
  SparkBackupReceiveState,
} from "./wallet"

export const SPARK_AUTOSTART_ENV = "PYLON_SPARK_AUTOSTART" as const

/**
 * The autostart-readiness states.
 *
 * - `disabled`            — flag off; no autostart decision made (default/inert).
 * - `credential-missing`  — opted in, but no local Spark backup credential.
 * - `helper-not-ready`    — opted in, credential present, but the helper has not
 *                           reached an address-ready state.
 * - `autostart-ready`     — opted in, credential present, helper reached a
 *                           payout-ready receive target WITHOUT operator action.
 */
export type SparkHelperAutostartState =
  | "disabled"
  | "credential-missing"
  | "helper-not-ready"
  | "autostart-ready"

export type SparkHelperAutostartProjection = {
  schema: "openagents.pylon.spark_helper_autostart.v0.1"
  enabled: boolean
  state: SparkHelperAutostartState
  // True only in `autostart-ready`: a normal contributor's helper is payout-ready
  // with no operator hand-start required for THIS observation.
  payoutReady: boolean
  // Mirrors the underlying spark-backup receive state this decision was derived
  // from (for traceability). Never carries raw targets.
  derivedFromReceiveState: SparkBackupReceiveState
  blockerRefs: string[]
  nextActionRefs: string[]
  // Public-safe redacted ref for the readiness observation, present only when
  // `autostart-ready`. Never a raw address/balance/credential.
  readinessReceiptRef: string | null
  contentRedacted: true
}

export type SparkHelperAutostartOptions = {
  // Explicit override. When undefined, falls back to the env flag.
  enabled?: boolean
  env?: NodeJS.ProcessEnv
}

const READY_RECEIVE_STATES: ReadonlySet<SparkBackupReceiveState> = new Set<
  SparkBackupReceiveState
>(["address-ready", "cached-address-ready"])

function isAutostartEnabled(
  options: SparkHelperAutostartOptions,
  env: NodeJS.ProcessEnv,
): boolean {
  if (options.enabled !== undefined) return options.enabled
  // INERT by default: only an explicit opt-in turns this on.
  const flag = env[SPARK_AUTOSTART_ENV]
  return flag === "1" || flag === "true"
}

/**
 * Classify whether the Spark backup helper would be considered autostart-ready
 * for a normal contributor, given an already-computed receive projection.
 *
 * Pure and side-effect-free. Default-inert: returns `disabled` unless explicitly
 * opted in.
 */
export function classifySparkHelperAutostart(
  receive: SparkBackupReceiveProjection,
  options: SparkHelperAutostartOptions = {},
): SparkHelperAutostartProjection {
  const env = options.env ?? process.env
  const enabled = isAutostartEnabled(options, env)

  const base: SparkHelperAutostartProjection = {
    schema: "openagents.pylon.spark_helper_autostart.v0.1",
    enabled,
    state: "disabled",
    payoutReady: false,
    derivedFromReceiveState: receive.state,
    blockerRefs: [],
    nextActionRefs: [],
    readinessReceiptRef: null,
    contentRedacted: true,
  }

  if (!enabled) {
    return {
      ...base,
      state: "disabled",
      nextActionRefs: ["action.pylon.spark_autostart.opt_in"],
    }
  }

  if (!receive.credentialReady) {
    return {
      ...base,
      state: "credential-missing",
      blockerRefs: ["blocker.wallet.spark_backup.credential_missing"],
      nextActionRefs: ["action.wallet.spark_backup.configure_local_credential"],
    }
  }

  if (!receive.helperReady && !READY_RECEIVE_STATES.has(receive.state)) {
    return {
      ...base,
      state: "helper-not-ready",
      blockerRefs: ["blocker.product_promises.spark_helper_autostart_receipt_missing"],
      nextActionRefs: ["action.wallet.spark_backup.install_or_start_helper"],
    }
  }

  // The helper reached a payout-ready receive target. For autostart purposes we
  // accept both a fresh `address-ready` and an offline `cached-address-ready`
  // (a cached target is still a payout destination the contributor can receive
  // to without operator action).
  return {
    ...base,
    state: "autostart-ready",
    payoutReady: true,
    readinessReceiptRef: buildSparkHelperAutostartReceiptRef(receive.state),
  }
}

/**
 * Build the public-safe, redacted receipt ref for an autostart-ready
 * observation. Carries only the receive state class, never a raw target.
 */
export function buildSparkHelperAutostartReceiptRef(
  receiveState: SparkBackupReceiveState,
): string {
  return `receipt.pylon.spark_helper_autostart.${receiveState}.v0.1`
}

export type SparkHelperAutostartReceipt = {
  schema: "openagents.pylon.spark_helper_autostart_receipt.v0.1"
  ref: string
  payoutReady: boolean
  derivedFromReceiveState: SparkBackupReceiveState
  // No operator hand-start was required for this observation.
  operatorHandStartRequired: boolean
  observedAt: string
  contentRedacted: true
}

/**
 * Build a dereferenceable, public-safe receipt for an autostart-ready
 * observation. Returns null unless the projection is `autostart-ready`, so a
 * receipt can only exist when readiness was actually observed.
 */
export function buildSparkHelperAutostartReceipt(
  projection: SparkHelperAutostartProjection,
  observedAt: string,
): SparkHelperAutostartReceipt | null {
  if (projection.state !== "autostart-ready" || projection.readinessReceiptRef === null) {
    return null
  }
  return {
    schema: "openagents.pylon.spark_helper_autostart_receipt.v0.1",
    ref: projection.readinessReceiptRef,
    payoutReady: projection.payoutReady,
    derivedFromReceiveState: projection.derivedFromReceiveState,
    operatorHandStartRequired: false,
    observedAt,
    contentRedacted: true,
  }
}
