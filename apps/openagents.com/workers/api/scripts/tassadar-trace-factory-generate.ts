/**
 * Tassadar verified trace factory — local pilot corpus generation
 * (issue #4748, RESEARCH_PLAN.md W2 "1-5M tokens locally").
 *
 * Generates the corpus.tassadar_trace.v0_1 local pilot with the REAL TS
 * executor over the frozen v0.1 workload families, validates every
 * record through Tier 0 (schema/hash) and Tier 1 (full independent
 * replay), and writes:
 *
 *   corpus/tassadar-trace-corpus.v0_1.manifest.json   (tracked)
 *   corpus/tassadar-trace-corpus.v0_1/shards/*.ttrc   (untracked)
 *   corpus/tassadar-trace-corpus.v0_1/verdicts.jsonl  (untracked)
 *
 * Iron rules hold here too: records are minted FROM execution, receipts
 * come from the tier ladder, and nothing is written to a shard while
 * unverified — quarantined/rejected records land in the typed failure
 * breakdown instead.
 *
 * Run: bun scripts/tassadar-trace-factory-generate.ts
 */
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import type { TassadarAlmNumericModel } from '@openagentsinc/tassadar-executor'

import { tassadarPocLoopSumFixture } from '../src/tassadar-poc-fixture'
import {
  buildTraceRecordFromExecution,
  tassadarTsExecutorHash,
  withValidatorReceipts,
} from '../src/tassadar-trace-factory/record-factory'
import {
  encodeTraceRecord,
  sha256HexOfBytes,
  TASSADAR_TRACE_PROFILE_VERSION,
  TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
  TASSADAR_TRACE_TOKEN_ENCODING_VERSION,
  type TassadarTraceRecord,
} from '../src/tassadar-trace-factory/trace-record'
import {
  admissionDecision,
  generationAssignmentDigestViolations,
  receiptFromVerdict,
  runTierOneFullReplay,
  runTierZeroValidation,
  type TassadarValidationContext,
  type TassadarValidatorVerdict,
  type TassadarWorkerStanding,
} from '../src/tassadar-trace-factory/validation-policy'
import {
  splitAssignmentForRecord,
  TASSADAR_TRAINING_SPLIT_POLICY_V0_1,
} from '../src/tassadar-trace-factory/training-split-policy'
import {
  closeTick,
  trainingRecordRefFromClosedTick,
} from '../src/tassadar-trace-factory/tick-closure'
import {
  rebuildFactoryProjection,
  type TassadarFactoryProjectionEvent,
} from '../src/tassadar-trace-factory/projection-rebuild'
import {
  anchorWorkloadFromFixture,
  buildFamilyWorkload,
  deriveRecordSeed,
  TASSADAR_FAMILY_BUILDER_VERSION,
  type TassadarFamilyWorkload,
} from '../src/tassadar-trace-factory/workload-families'

export const TASSADAR_CORPUS_ID = 'corpus.tassadar_trace.v0_1.local_pilot'
export const TASSADAR_CORPUS_MANIFEST_VERSION = 'corpus_manifest.v0.1'
/** Frozen pilot master seed; record seeds derive deterministically. */
export const TASSADAR_CORPUS_MASTER_SEED = '4748c0de20260611'

const TRAIN_STEP_CYCLE = [128, 256, 384, 512] as const
const EVAL_LONG_STEP_CYCLE = [1024, 2048] as const

type FamilyPlan = Readonly<{
  familyId: string
  trainRecords: number
  evalLongRecords: number
}>

/** ~3.5M tokens across all six families; see the evidence doc math. */
const FAMILY_PLANS: ReadonlyArray<FamilyPlan> = [
  { evalLongRecords: 16, familyId: 'family.arithmetic_carry.v1', trainRecords: 96 },
  { evalLongRecords: 12, familyId: 'family.memory_load_store.v1', trainRecords: 64 },
  { evalLongRecords: 8, familyId: 'family.branch_gated_control.v1', trainRecords: 48 },
  {
    evalLongRecords: 8,
    familyId: 'family.application_state_machine.v1',
    trainRecords: 32,
  },
  { evalLongRecords: 4, familyId: 'family.near_miss_lookup.v1', trainRecords: 20 },
]

const ANCHOR_PREFIXES = [80, 72, 64, 48, 32, 16] as const

const PILOT_WORKER_STANDING: TassadarWorkerStanding = {
  isNewFamily: true,
  isNewProfile: true,
  isNewWorker: true,
  verifiedRecordCount: 0,
  workerRef: 'worker.local_pilot.ts_executor',
}

const VALIDATOR_DEVICE_REF = 'device.local_pilot.replay_validator'

type ManifestRecordEntry = Readonly<{
  recordId: string
  familyId: string
  inputSeed: string
  stepCount: number
  tokenCount: number
  programHash: string
  fullTraceDigest: string
  finalOutputDigest: string
  split: string
  shard: string
  byteOffset: number
  byteLength: number
  trainingRecordRef: string | null
}>

type TypedFailure = Readonly<{
  recordId: string
  familyId: string
  stage: string
  kind: string
  detail: string
}>

const main = async (): Promise<void> => {
  const corpusRoot = join(import.meta.dirname, '..', 'corpus')
  const dataDir = join(corpusRoot, 'tassadar-trace-corpus.v0_1')
  const shardDir = join(dataDir, 'shards')
  mkdirSync(shardDir, { recursive: true })

  const startedAtIso = new Date().toISOString()
  const executorHash = await tassadarTsExecutorHash()
  const fixtureModel =
    tassadarPocLoopSumFixture.model as unknown as TassadarAlmNumericModel
  const fixtureSteps = tassadarPocLoopSumFixture.steps as unknown as ReadonlyArray<
    ReadonlyArray<number>
  >

  const records: Array<ManifestRecordEntry> = []
  const failures: Array<TypedFailure> = []
  const verdictLog: Array<TassadarValidatorVerdict> = []
  const projectionEvents: Array<TassadarFactoryProjectionEvent> = []
  const familySummaries: Array<{
    familyId: string
    compilerHash: string
    recordCount: number
    tokenCount: number
    shard: string
    shardSha256: string
    shardBytes: number
  }> = []

  let totalTokens = 0
  let tier0Verified = 0
  let tier1Verified = 0
  let attempted = 0
  let closedTicks = 0

  const processWorkload = async (
    workload: TassadarFamilyWorkload,
    shardChunks: Array<Uint8Array>,
    shardName: string,
    shardOffset: { value: number },
  ): Promise<void> => {
    attempted += 1
    // Iron rule 2 — the generation-side assignment payload carries no
    // expected digests; assert it on the payload we would dispatch.
    const assignmentPayload = {
      familyId: workload.familyId,
      inputSeed: workload.inputSeed,
      jobKind: 'tassadar_trace_factory_generate',
      stepCount: workload.steps.length,
    }
    const banViolations = generationAssignmentDigestViolations(assignmentPayload)
    if (banViolations.length > 0) {
      throw new globalThis.Error(
        `generation assignment digest ban violated: ${JSON.stringify(banViolations)}`,
      )
    }
    const record = await buildTraceRecordFromExecution(workload)
    const registeredAtIso = new Date().toISOString()
    projectionEvents.push({
      familyId: record.familyId,
      kind: 'record_registered',
      occurredAtIso: registeredAtIso,
      recordId: record.recordId,
      tokenCount: record.traceTokenIds.length,
    })
    const context: TassadarValidationContext = {
      validatedAtIso: new Date().toISOString(),
      validatorDeviceRef: VALIDATOR_DEVICE_REF,
    }
    const tierZero = await runTierZeroValidation(record, context)
    verdictLog.push(tierZero)
    if (tierZero.outcome !== 'verified') {
      failures.push({
        detail: tierZero.rejection?.detail ?? 'unspecified',
        familyId: record.familyId,
        kind: tierZero.rejection?.kind ?? 'unspecified',
        recordId: record.recordId,
        stage: 'tier0_schema_hash',
      })
      projectionEvents.push({
        familyId: record.familyId,
        fromStatus: 'quarantined',
        kind: 'validation_transition',
        occurredAtIso: context.validatedAtIso,
        recordId: record.recordId,
        tokenCount: record.traceTokenIds.length,
        toStatus: 'rejected',
        verdictRef: `verdict.${record.recordId}.tier0`,
      })

      return
    }
    tier0Verified += 1
    const tierOne = await runTierOneFullReplay(record, workload, context)
    verdictLog.push(tierOne)
    if (tierOne.outcome !== 'verified') {
      failures.push({
        detail: tierOne.rejection?.detail ?? 'unspecified',
        familyId: record.familyId,
        kind: tierOne.rejection?.kind ?? 'unspecified',
        recordId: record.recordId,
        stage: 'tier1_full_replay',
      })
      projectionEvents.push({
        familyId: record.familyId,
        fromStatus: 'quarantined',
        kind: 'validation_transition',
        occurredAtIso: context.validatedAtIso,
        recordId: record.recordId,
        tokenCount: record.traceTokenIds.length,
        toStatus: 'rejected',
        verdictRef: `verdict.${record.recordId}.tier1`,
      })

      return
    }
    tier1Verified += 1
    const verified: TassadarTraceRecord = withValidatorReceipts(record, [
      receiptFromVerdict(tierZero),
      receiptFromVerdict(tierOne),
    ])
    const admission = admissionDecision(verified, PILOT_WORKER_STANDING)
    if (admission.status !== 'admitted') {
      failures.push({
        detail: JSON.stringify(admission),
        familyId: record.familyId,
        kind: `admission_${admission.status}`,
        recordId: record.recordId,
        stage: 'admission',
      })

      return
    }
    projectionEvents.push({
      familyId: record.familyId,
      fromStatus: 'quarantined',
      kind: 'validation_transition',
      occurredAtIso: context.validatedAtIso,
      recordId: record.recordId,
      tokenCount: record.traceTokenIds.length,
      toStatus: 'verified',
      verdictRef: `verdict.${record.recordId}.tier1`,
    })
    const tick = closeTick({
      evaluation: {
        outcome: 'verified',
        tier: 1,
        verdictRef: `verdict.${record.recordId}.tier1`,
      },
      execution: {
        executorHash: record.executorHash,
        fullTraceDigest: record.fullTraceDigest,
        stepCount: record.stepCount,
      },
      intent: {
        assignmentRef: `assignment.trace_factory.${record.recordId}`,
        declaredStepCount: workload.steps.length,
        familyId: record.familyId,
        inputSeed: record.inputSeed,
      },
      stateDelta: {
        admittedTo: 'corpus',
        recordId: record.recordId,
        tokenCount: record.traceTokenIds.length,
      },
    })
    if (!tick.closed) {
      failures.push({
        detail: `open faces: ${tick.openFaces.join(',')}`,
        familyId: record.familyId,
        kind: 'tick_not_closed',
        recordId: record.recordId,
        stage: 'tick_closure',
      })

      return
    }
    closedTicks += 1
    const trainingRef = trainingRecordRefFromClosedTick(tick.tick)
    const encoded = encodeTraceRecord(verified)
    records.push({
      byteLength: encoded.length,
      byteOffset: shardOffset.value,
      familyId: record.familyId,
      finalOutputDigest: record.finalOutputDigest,
      fullTraceDigest: record.fullTraceDigest,
      inputSeed: record.inputSeed,
      programHash: record.programHash,
      recordId: record.recordId,
      shard: shardName,
      split: splitAssignmentForRecord(record),
      stepCount: record.stepCount,
      tokenCount: record.traceTokenIds.length,
      trainingRecordRef: trainingRef.ok ? trainingRef.trainingRecordRef : null,
    })
    shardChunks.push(encoded)
    shardOffset.value += encoded.length
    totalTokens += record.traceTokenIds.length
  }

  for (const plan of FAMILY_PLANS) {
    const shardName = `shards/${plan.familyId.replace(/\./g, '_')}.ttrc`
    const shardChunks: Array<Uint8Array> = []
    const shardOffset = { value: 0 }
    const familyStartTokens = totalTokens
    const familyStartRecords = records.length
    let compilerHash = ''
    const recordCount = plan.trainRecords + plan.evalLongRecords
    for (let index = 0; index < recordCount; index += 1) {
      const inputSeed = await deriveRecordSeed(
        TASSADAR_CORPUS_MASTER_SEED,
        plan.familyId,
        index,
      )
      const stepCount =
        index < plan.trainRecords
          ? (TRAIN_STEP_CYCLE[index % TRAIN_STEP_CYCLE.length] ?? 256)
          : (EVAL_LONG_STEP_CYCLE[
              (index - plan.trainRecords) % EVAL_LONG_STEP_CYCLE.length
            ] ?? 1024)
      const built = await buildFamilyWorkload({
        familyId: plan.familyId,
        inputSeed,
        stepCount,
      })
      if (!built.ok) {
        failures.push({
          detail: built.failure.detail,
          familyId: plan.familyId,
          kind: built.failure.kind,
          recordId: `unbuilt.${plan.familyId}.${index}`,
          stage: 'workload_build',
        })
        attempted += 1
        continue
      }
      compilerHash = built.workload.compilerHash
      await processWorkload(built.workload, shardChunks, shardName, shardOffset)
    }
    const shardBytes = concat(shardChunks)
    writeFileSync(join(dataDir, shardName), shardBytes)
    familySummaries.push({
      compilerHash,
      familyId: plan.familyId,
      recordCount: records.length - familyStartRecords,
      shard: shardName,
      shardBytes: shardBytes.length,
      shardSha256: await sha256HexOfBytes(shardBytes),
      tokenCount: totalTokens - familyStartTokens,
    })
    console.log(
      `${plan.familyId}: ${records.length - familyStartRecords} records, ${totalTokens - familyStartTokens} tokens`,
    )
  }

  // The psionic-compiled anchor family: executed prefixes of the
  // committed loop-sum fixture, pinning Rust/TS digest parity.
  {
    const shardName = 'shards/family_stack_loop_sum_compiled_v1.ttrc'
    const shardChunks: Array<Uint8Array> = []
    const shardOffset = { value: 0 }
    const familyStartTokens = totalTokens
    const familyStartRecords = records.length
    for (const prefix of ANCHOR_PREFIXES) {
      const built = anchorWorkloadFromFixture({
        fixtureBundleDigest: fixtureModel.bundle_digest,
        fixtureSteps,
        model: fixtureModel,
        stepCount: prefix,
      })
      if (!built.ok) {
        failures.push({
          detail: built.failure.detail,
          familyId: 'family.stack_loop_sum.compiled.v1',
          kind: built.failure.kind,
          recordId: `unbuilt.anchor.${prefix}`,
          stage: 'workload_build',
        })
        attempted += 1
        continue
      }
      await processWorkload(built.workload, shardChunks, shardName, shardOffset)
    }
    const shardBytes = concat(shardChunks)
    writeFileSync(join(dataDir, shardName), shardBytes)
    familySummaries.push({
      compilerHash: fixtureModel.bundle_digest,
      familyId: 'family.stack_loop_sum.compiled.v1',
      recordCount: records.length - familyStartRecords,
      shard: shardName,
      shardBytes: shardBytes.length,
      shardSha256: await sha256HexOfBytes(shardBytes),
      tokenCount: totalTokens - familyStartTokens,
    })
    console.log(
      `family.stack_loop_sum.compiled.v1: ${records.length - familyStartRecords} records, ${totalTokens - familyStartTokens} tokens`,
    )
  }

  const projection = rebuildFactoryProjection(projectionEvents)
  const schemaValidRate = attempted === 0 ? 0 : tier0Verified / attempted
  const fullReplayPassRate = tier0Verified === 0 ? 0 : tier1Verified / tier0Verified

  const manifest = {
    anchorFixture: {
      expectedTraceDigest: tassadarPocLoopSumFixture.expectedTraceDigest,
      fixtureId: tassadarPocLoopSumFixture.fixtureId,
      modelDigest: fixtureModel.graph_digest,
    },
    corpusId: TASSADAR_CORPUS_ID,
    executorHash,
    familyBuilderVersion: TASSADAR_FAMILY_BUILDER_VERSION,
    families: familySummaries,
    generatedAtIso: startedAtIso,
    manifestVersion: TASSADAR_CORPUS_MANIFEST_VERSION,
    masterSeed: TASSADAR_CORPUS_MASTER_SEED,
    profileVersion: TASSADAR_TRACE_PROFILE_VERSION,
    projectionAfterGeneration: projection,
    records,
    splitPolicyVersion: TASSADAR_TRAINING_SPLIT_POLICY_V0_1.policyVersion,
    tokenEncodingVersion: TASSADAR_TRACE_TOKEN_ENCODING_VERSION,
    traceRecordSchemaVersion: TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
    validation: {
      attempted,
      closedTicks,
      failures,
      fullReplayPassRate,
      schemaValidRate,
      tier0Verified,
      tier1Verified,
      validatorDeviceRef: VALIDATOR_DEVICE_REF,
    },
    totals: {
      families: familySummaries.length,
      records: records.length,
      tokens: totalTokens,
    },
  }
  writeFileSync(
    join(corpusRoot, 'tassadar-trace-corpus.v0_1.manifest.json'),
    `${JSON.stringify(manifest, null, 2)}\n`,
  )
  writeFileSync(
    join(dataDir, 'verdicts.jsonl'),
    `${verdictLog.map(verdict => JSON.stringify(verdict)).join('\n')}\n`,
  )

  console.log('')
  console.log(`corpus: ${TASSADAR_CORPUS_ID}`)
  console.log(`records admitted: ${records.length}/${attempted}`)
  console.log(`tokens: ${totalTokens}`)
  console.log(`closed ticks: ${closedTicks}`)
  console.log(`schema-valid rate (tier0/attempted): ${(schemaValidRate * 100).toFixed(3)}%`)
  console.log(
    `full-replay pass rate (tier1/tier0-verified): ${(fullReplayPassRate * 100).toFixed(3)}%`,
  )
  console.log(`typed failures: ${failures.length}`)
  for (const failure of failures) {
    console.log(`  ${failure.stage}/${failure.kind}: ${failure.recordId}`)
  }
  console.log(
    `projection (validation transitions only): verified=${projection.verifiedRecords} tokens=${projection.verifiedTokens} rate=${projection.validationRate}`,
  )
  if (failures.length > 0) process.exitCode = 1
}

const concat = (chunks: ReadonlyArray<Uint8Array>): Uint8Array => {
  let total = 0
  for (const chunk of chunks) total += chunk.length
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }

  return joined
}

await main()
