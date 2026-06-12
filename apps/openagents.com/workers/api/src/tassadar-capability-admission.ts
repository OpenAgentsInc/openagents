/**
 * Worker-side admission for the Tassadar executor capability claim
 * (W4.1, openagents#4750). Mirrors the psionic provider-facing
 * `TassadarCapabilityEnvelope` posture: a Pylon may advertise
 * `capability.tassadar_poc.numeric_model_executor` only together with a
 * self-test receipt ref minted by a real digest-pinned execution on the
 * device. A claim without the receipt is refused with a typed refusal
 * ref and never reaches the stored registration row, so the dispatch
 * gate and the administrator-tick eligibility filter only ever see
 * receipted executor capacity. Serving/pricing claims remain
 * disclosure-gated and are out of scope.
 */
import { Schema as S } from 'effect'

import {
  TASSADAR_EXECUTOR_CAPABILITY_REF,
  TASSADAR_EXECUTOR_LEG_REFS,
  TASSADAR_EXECUTOR_WINDOW_VERSION_REF,
  TASSADAR_TS_REPLAY_CLASS_ID,
  hasReceiptedTassadarExecutorCapability,
  isTassadarExecutorSelfTestReceiptRef,
} from '@openagentsinc/tassadar-executor'

export const TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF =
  'refusal.public.pylon_capability.tassadar_executor_unreceipted'

export const TASSADAR_DISPATCH_CAPABILITY_UNRECEIPTED_BLOCKER_REF =
  'blocker.public.pylon_dispatch.tassadar_capability_unreceipted'

/** Self-test receipt refs: `receipt.tassadar_executor.self_test.v1.<16 hex>`. */
export const TassadarExecutorSelfTestReceiptRef = S.String.check(
  S.isPattern(/^receipt\.tassadar_executor\.self_test\.v1\.[0-9a-f]{16}$/),
)

/**
 * Schema-enforced capability matrix row: every receipt-bearing field
 * must derive from a compile or replay receipt, every fixed field must
 * match the envelope contract. Free-form configuration strings fail
 * decoding (the E6 design intent: rows derive from receipts).
 */
export const TassadarExecutorCapabilityMatrixRow = S.Struct({
  capabilityRef: S.Literal(TASSADAR_EXECUTOR_CAPABILITY_REF),
  compileReceiptRef: S.String.check(
    S.isPattern(/^receipt\.tassadar_compile\.model_digest\.[0-9a-f]{16}$/),
  ),
  legRefs: S.Array(
    S.Literals([...TASSADAR_EXECUTOR_LEG_REFS]),
  ).check(S.isMinLength(1)),
  posture: S.Literal('execute_exact_or_refuse'),
  replayClassId: S.Literal(TASSADAR_TS_REPLAY_CLASS_ID),
  replayReceiptRef: TassadarExecutorSelfTestReceiptRef,
  schema: S.Literal('openagents.tassadar_executor.capability_matrix_row.v1'),
  windowVersionRef: S.Literal(TASSADAR_EXECUTOR_WINDOW_VERSION_REF),
  workloadFamilyRef: S.Literal(
    'workload.tassadar_executor.alm_numeric_trace.v1',
  ),
})
export type TassadarExecutorCapabilityMatrixRow =
  typeof TassadarExecutorCapabilityMatrixRow.Type

export type TassadarExecutorCapabilityAdmission = Readonly<{
  state: 'admitted' | 'refused' | 'not_claimed'
  /** Capability refs that may be stored on the registration row. */
  admittedCapabilityRefs: ReadonlyArray<string>
  refusalRefs: ReadonlyArray<string>
  selfTestReceiptRefs: ReadonlyArray<string>
}>

/**
 * Admits or refuses an executor-capability claim. Refusal strips the
 * capability ref (and orphaned receipt refs) from the storable set so
 * an unreceipted claim never becomes dispatchable registry state.
 */
export const admitTassadarExecutorCapabilityClaim = (
  capabilityRefs: ReadonlyArray<string>,
): TassadarExecutorCapabilityAdmission => {
  const claimed = capabilityRefs.includes(TASSADAR_EXECUTOR_CAPABILITY_REF)
  const selfTestReceiptRefs = capabilityRefs.filter(
    isTassadarExecutorSelfTestReceiptRef,
  )

  if (!claimed) {
    return {
      admittedCapabilityRefs: capabilityRefs.filter(
        ref => !isTassadarExecutorSelfTestReceiptRef(ref),
      ),
      refusalRefs: [],
      selfTestReceiptRefs: [],
      state: 'not_claimed',
    }
  }

  if (selfTestReceiptRefs.length === 0) {
    return {
      admittedCapabilityRefs: capabilityRefs.filter(
        ref => ref !== TASSADAR_EXECUTOR_CAPABILITY_REF,
      ),
      refusalRefs: [TASSADAR_EXECUTOR_CAPABILITY_UNRECEIPTED_REFUSAL_REF],
      selfTestReceiptRefs: [],
      state: 'refused',
    }
  }

  return {
    admittedCapabilityRefs: capabilityRefs,
    refusalRefs: [],
    selfTestReceiptRefs,
    state: 'admitted',
  }
}

/**
 * Dispatch-gate predicate: when an assignment requires the executor
 * capability, the target registration must hold the receipted claim.
 * Registrations written before W4.1 may still carry the bare capability
 * ref; they stay blocked until the device re-runs go-online and its
 * self-test receipt lands.
 */
export const tassadarDispatchCapabilityUnreceipted = (
  requiredCapabilityRefs: ReadonlyArray<string>,
  registrationCapabilityRefs: ReadonlyArray<string>,
): boolean =>
  requiredCapabilityRefs.includes(TASSADAR_EXECUTOR_CAPABILITY_REF) &&
  !hasReceiptedTassadarExecutorCapability(registrationCapabilityRefs)

/**
 * Administrator-tick eligibility predicate: an online Pylon counts as
 * executor-eligible only with the receipted capability.
 */
export const pylonCapabilityRefsEligibleForExecutorDispatch = (
  capabilityRefs: ReadonlyArray<string>,
): boolean => hasReceiptedTassadarExecutorCapability(capabilityRefs)
