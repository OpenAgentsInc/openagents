/**
 * Builds trace_record v0.1 artifacts from real executions of the TS
 * executor (issue #4748). This is the only sanctioned path from an
 * execution to a corpus record: the digests are computed FROM the
 * execution, never accepted from the assignment side (iron rule 2),
 * and a freshly built record carries no validator receipts — it is
 * quarantined until the tier ladder says otherwise (iron rule 3).
 */
import { executeTassadarNumericModel } from '@openagentsinc/tassadar-executor'

import {
  finalOutputDigestFromTokens,
  sha256HexOfText,
  TASSADAR_TRACE_PROFILE_VERSION,
  TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
  traceRecordIdFor,
  traceTokensFromStepOutputs,
  type TassadarTraceRecord,
  type TassadarTraceTokenWidth,
  type TassadarValidatorReceipt,
} from './trace-record'
import type { TassadarFamilyWorkload } from './workload-families'

export const TASSADAR_TS_EXECUTOR_ID = 'tassadar.alm_numeric_executor.ts.v1'
export const TASSADAR_TS_EXECUTOR_PACKAGE_VERSION =
  '@openagentsinc/tassadar-executor@0.1.0'

let executorHashMemo: string | null = null

/** Pinned executor hash carried by every record this factory produces. */
export const tassadarTsExecutorHash = async (): Promise<string> => {
  if (executorHashMemo === null) {
    executorHashMemo = await sha256HexOfText(
      `${TASSADAR_TS_EXECUTOR_ID}|${TASSADAR_TS_EXECUTOR_PACKAGE_VERSION}`,
    )
  }

  return executorHashMemo
}

/**
 * Executes the workload for real and mints the quarantined record. The
 * executor may refuse (exactness window, arity, missing key); refusals
 * propagate as the executor's own typed error and no record exists.
 */
export const buildTraceRecordFromExecution = async (
  workload: TassadarFamilyWorkload,
  tokenWidth: TassadarTraceTokenWidth = 'uint16',
): Promise<TassadarTraceRecord> => {
  const trace = await executeTassadarNumericModel(
    workload.model,
    workload.steps,
  )
  const { stepOffsets, tokens } = traceTokensFromStepOutputs(
    trace.stepOutputs,
    tokenWidth,
  )
  const finalOutputDigest = await finalOutputDigestFromTokens(
    workload.model.graph_digest,
    tokens,
    stepOffsets,
    tokenWidth,
  )
  const recordId = await traceRecordIdFor({
    inputSeed: workload.inputSeed,
    programHash: workload.model.graph_digest,
    stepCount: trace.stepCount,
  })
  const executorHash = await tassadarTsExecutorHash()
  const validatorReceipts: ReadonlyArray<TassadarValidatorReceipt> = []

  return {
    compilerHash: workload.compilerHash,
    executorHash,
    familyId: workload.familyId,
    finalOutputDigest,
    fullTraceDigest: trace.traceDigest,
    inputSeed: workload.inputSeed,
    profileVersion: TASSADAR_TRACE_PROFILE_VERSION,
    programHash: workload.model.graph_digest,
    recordId,
    schemaVersion: TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
    stepCount: trace.stepCount,
    stepOffsets,
    tokenWidth,
    traceTokenIds: tokens,
    validatorReceipts,
  }
}

/** Attaches verdict receipts without mutating the source record. */
export const withValidatorReceipts = (
  record: TassadarTraceRecord,
  receipts: ReadonlyArray<TassadarValidatorReceipt>,
): TassadarTraceRecord => ({
  ...record,
  validatorReceipts: [...record.validatorReceipts, ...receipts],
})
