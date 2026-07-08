/**
 * TypeScript consumer of the psionic provider-facing
 * `TassadarCapabilityEnvelope` posture (psionic
 * crates/psionic-provider/src/lib.rs) for Pylon capability reporting
 * (W4.1, openagents#4750). The Rust envelope wraps a served capability
 * publication in receipts so a provider can never advertise more than
 * its benchmarks proved; this module mirrors the same no-overclaim
 * contract for the bounded numeric-executor lane that ships with Pylon:
 *
 * - the executor capability is declared only behind a SELF-TEST receipt
 *   (a real digest-pinned execution on this device, compared
 *   byte-for-byte against the compile-pinned trace digest), never by
 *   configuration assertion;
 * - the declared profile (window version, legs supported, replay class)
 *   derives from compile/replay receipts, not free-form config strings;
 * - a failed or absent self-test produces a typed refusal and no
 *   declaration.
 *
 * Serving and pricing claims remain disclosure-gated and are out of
 * scope here, exactly as in the GEPA capability envelope.
 */

import {
  TASSADAR_ALM_NUMERIC_EXACT_WINDOW,
  type TassadarAlmNumericModel,
} from "./numeric-executor.js"
import {
  TASSADAR_TS_REPLAY_CLASS_ID,
  verifyTassadarFullReplay,
} from "./replay.js"
import { TASSADAR_EXECUTOR_CAPABILITY_REF } from "./lane.js"

export const TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_SCHEMA =
  "openagents.tassadar_executor.self_test_receipt.v1"
export const TASSADAR_EXECUTOR_CAPABILITY_ENVELOPE_SCHEMA =
  "openagents.pylon.tassadar_capability_envelope.v1"
export const TASSADAR_EXECUTOR_CAPABILITY_MATRIX_ROW_SCHEMA =
  "openagents.tassadar_executor.capability_matrix_row.v1"

/** Exactness-window version the executor enforces (2^53 checked window). */
export const TASSADAR_EXECUTOR_WINDOW_VERSION_REF =
  "window.tassadar_executor.exact_2p53.v1"
export const TASSADAR_EXECUTOR_EXACT_WINDOW_LABEL = "2^53" as const

/** Execution/verification legs this TS executor lane supports. */
export const TASSADAR_EXECUTOR_LEG_REFS = [
  "leg.tassadar_executor.alm_numeric_execute.v1",
  "leg.tassadar_executor.exact_trace_replay_full.v1",
  "leg.tassadar_executor.exact_trace_replay_window.v1",
] as const

export const TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PREFIX =
  "receipt.tassadar_executor.self_test.v1"

/**
 * Receipt refs are `receipt.tassadar_executor.self_test.v1.<16 hex>`,
 * where the suffix is the first 16 hex chars of the locally replayed
 * trace digest. Dotted segments keep the ref outside the public
 * long-base64url secret-scanner shape.
 */
export const TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN =
  /^receipt\.tassadar_executor\.self_test\.v1\.[0-9a-f]{16}$/

export const TASSADAR_COMPILE_RECEIPT_REF_PREFIX =
  "receipt.tassadar_compile.model_digest"

export const tassadarExecutorSelfTestReceiptRef = (
  replayedTraceDigest: string,
): string =>
  `${TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PREFIX}.${replayedTraceDigest.slice(0, 16)}`

export const tassadarCompileReceiptRef = (modelDigest: string): string =>
  `${TASSADAR_COMPILE_RECEIPT_REF_PREFIX}.${modelDigest.slice(0, 16)}`

/** Digest-pinned self-test workload: the compile-side pins plus the model and inputs. */
export type TassadarSelfTestWorkload = Readonly<{
  fixtureId: string
  expectedModelDigest: string
  expectedTraceDigest: string
  model: TassadarAlmNumericModel
  steps: ReadonlyArray<ReadonlyArray<number>>
}>

export type TassadarExecutorSelfTestReceipt = Readonly<{
  schema: typeof TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_SCHEMA
  capabilityRef: typeof TASSADAR_EXECUTOR_CAPABILITY_REF
  fixtureId: string
  /** Compile-side pinned digest of the workload model (compile receipt). */
  modelDigest: string
  graphDigest: string
  expectedTraceDigest: string
  replayedTraceDigest: string | null
  replayClassId: typeof TASSADAR_TS_REPLAY_CLASS_ID
  executorId: "tassadar.alm_numeric_executor.ts.v1"
  stepCount: number
  outcome: "verified" | "failed"
  refusalDetail: string | null
  observedAt: string
  /** Public-safe receipt ref; present only when the self-test verified. */
  receiptRef: string | null
}>

/**
 * Runs the real executor self-test: executes the digest-pinned workload
 * on this device via the shared executor and compares the trace digest
 * byte-for-byte. The returned receipt is public-safe digest-match
 * evidence; it never carries the model or the trace rows.
 */
export const runTassadarExecutorSelfTest = async (
  input: Readonly<{
    workload: TassadarSelfTestWorkload
    observedAt?: string
  }>,
): Promise<TassadarExecutorSelfTestReceipt> => {
  const { workload } = input
  const observedAt = input.observedAt ?? new Date().toISOString()
  const verdict = await verifyTassadarFullReplay({
    claimedTraceDigest: workload.expectedTraceDigest,
    model: workload.model,
    steps: workload.steps,
    validatorDeviceRef: "device.self_test.local",
  })
  const verified = verdict.outcome === "verified"
  return {
    capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
    executorId: "tassadar.alm_numeric_executor.ts.v1",
    expectedTraceDigest: workload.expectedTraceDigest,
    fixtureId: workload.fixtureId,
    graphDigest: workload.model.graph_digest,
    modelDigest: workload.expectedModelDigest,
    observedAt,
    outcome: verified ? "verified" : "failed",
    receiptRef:
      verified && verdict.replayedTraceDigest !== null
        ? tassadarExecutorSelfTestReceiptRef(verdict.replayedTraceDigest)
        : null,
    refusalDetail: verified
      ? null
      : verdict.rejection === null
        ? "self-test replay rejected without a named rejection"
        : verdict.rejection.reason === "execution_refused"
          ? `execution_refused: ${verdict.rejection.detail}`
          : verdict.rejection.reason,
    replayClassId: TASSADAR_TS_REPLAY_CLASS_ID,
    replayedTraceDigest: verdict.replayedTraceDigest,
    schema: TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_SCHEMA,
    stepCount: verdict.replayedSteps,
  }
}

export type TassadarExecutorCapabilityEnvelope = Readonly<{
  schema: typeof TASSADAR_EXECUTOR_CAPABILITY_ENVELOPE_SCHEMA
  capabilityRef: typeof TASSADAR_EXECUTOR_CAPABILITY_REF
  windowVersionRef: typeof TASSADAR_EXECUTOR_WINDOW_VERSION_REF
  exactnessWindowLabel: typeof TASSADAR_EXECUTOR_EXACT_WINDOW_LABEL
  exactnessWindow: number
  legRefs: ReadonlyArray<string>
  replayClassId: typeof TASSADAR_TS_REPLAY_CLASS_ID
  executorId: "tassadar.alm_numeric_executor.ts.v1"
  selfTestReceipt: TassadarExecutorSelfTestReceipt
}>

export type TassadarCapabilityDeclarationRefusal = Readonly<{
  kind: "self_test_failed" | "self_test_receipt_missing"
  refusalRef: "refusal.tassadar_executor.capability_undeclarable"
  detail: string
}>

export type TassadarCapabilityDeclaration =
  | Readonly<{
      declared: true
      envelope: TassadarExecutorCapabilityEnvelope
      matrixRow: TassadarCapabilityMatrixRow
      /** Refs a Pylon publishes for this capability: claim + receipt. */
      capabilityRefs: ReadonlyArray<string>
    }>
  | Readonly<{
      declared: false
      refusal: TassadarCapabilityDeclarationRefusal
    }>

/**
 * Builds the provider-facing envelope from a self-test receipt. Mirrors
 * `TassadarCapabilityEnvelope::from_executor_capability_publication`:
 * an unverified publication is a typed refusal, never a declaration.
 */
export const buildTassadarExecutorCapabilityDeclaration = (
  receipt: TassadarExecutorSelfTestReceipt,
): TassadarCapabilityDeclaration => {
  if (receipt.outcome !== "verified" || receipt.receiptRef === null) {
    return {
      declared: false,
      refusal: {
        detail:
          receipt.refusalDetail ??
          "executor self-test did not verify the pinned trace digest",
        kind:
          receipt.outcome === "verified"
            ? "self_test_receipt_missing"
            : "self_test_failed",
        refusalRef: "refusal.tassadar_executor.capability_undeclarable",
      },
    }
  }
  const envelope: TassadarExecutorCapabilityEnvelope = {
    capabilityRef: receipt.capabilityRef,
    exactnessWindow: TASSADAR_ALM_NUMERIC_EXACT_WINDOW,
    exactnessWindowLabel: TASSADAR_EXECUTOR_EXACT_WINDOW_LABEL,
    executorId: receipt.executorId,
    legRefs: [...TASSADAR_EXECUTOR_LEG_REFS],
    replayClassId: receipt.replayClassId,
    schema: TASSADAR_EXECUTOR_CAPABILITY_ENVELOPE_SCHEMA,
    selfTestReceipt: receipt,
    windowVersionRef: TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
  }
  return {
    capabilityRefs: [receipt.capabilityRef, receipt.receiptRef],
    declared: true,
    envelope,
    matrixRow: tassadarCapabilityMatrixRowFromEnvelope(envelope),
  }
}

/**
 * One row of the workload capability matrix, derived from receipts per
 * the original E6 design intent: the compile receipt pins the model
 * digest, the replay receipt pins the locally verified trace digest.
 * No field is a free-form configuration string.
 */
export type TassadarCapabilityMatrixRow = Readonly<{
  schema: typeof TASSADAR_EXECUTOR_CAPABILITY_MATRIX_ROW_SCHEMA
  capabilityRef: typeof TASSADAR_EXECUTOR_CAPABILITY_REF
  workloadFamilyRef: "workload.tassadar_executor.alm_numeric_trace.v1"
  windowVersionRef: typeof TASSADAR_EXECUTOR_WINDOW_VERSION_REF
  legRefs: ReadonlyArray<string>
  replayClassId: typeof TASSADAR_TS_REPLAY_CLASS_ID
  compileReceiptRef: string
  replayReceiptRef: string
  posture: "execute_exact_or_refuse"
}>

export const tassadarCapabilityMatrixRowFromEnvelope = (
  envelope: TassadarExecutorCapabilityEnvelope,
): TassadarCapabilityMatrixRow => {
  if (envelope.selfTestReceipt.receiptRef === null) {
    throw new TassadarCapabilityShapeError(
      "capability matrix row requires a verified self-test receipt ref",
    )
  }
  return {
    capabilityRef: envelope.capabilityRef,
    compileReceiptRef: tassadarCompileReceiptRef(
      envelope.selfTestReceipt.modelDigest,
    ),
    legRefs: envelope.legRefs,
    posture: "execute_exact_or_refuse",
    replayClassId: envelope.replayClassId,
    replayReceiptRef: envelope.selfTestReceipt.receiptRef,
    schema: TASSADAR_EXECUTOR_CAPABILITY_MATRIX_ROW_SCHEMA,
    windowVersionRef: envelope.windowVersionRef,
    workloadFamilyRef: "workload.tassadar_executor.alm_numeric_trace.v1",
  }
}

export class TassadarCapabilityShapeError extends Error {
  readonly _tag = "TassadarCapabilityShapeError"
}

const requireExact = (
  value: unknown,
  expected: string,
  field: string,
): void => {
  if (value !== expected) {
    throw new TassadarCapabilityShapeError(
      `capability matrix row field ${field} must be ${expected}, got ${String(value)}`,
    )
  }
}

/**
 * Shape-enforcing decoder for capability matrix rows: rows derive from
 * compile/replay receipts, so every receipt-bearing field must match
 * its receipt-derived pattern and every fixed field its envelope value.
 * Free-form configuration strings are rejected with a typed error.
 */
export const decodeTassadarCapabilityMatrixRow = (
  value: unknown,
): TassadarCapabilityMatrixRow => {
  if (value === null || typeof value !== "object") {
    throw new TassadarCapabilityShapeError(
      "capability matrix row must be an object",
    )
  }
  const row = value as Record<string, unknown>
  requireExact(
    row.schema,
    TASSADAR_EXECUTOR_CAPABILITY_MATRIX_ROW_SCHEMA,
    "schema",
  )
  requireExact(row.capabilityRef, TASSADAR_EXECUTOR_CAPABILITY_REF, "capabilityRef")
  requireExact(
    row.workloadFamilyRef,
    "workload.tassadar_executor.alm_numeric_trace.v1",
    "workloadFamilyRef",
  )
  requireExact(
    row.windowVersionRef,
    TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
    "windowVersionRef",
  )
  requireExact(row.replayClassId, TASSADAR_TS_REPLAY_CLASS_ID, "replayClassId")
  requireExact(row.posture, "execute_exact_or_refuse", "posture")
  const legRefs = row.legRefs
  if (
    !Array.isArray(legRefs) ||
    legRefs.length === 0 ||
    !legRefs.every(
      (leg) =>
        typeof leg === "string" &&
        (TASSADAR_EXECUTOR_LEG_REFS as ReadonlyArray<string>).includes(leg),
    )
  ) {
    throw new TassadarCapabilityShapeError(
      "capability matrix row legRefs must be a non-empty subset of the known executor legs",
    )
  }
  const compileReceiptRef = row.compileReceiptRef
  if (
    typeof compileReceiptRef !== "string" ||
    !new RegExp(
      `^${TASSADAR_COMPILE_RECEIPT_REF_PREFIX.replaceAll(".", "\\.")}\\.[0-9a-f]{16}$`,
    ).test(compileReceiptRef)
  ) {
    throw new TassadarCapabilityShapeError(
      "capability matrix row compileReceiptRef must derive from a model digest",
    )
  }
  const replayReceiptRef = row.replayReceiptRef
  if (
    typeof replayReceiptRef !== "string" ||
    !TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN.test(replayReceiptRef)
  ) {
    throw new TassadarCapabilityShapeError(
      "capability matrix row replayReceiptRef must derive from a verified self-test receipt",
    )
  }
  return {
    capabilityRef: TASSADAR_EXECUTOR_CAPABILITY_REF,
    compileReceiptRef,
    legRefs: legRefs as ReadonlyArray<string>,
    posture: "execute_exact_or_refuse",
    replayClassId: TASSADAR_TS_REPLAY_CLASS_ID,
    replayReceiptRef,
    schema: TASSADAR_EXECUTOR_CAPABILITY_MATRIX_ROW_SCHEMA,
    windowVersionRef: TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
    workloadFamilyRef: "workload.tassadar_executor.alm_numeric_trace.v1",
  }
}

export const isTassadarExecutorSelfTestReceiptRef = (ref: string): boolean =>
  TASSADAR_EXECUTOR_SELF_TEST_RECEIPT_REF_PATTERN.test(ref)

/** True when refs carry the executor capability AND a self-test receipt ref. */
export const hasReceiptedTassadarExecutorCapability = (
  refs: ReadonlyArray<string>,
): boolean =>
  refs.includes(TASSADAR_EXECUTOR_CAPABILITY_REF) &&
  refs.some(isTassadarExecutorSelfTestReceiptRef)

/**
 * Removes an unreceipted executor-capability claim (and orphaned
 * receipt refs) from a publishable ref list. Pylons run this before
 * registering so a configuration-asserted capability never reaches the
 * network without its self-test receipt.
 */
export const stripUnreceiptedTassadarExecutorCapability = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  if (hasReceiptedTassadarExecutorCapability(refs)) {
    return refs
  }
  return refs.filter(
    (ref) =>
      ref !== TASSADAR_EXECUTOR_CAPABILITY_REF &&
      !isTassadarExecutorSelfTestReceiptRef(ref),
  )
}
