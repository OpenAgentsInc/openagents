/**
 * Server-side exact-replay validation for the Tassadar executor-trace
 * proof of concept (compute.tassadar_executor_poc.v1, issue #4692).
 *
 * The production Cloudflare Worker acts as the validator device: it
 * re-executes the dispatched digest-pinned workload itself and reports
 * the digest it computed, so the replay evidence is produced on a
 * physically separate machine from the worker Pylon. The verdict then
 * flows through the standard training-verification challenge lifecycle
 * (#4674, exact_trace_replay class).
 *
 * The contributor weak-device validator lane (#4676) remains the target
 * architecture; this route is the bounded v1 validator and says so.
 */
import * as S from 'effect/Schema'

import {
  collectInterpreterOutputs,
  executeTassadarNumericModel,
  TassadarNumericExecutionError,
  type TassadarAlmNumericModel,
} from '@openagentsinc/tassadar-executor'
import {
  TassadarDenseModuleError,
  executeTassadarDenseWeightModule,
  type TassadarDenseWeightModule,
} from '@openagentsinc/tassadar-executor/dense-weight-module'
import {
  verifyTassadarLinkedDenseComposition,
  type TassadarLinkedDenseProgramFixture,
  type TassadarLinkedDenseReplayVerification,
} from '@openagentsinc/tassadar-executor/linked-dense-module'

export const TassadarReplayValidatorDeviceRef =
  'device.cloudflare_worker.openagents_api'

export const TassadarReplayRequest = S.Struct({
  assignmentRef: S.String,
  claimedTraceDigest: S.String,
  pylonDeviceRef: S.String,
  workload: S.Struct({
    denseModule: S.optionalKey(S.Record(S.String, S.Unknown)),
    linkedDenseFixture: S.optionalKey(S.Record(S.String, S.Unknown)),
    model: S.Record(S.String, S.Unknown),
    steps: S.Array(S.Array(S.Number)),
  }),
})
export type TassadarReplayRequest = typeof TassadarReplayRequest.Type

export type TassadarReplayResponse = Readonly<{
  assignmentRef: string
  claimedTraceDigest: string
  halted: boolean
  outcome: 'verified' | 'rejected'
  outputCount: number
  pylonDeviceRef: string
  rejectionReason:
    | 'trace_digest_mismatch'
    | 'execution_refused'
    | 'composition_verification_failed'
    | null
  compositionVerification: TassadarLinkedDenseReplayVerification | null
  replayedSteps: number
  replayedTraceDigest: string | null
  validatorDeviceRef: typeof TassadarReplayValidatorDeviceRef
}>

/** Restores the transit shape (initialChannelWrites) to the executor wire format. */
const restoreTransitModel = (
  model: Record<string, unknown>,
): TassadarAlmNumericModel => {
  if (model['seed_writes'] === undefined && model['initialChannelWrites'] !== undefined) {
    const { initialChannelWrites, ...rest } = model
    return { ...rest, seed_writes: initialChannelWrites } as TassadarAlmNumericModel
  }
  return model as unknown as TassadarAlmNumericModel
}

const runLinkedDenseCompositionValidation = async (
  request: TassadarReplayRequest,
  base: Omit<TassadarReplayResponse, 'compositionVerification' | 'halted' | 'outcome' | 'outputCount' | 'rejectionReason' | 'replayedSteps' | 'replayedTraceDigest'>,
): Promise<TassadarReplayResponse> => {
  const verification = await verifyTassadarLinkedDenseComposition(
    request.workload.linkedDenseFixture as TassadarLinkedDenseProgramFixture,
  )
  const digestMatches =
    verification.composedTraceDigest === request.claimedTraceDigest
  const verified = verification.compositionVerificationCleared && digestMatches
  return {
    ...base,
    compositionVerification: verification,
    halted: false,
    outcome: verified ? 'verified' : 'rejected',
    outputCount: 0,
    rejectionReason: verified
      ? null
      : digestMatches
        ? 'composition_verification_failed'
        : 'trace_digest_mismatch',
    replayedSteps: request.workload.steps.length,
    replayedTraceDigest: verification.composedTraceDigest,
  }
}

export const runTassadarReplayValidation = async (
  request: TassadarReplayRequest,
): Promise<TassadarReplayResponse> => {
  const base = {
    assignmentRef: request.assignmentRef,
    claimedTraceDigest: request.claimedTraceDigest,
    pylonDeviceRef: request.pylonDeviceRef,
    validatorDeviceRef: TassadarReplayValidatorDeviceRef,
  } as const
  try {
    if (request.workload.linkedDenseFixture !== undefined) {
      return await runLinkedDenseCompositionValidation(request, base)
    }
    const trace =
      request.workload.denseModule === undefined
        ? await executeTassadarNumericModel(
            restoreTransitModel(request.workload.model),
            request.workload.steps,
          )
        : await executeTassadarDenseWeightModule(
            request.workload.denseModule as unknown as TassadarDenseWeightModule,
            request.workload.steps,
          )
    const { outputs, halted } = collectInterpreterOutputs(trace.stepOutputs)
    const matches = trace.traceDigest === request.claimedTraceDigest
    return {
      ...base,
      compositionVerification: null,
      halted,
      outcome: matches ? 'verified' : 'rejected',
      outputCount: outputs.length,
      rejectionReason: matches ? null : 'trace_digest_mismatch',
      replayedSteps: trace.stepCount,
      replayedTraceDigest: trace.traceDigest,
    }
  } catch (error: unknown) {
    if (
      error instanceof TassadarNumericExecutionError ||
      error instanceof TassadarDenseModuleError
    ) {
      return {
        ...base,
        compositionVerification: null,
        halted: false,
        outcome: 'rejected',
        outputCount: 0,
        rejectionReason: 'execution_refused',
        replayedSteps: 0,
        replayedTraceDigest: null,
      }
    }
    throw error
  }
}
