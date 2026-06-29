import { TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND } from '@openagentsinc/tassadar-executor'
import { Schema as S } from 'effect'

import type { BuyModeDispatchInput } from './buy-mode-dispatcher'
import type { TrainingVerificationChallengeCreateRequest } from './training-verification'

const ExecutorTraceCloseoutRef = S.Trim.check(
  S.isNonEmpty(),
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const ExecutorTraceSampledStep = S.Number.check(
  S.isInt(),
  S.isBetween({ minimum: 0, maximum: 100_000_000 }),
)

export const TassadarExecutorTraceJobKind =
  TASSADAR_EXECUTOR_TRACE_HOMEWORK_JOB_KIND
export const TassadarExactTraceReplayVerificationClass = 'exact_trace_replay'
export const TassadarPsionicConnectorRef =
  'psionic.connector.bounded_executor_trace.v1'
export const TassadarBoundedProfileRef =
  'tassadar-article-transformer-trace-bound-trained-v0'
export const TassadarBoundedRouteRef =
  'tassadar.article_route.direct_hull_cache_runtime.v1'
export const TassadarInternalWorkClassRef =
  'tassadar.internal_compute.article_closeout.v1'
export const TassadarRequestSchemaRef =
  'openagents.tassadar_executor_trace_request.v1'
export const TassadarOutputSchemaRef =
  'openagents.tassadar_executor_trace_output.v1'

export const TassadarExecutorTraceWorkloadFamilies = [
  'article_closeout',
  'sudoku_trace',
  'hungarian_trace',
  'kernel_trace',
] as const
export type TassadarExecutorTraceWorkloadFamily =
  (typeof TassadarExecutorTraceWorkloadFamilies)[number]

export type TassadarExecutorTracePayload = Readonly<{
  assignmentRef: string
  boundedProfileRef: typeof TassadarBoundedProfileRef
  boundedRouteRef: typeof TassadarBoundedRouteRef
  connectorRef: typeof TassadarPsionicConnectorRef
  disclosureBoundaryRefs: ReadonlyArray<string>
  jobKind: typeof TassadarExecutorTraceJobKind
  outputSchemaRef: typeof TassadarOutputSchemaRef
  publicAcceptanceVerdictProjectionAllowed: false
  publicCapabilityCopyAllowed: false
  requestSchemaRef: typeof TassadarRequestSchemaRef
  sampledWindowPolicyRef: string
  verificationClass: typeof TassadarExactTraceReplayVerificationClass
  workClassRef: typeof TassadarInternalWorkClassRef
  workloadFamily: TassadarExecutorTraceWorkloadFamily
}>

export type TassadarExecutorTraceDispatchInput =
  Omit<BuyModeDispatchInput, 'content'> &
    Readonly<{
      assignmentRef: string
      workloadFamily: TassadarExecutorTraceWorkloadFamily
    }>

export type TassadarExecutorTraceCloseoutEvidence = Readonly<{
  assignmentRef: string
  pylonDeviceRef: string
  replayDigestRef: string
  sampledWindow: Readonly<{
    endStep: number
    startStep: number
  }>
  sampledWindowRef: string
  traceCommitmentDigestRef: string
  validatorDeviceRef: string
  workerReceiptRef: string
  workloadFamily: TassadarExecutorTraceWorkloadFamily
}>

/**
 * Decoder for a public-safe executor-trace closeout submission (#5008). All
 * fields are public-safe refs or bounded step integers; no raw prompts, host
 * paths, wallet material, or preimages are accepted.
 */
export const TassadarExecutorTraceCloseoutEvidenceSchema = S.Struct({
  assignmentRef: ExecutorTraceCloseoutRef,
  pylonDeviceRef: ExecutorTraceCloseoutRef,
  replayDigestRef: ExecutorTraceCloseoutRef,
  sampledWindow: S.Struct({
    endStep: ExecutorTraceSampledStep,
    startStep: ExecutorTraceSampledStep,
  }),
  sampledWindowRef: ExecutorTraceCloseoutRef,
  traceCommitmentDigestRef: ExecutorTraceCloseoutRef,
  validatorDeviceRef: ExecutorTraceCloseoutRef,
  workerReceiptRef: ExecutorTraceCloseoutRef,
  workloadFamily: S.Literals([...TassadarExecutorTraceWorkloadFamilies]),
})

export type TassadarExecutorTraceDisclosureChecklist = Readonly<{
  agentsCapabilityClaimAllowed: false
  marketingCopyAllowed: false
  publicAcceptanceVerdictProjectionAllowed: false
  publicCapabilityCopyAllowed: false
  registryCapabilityEditAllowed: false
  requiredReviewRefs: ReadonlyArray<string>
}>

export type TassadarExecutorTraceReadiness = Readonly<{
  blockerRefs: ReadonlyArray<string>
  dispatchableJobKind: typeof TassadarExecutorTraceJobKind
  noSpendDispatchReady: boolean
  paidSettlementReady: false
  separateDeviceReplayRequired: true
  verificationClass: typeof TassadarExactTraceReplayVerificationClass
}>

export class TassadarExecutorTraceUnsafeProjectionError extends Error {
  readonly _tag = 'TassadarExecutorTraceUnsafeProjectionError'
}

const unsafeProjectionPattern =
  /(api[_-]?key|bearer|bolt11|invoice|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|seed|wallet)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeProjectionPattern.test(json)) {
    throw new TassadarExecutorTraceUnsafeProjectionError(
      'Tassadar executor-trace payload must stay internal and disclosure-bounded.',
    )
  }

  return json
}

export const tassadarExecutorTraceDisclosureChecklist =
  (): TassadarExecutorTraceDisclosureChecklist => ({
    agentsCapabilityClaimAllowed: false,
    marketingCopyAllowed: false,
    publicAcceptanceVerdictProjectionAllowed: false,
    publicCapabilityCopyAllowed: false,
    registryCapabilityEditAllowed: false,
    requiredReviewRefs: [
      'docs/tassadar/2026-06-10-tassadar-percepta-audit.md',
      'docs/tassadar/work-that-proves-itself.md',
      'issue.github.openagents.4684.disclosure_boundary',
    ],
  })

export const buildTassadarExecutorTracePayload = (
  input: Readonly<{
    assignmentRef: string
    workloadFamily: TassadarExecutorTraceWorkloadFamily
  }>,
): TassadarExecutorTracePayload => {
  const payload: TassadarExecutorTracePayload = {
    assignmentRef: input.assignmentRef,
    boundedProfileRef: TassadarBoundedProfileRef,
    boundedRouteRef: TassadarBoundedRouteRef,
    connectorRef: TassadarPsionicConnectorRef,
    disclosureBoundaryRefs: [
      'boundary.tassadar.no_public_acceptance_verdict_projection',
      'boundary.tassadar.no_registry_capability_promise',
      'boundary.tassadar.bounded_psionic_profiles_only',
    ],
    jobKind: TassadarExecutorTraceJobKind,
    outputSchemaRef: TassadarOutputSchemaRef,
    publicAcceptanceVerdictProjectionAllowed: false,
    publicCapabilityCopyAllowed: false,
    requestSchemaRef: TassadarRequestSchemaRef,
    sampledWindowPolicyRef: 'policy.tassadar.sampled_window_exact_replay.v1',
    verificationClass: TassadarExactTraceReplayVerificationClass,
    workClassRef: TassadarInternalWorkClassRef,
    workloadFamily: input.workloadFamily,
  }

  publicSafeJson(payload)

  return payload
}

export const buildTassadarExecutorTraceDispatch = (
  input: TassadarExecutorTraceDispatchInput,
): BuyModeDispatchInput => ({
  amountMsats: input.amountMsats,
  campaignId: input.campaignId,
  content: publicSafeJson(
    buildTassadarExecutorTracePayload({
      assignmentRef: input.assignmentRef,
      workloadFamily: input.workloadFamily,
    }),
  ),
  idempotencyKeyHash: input.idempotencyKeyHash,
  jobId: input.jobId,
  nowIso: input.nowIso,
  providerPubkeys: input.providerPubkeys,
})

export const tassadarExecutorTraceVerificationChallengeRequest = (
  input: Readonly<{
    closeout: TassadarExecutorTraceCloseoutEvidence
    trainingRunRef: string
    windowRef: string
  }>,
): TrainingVerificationChallengeCreateRequest => {
  if (input.closeout.pylonDeviceRef === input.closeout.validatorDeviceRef) {
    throw new TassadarExecutorTraceUnsafeProjectionError(
      'exact_trace_replay requires a validator device distinct from the worker Pylon.',
    )
  }

  return {
    commitmentRefs: [
      `commitment.tassadar_executor_trace.${input.closeout.assignmentRef}.${input.closeout.workloadFamily}`,
    ],
    contributionRef: `contribution.tassadar_executor_trace.${input.closeout.assignmentRef}.${input.closeout.workloadFamily}`,
    homeworkKind: TassadarExecutorTraceJobKind,
    payload: {
      contributionRefs: [
        `contribution.tassadar_executor_trace.${input.closeout.assignmentRef}.${input.closeout.workloadFamily}`,
      ],
      pylonDeviceRef: input.closeout.pylonDeviceRef,
      replayDigestRef: input.closeout.replayDigestRef,
      sampledWindow: input.closeout.sampledWindow,
      sampledWindowRef: input.closeout.sampledWindowRef,
      traceCommitmentDigestRef: input.closeout.traceCommitmentDigestRef,
      validatorDeviceRef: input.closeout.validatorDeviceRef,
      workloadFamily: input.closeout.workloadFamily,
    },
    samplingPolicy: 'per_contribution',
    trainingRunRef: input.trainingRunRef,
    verificationClass: TassadarExactTraceReplayVerificationClass,
    windowRef: input.windowRef,
  }
}

export const tassadarExecutorTraceReadiness =
  (): TassadarExecutorTraceReadiness => ({
    blockerRefs: [
      'blocker.tassadar.requires_psionic_connector_4664_live_endpoint',
      'blocker.tassadar.requires_separate_device_replay_receipt',
      'blocker.tassadar.requires_operator_funded_paid_closeout',
      'blocker.tassadar.public_acceptance_verdicts_suppressed_by_policy',
    ],
    dispatchableJobKind: TassadarExecutorTraceJobKind,
    noSpendDispatchReady: true,
    paidSettlementReady: false,
    separateDeviceReplayRequired: true,
    verificationClass: TassadarExactTraceReplayVerificationClass,
  })
