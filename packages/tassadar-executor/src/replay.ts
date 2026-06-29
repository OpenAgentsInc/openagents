/**
 * Exact trace-replay verdicts for numeric-model executions, mirroring
 * psionic's `exact_trace_replay.alm_compiled.v1` semantics (psionic
 * docs/TASSADAR_ALM_TRACE_REPLAY.md): deterministic re-execution and
 * bitwise comparison. Verdicts inform acceptance; they move no money and
 * create no serving claim.
 */
import {
  type TassadarAlmNumericModel,
  TassadarNumericExecutionError,
  executeTassadarNumericModel,
} from "./numeric-executor.js"

export const TASSADAR_TS_REPLAY_CLASS_ID = "exact_trace_replay.alm_numeric_ts.v1"

export type TassadarReplayRejection =
  | Readonly<{ reason: "model_digest_mismatch"; actual: string }>
  | Readonly<{ reason: "trace_digest_mismatch"; actual: string }>
  | Readonly<{ reason: "row_mismatch"; step: number }>
  | Readonly<{ reason: "execution_refused"; detail: string }>

export type TassadarReplayVerdict = Readonly<{
  classId: typeof TASSADAR_TS_REPLAY_CLASS_ID
  outcome: "verified" | "rejected"
  rejection: TassadarReplayRejection | null
  replayedSteps: number
  comparedSteps: number
  graphDigest: string
  claimedTraceDigest: string
  replayedTraceDigest: string | null
  validatorDeviceRef: string
}>

export const verifyTassadarFullReplay = async (
  input: Readonly<{
    model: TassadarAlmNumericModel
    steps: ReadonlyArray<ReadonlyArray<number>>
    claimedTraceDigest: string
    validatorDeviceRef: string
  }>,
): Promise<TassadarReplayVerdict> => {
  const base = {
    claimedTraceDigest: input.claimedTraceDigest,
    classId: TASSADAR_TS_REPLAY_CLASS_ID,
    graphDigest: input.model.graph_digest,
    validatorDeviceRef: input.validatorDeviceRef,
  } as const
  try {
    const trace = await executeTassadarNumericModel(input.model, input.steps)
    if (trace.traceDigest === input.claimedTraceDigest) {
      return {
        ...base,
        comparedSteps: trace.stepCount,
        outcome: "verified",
        rejection: null,
        replayedSteps: trace.stepCount,
        replayedTraceDigest: trace.traceDigest,
      }
    }
    return {
      ...base,
      comparedSteps: trace.stepCount,
      outcome: "rejected",
      rejection: { actual: trace.traceDigest, reason: "trace_digest_mismatch" },
      replayedSteps: trace.stepCount,
      replayedTraceDigest: trace.traceDigest,
    }
  } catch (error: unknown) {
    if (error instanceof TassadarNumericExecutionError) {
      const refused: TassadarNumericExecutionError = error
      return {
        ...base,
        comparedSteps: 0,
        outcome: "rejected",
        rejection: { detail: refused.message, reason: "execution_refused" },
        replayedSteps: 0,
        replayedTraceDigest: null,
      }
    }
    throw error
  }
}

/** Window spot-check: replay and diff claimed rows, naming the exact first mismatching step. */
export const verifyTassadarWindow = async (
  input: Readonly<{
    model: TassadarAlmNumericModel
    steps: ReadonlyArray<ReadonlyArray<number>>
    windowStart: number
    claimedRows: ReadonlyArray<ReadonlyArray<bigint>>
    validatorDeviceRef: string
  }>,
): Promise<TassadarReplayVerdict> => {
  const base = {
    claimedTraceDigest: "",
    classId: TASSADAR_TS_REPLAY_CLASS_ID,
    graphDigest: input.model.graph_digest,
    validatorDeviceRef: input.validatorDeviceRef,
  } as const
  const trace = await executeTassadarNumericModel(input.model, input.steps)
  for (let offset = 0; offset < input.claimedRows.length; offset += 1) {
    const step = input.windowStart + offset
    const actual = trace.stepOutputs[step]
    const claimed = input.claimedRows[offset]
    const matches =
      actual !== undefined &&
      claimed !== undefined &&
      actual.length === claimed.length &&
      actual.every((value: bigint, column: number) => value === claimed[column])
    if (!matches) {
      return {
        ...base,
        comparedSteps: input.claimedRows.length,
        outcome: "rejected",
        rejection: { reason: "row_mismatch", step },
        replayedSteps: trace.stepCount,
        replayedTraceDigest: trace.traceDigest,
      }
    }
  }
  return {
    ...base,
    comparedSteps: input.claimedRows.length,
    outcome: "verified",
    rejection: null,
    replayedSteps: trace.stepCount,
    replayedTraceDigest: trace.traceDigest,
  }
}
