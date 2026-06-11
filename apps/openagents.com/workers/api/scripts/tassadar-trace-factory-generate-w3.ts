/**
 * Tassadar verified trace factory — W3 100M-token corpus snapshot
 * (issue #4749, RESEARCH_PLAN.md W3; scale stage of the W2 factory from
 * issue #4748).
 *
 * Generates `corpus.tassadar_trace.v0_2.w3_100m`: a seed-sharded,
 * deterministic, fully verified ~100M-token corpus over the six frozen
 * v0.1 workload families, with every record passing Tier 0 (schema/
 * hash) AND Tier 1 (full independent replay). Sampled admission is NOT
 * used: the W2 iron rule (`trainingEligibility`) requires a verified
 * full-replay receipt (tier 1 or 3) on every training record, so this
 * snapshot runs full replay on all records.
 *
 * Outputs:
 *   corpus/tassadar-trace-corpus.v0_2.w3_100m.manifest.json   (tracked)
 *   corpus/tassadar-trace-corpus.v0_2.w3_100m/shards/*.ttrc   (untracked)
 *   corpus/tassadar-trace-corpus.v0_2.w3_100m/records.jsonl   (untracked,
 *     digest pinned in the manifest)
 *   corpus/tassadar-trace-corpus.v0_2.w3_100m/verdicts.jsonl  (untracked)
 *
 * Run: bun scripts/tassadar-trace-factory-generate-w3.ts
 */
import { appendFileSync, mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import type { TassadarAlmNumericModel } from '@openagents/tassadar-executor'

import { tassadarPocLoopSumFixture } from '../src/tassadar-poc-fixture'
import {
  buildTraceRecordFromExecution,
  tassadarTsExecutorHash,
  withValidatorReceipts,
} from '../src/tassadar-trace-factory/record-factory'
import {
  encodeTraceRecord,
  sha256HexOfBytes,
  sha256HexOfText,
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
  trainingEligibility,
  type TassadarValidationContext,
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
  anchorWorkloadFromFixture,
  buildFamilyWorkload,
  deriveRecordSeed,
  TASSADAR_FAMILY_BUILDER_VERSION,
  type TassadarFamilyWorkload,
} from '../src/tassadar-trace-factory/workload-families'

export const TASSADAR_W3_CORPUS_ID = 'corpus.tassadar_trace.v0_2.w3_100m'
export const TASSADAR_W3_CORPUS_DIR = 'tassadar-trace-corpus.v0_2.w3_100m'
export const TASSADAR_W3_CORPUS_MANIFEST_VERSION = 'corpus_manifest.v0.1'
/** Frozen W3 master seed; record seeds derive deterministically. */
export const TASSADAR_W3_CORPUS_MASTER_SEED = '4749facade202606'

const TRAIN_STEP_CYCLE = [128, 256, 384, 512] as const
/** 2x / 4x / 8x of the split policy's trainMaxSteps (512). */
const LONG_STEP_CYCLE = [1024, 2048, 4096] as const

type W3Plan = Readonly<{
  familyId: string
  tag: string
  recordCount: number
  stepCycle: ReadonlyArray<number>
  /** Disjoint per-plan index base keeps record seeds disjoint by split. */
  indexBase: number
}>

/**
 * ~103.6M tokens total: 85.5M train (arithmetic/memory/branch at <=512
 * steps), the rest eval (held-out economic family, length extrapolation
 * at 1024/2048/4096, near-miss adversaries, compiled anchor prefixes).
 */
const W3_PLANS: ReadonlyArray<W3Plan> = [
  { familyId: 'family.arithmetic_carry.v1', indexBase: 0, recordCount: 4400, stepCycle: TRAIN_STEP_CYCLE, tag: 'train' },
  { familyId: 'family.arithmetic_carry.v1', indexBase: 1_000_000, recordCount: 60, stepCycle: LONG_STEP_CYCLE, tag: 'long' },
  { familyId: 'family.memory_load_store.v1', indexBase: 0, recordCount: 4000, stepCycle: TRAIN_STEP_CYCLE, tag: 'train' },
  { familyId: 'family.memory_load_store.v1', indexBase: 1_000_000, recordCount: 60, stepCycle: LONG_STEP_CYCLE, tag: 'long' },
  { familyId: 'family.branch_gated_control.v1', indexBase: 0, recordCount: 3400, stepCycle: TRAIN_STEP_CYCLE, tag: 'train' },
  { familyId: 'family.branch_gated_control.v1', indexBase: 1_000_000, recordCount: 60, stepCycle: LONG_STEP_CYCLE, tag: 'long' },
  { familyId: 'family.application_state_machine.v1', indexBase: 0, recordCount: 256, stepCycle: TRAIN_STEP_CYCLE, tag: 'short' },
  { familyId: 'family.application_state_machine.v1', indexBase: 1_000_000, recordCount: 66, stepCycle: LONG_STEP_CYCLE, tag: 'long' },
  { familyId: 'family.near_miss_lookup.v1', indexBase: 0, recordCount: 192, stepCycle: TRAIN_STEP_CYCLE, tag: 'short' },
  { familyId: 'family.near_miss_lookup.v1', indexBase: 1_000_000, recordCount: 48, stepCycle: LONG_STEP_CYCLE, tag: 'long' },
]

const ANCHOR_PREFIXES = [80, 72, 64, 48, 32, 16] as const

const W3_WORKER_STANDING: TassadarWorkerStanding = {
  isNewFamily: true,
  isNewProfile: true,
  isNewWorker: true,
  verifiedRecordCount: 0,
  workerRef: 'worker.w3_100m.ts_executor',
}

const VALIDATOR_DEVICE_REF = 'device.w3_100m.replay_validator'

type RecordEntry = Readonly<{
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
  trainingEligible: boolean
}>

type TypedFailure = Readonly<{
  recordId: string
  familyId: string
  stage: string
  kind: string
  detail: string
}>

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

const main = async (): Promise<void> => {
  const corpusRoot = join(import.meta.dirname, '..', 'corpus')
  const dataDir = join(corpusRoot, TASSADAR_W3_CORPUS_DIR)
  const shardDir = join(dataDir, 'shards')
  mkdirSync(shardDir, { recursive: true })
  const recordsPath = join(dataDir, 'records.jsonl')
  const verdictsPath = join(dataDir, 'verdicts.jsonl')
  writeFileSync(recordsPath, '')
  writeFileSync(verdictsPath, '')

  const startedAtIso = new Date().toISOString()
  const startedMs = performance.now()
  const executorHash = await tassadarTsExecutorHash()
  const fixtureModel =
    tassadarPocLoopSumFixture.model as unknown as TassadarAlmNumericModel
  const fixtureSteps = tassadarPocLoopSumFixture.steps as unknown as ReadonlyArray<
    ReadonlyArray<number>
  >

  const failures: Array<TypedFailure> = []
  const shardSummaries: Array<{
    familyId: string
    tag: string
    shard: string
    recordCount: number
    tokenCount: number
    shardBytes: number
    shardSha256: string
    compilerHash: string
  }> = []
  const splitTokenTotals = new Map<string, number>()
  const splitRecordTotals = new Map<string, number>()

  let totalTokens = 0
  let totalRecords = 0
  let tier0Verified = 0
  let tier1Verified = 0
  let attempted = 0
  let closedTicks = 0
  let trainingEligibleRecords = 0
  let recordsJsonlText = ''

  const processWorkload = async (
    workload: TassadarFamilyWorkload,
    shardChunks: Array<Uint8Array>,
    shardName: string,
    shardOffset: { value: number },
  ): Promise<void> => {
    attempted += 1
    const assignmentPayload = {
      familyId: workload.familyId,
      inputSeed: workload.inputSeed,
      jobKind: 'tassadar_trace_factory_generate_w3',
      stepCount: workload.steps.length,
    }
    const banViolations = generationAssignmentDigestViolations(assignmentPayload)
    if (banViolations.length > 0) {
      throw new globalThis.Error(
        `generation assignment digest ban violated: ${JSON.stringify(banViolations)}`,
      )
    }
    const record = await buildTraceRecordFromExecution(workload)
    const context: TassadarValidationContext = {
      validatedAtIso: new Date().toISOString(),
      validatorDeviceRef: VALIDATOR_DEVICE_REF,
    }
    const tierZero = await runTierZeroValidation(record, context)
    if (tierZero.outcome !== 'verified') {
      appendFileSync(verdictsPath, `${JSON.stringify(tierZero)}\n`)
      failures.push({
        detail: tierZero.rejection?.detail ?? 'unspecified',
        familyId: record.familyId,
        kind: tierZero.rejection?.kind ?? 'unspecified',
        recordId: record.recordId,
        stage: 'tier0_schema_hash',
      })

      return
    }
    tier0Verified += 1
    const tierOne = await runTierOneFullReplay(record, workload, context)
    if (tierOne.outcome !== 'verified') {
      appendFileSync(
        verdictsPath,
        `${JSON.stringify(tierZero)}\n${JSON.stringify(tierOne)}\n`,
      )
      failures.push({
        detail: tierOne.rejection?.detail ?? 'unspecified',
        familyId: record.familyId,
        kind: tierOne.rejection?.kind ?? 'unspecified',
        recordId: record.recordId,
        stage: 'tier1_full_replay',
      })

      return
    }
    tier1Verified += 1
    const verified: TassadarTraceRecord = withValidatorReceipts(record, [
      receiptFromVerdict(tierZero),
      receiptFromVerdict(tierOne),
    ])
    const admission = admissionDecision(verified, W3_WORKER_STANDING)
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
        assignmentRef: `assignment.trace_factory_w3.${record.recordId}`,
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
    const eligibility = trainingEligibility(verified)
    if (eligibility.eligible) trainingEligibleRecords += 1
    const encoded = encodeTraceRecord(verified)
    const split = splitAssignmentForRecord(record)
    const entry: RecordEntry = {
      byteLength: encoded.length,
      byteOffset: shardOffset.value,
      familyId: record.familyId,
      finalOutputDigest: record.finalOutputDigest,
      fullTraceDigest: record.fullTraceDigest,
      inputSeed: record.inputSeed,
      programHash: record.programHash,
      recordId: record.recordId,
      shard: shardName,
      split,
      stepCount: record.stepCount,
      tokenCount: record.traceTokenIds.length,
      trainingEligible: eligibility.eligible,
      trainingRecordRef: trainingRef.ok ? trainingRef.trainingRecordRef : null,
    }
    recordsJsonlText += `${JSON.stringify(entry)}\n`
    if (recordsJsonlText.length > 4_000_000) {
      appendFileSync(recordsPath, recordsJsonlText)
      recordsJsonlText = ''
    }
    shardChunks.push(encoded)
    shardOffset.value += encoded.length
    totalTokens += record.traceTokenIds.length
    totalRecords += 1
    splitTokenTotals.set(
      split,
      (splitTokenTotals.get(split) ?? 0) + record.traceTokenIds.length,
    )
    splitRecordTotals.set(split, (splitRecordTotals.get(split) ?? 0) + 1)
  }

  for (const plan of W3_PLANS) {
    const shardName = `shards/${plan.familyId.replace(/\./g, '_')}.${plan.tag}.ttrc`
    const shardChunks: Array<Uint8Array> = []
    const shardOffset = { value: 0 }
    const planStartTokens = totalTokens
    const planStartRecords = totalRecords
    let compilerHash = ''
    for (let index = 0; index < plan.recordCount; index += 1) {
      const inputSeed = await deriveRecordSeed(
        TASSADAR_W3_CORPUS_MASTER_SEED,
        plan.familyId,
        plan.indexBase + index,
      )
      const stepCount = plan.stepCycle[index % plan.stepCycle.length] ?? 256
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
          recordId: `unbuilt.${plan.familyId}.${plan.indexBase + index}`,
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
    shardSummaries.push({
      compilerHash,
      familyId: plan.familyId,
      recordCount: totalRecords - planStartRecords,
      shard: shardName,
      shardBytes: shardBytes.length,
      shardSha256: await sha256HexOfBytes(shardBytes),
      tag: plan.tag,
      tokenCount: totalTokens - planStartTokens,
    })
    console.log(
      `${plan.familyId} [${plan.tag}]: ${totalRecords - planStartRecords} records, ${totalTokens - planStartTokens} tokens (${((performance.now() - startedMs) / 1000).toFixed(1)}s elapsed)`,
    )
  }

  // The psionic-compiled anchor family: executed prefixes of the
  // committed loop-sum fixture, pinning Rust/TS digest parity.
  {
    const shardName = 'shards/family_stack_loop_sum_compiled_v1.anchor.ttrc'
    const shardChunks: Array<Uint8Array> = []
    const shardOffset = { value: 0 }
    const planStartTokens = totalTokens
    const planStartRecords = totalRecords
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
    shardSummaries.push({
      compilerHash: fixtureModel.bundle_digest,
      familyId: 'family.stack_loop_sum.compiled.v1',
      recordCount: totalRecords - planStartRecords,
      shard: shardName,
      shardBytes: shardBytes.length,
      shardSha256: await sha256HexOfBytes(shardBytes),
      tag: 'anchor',
      tokenCount: totalTokens - planStartTokens,
    })
  }

  appendFileSync(recordsPath, recordsJsonlText)
  recordsJsonlText = ''
  const recordsJsonlDigest = await sha256HexOfText(
    await Bun.file(recordsPath).text(),
  )
  const shardDigestLines = shardSummaries
    .map(summary => `${summary.shard}:${summary.shardSha256}`)
    .sort()
  const snapshotDigest = await sha256HexOfText(
    `${TASSADAR_W3_CORPUS_ID}|${shardDigestLines.join('|')}|records:${recordsJsonlDigest}`,
  )

  const schemaValidRate = attempted === 0 ? 0 : tier0Verified / attempted
  const fullReplayPassRate =
    tier0Verified === 0 ? 0 : tier1Verified / tier0Verified
  const elapsedSeconds = (performance.now() - startedMs) / 1000

  const manifest = {
    anchorFixture: {
      expectedTraceDigest: tassadarPocLoopSumFixture.expectedTraceDigest,
      fixtureId: tassadarPocLoopSumFixture.fixtureId,
      modelDigest: fixtureModel.graph_digest,
    },
    corpusId: TASSADAR_W3_CORPUS_ID,
    executorHash,
    familyBuilderVersion: TASSADAR_FAMILY_BUILDER_VERSION,
    generatedAtIso: startedAtIso,
    generationSeconds: Number(elapsedSeconds.toFixed(1)),
    manifestVersion: TASSADAR_W3_CORPUS_MANIFEST_VERSION,
    masterSeed: TASSADAR_W3_CORPUS_MASTER_SEED,
    profileVersion: TASSADAR_TRACE_PROFILE_VERSION,
    records: {
      jsonl: 'records.jsonl',
      recordCount: totalRecords,
      sha256: recordsJsonlDigest,
    },
    shards: shardSummaries,
    snapshotDigest,
    splitPolicyVersion: TASSADAR_TRAINING_SPLIT_POLICY_V0_1.policyVersion,
    splits: Object.fromEntries(
      [...splitTokenTotals.keys()].sort().map(split => [
        split,
        {
          records: splitRecordTotals.get(split) ?? 0,
          tokens: splitTokenTotals.get(split) ?? 0,
        },
      ]),
    ),
    tokenEncodingVersion: TASSADAR_TRACE_TOKEN_ENCODING_VERSION,
    totals: {
      records: totalRecords,
      tokens: totalTokens,
      trainingEligibleRecords,
    },
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
  }
  writeFileSync(
    join(corpusRoot, `${TASSADAR_W3_CORPUS_DIR}.manifest.json`),
    `${JSON.stringify(manifest, null, 2)}\n`,
  )

  console.log('')
  console.log(`corpus: ${TASSADAR_W3_CORPUS_ID}`)
  console.log(`records admitted: ${totalRecords}/${attempted}`)
  console.log(`tokens: ${totalTokens}`)
  console.log(`training-eligible records: ${trainingEligibleRecords}`)
  console.log(`closed ticks: ${closedTicks}`)
  console.log(`schema-valid rate: ${(schemaValidRate * 100).toFixed(3)}%`)
  console.log(`full-replay pass rate: ${(fullReplayPassRate * 100).toFixed(3)}%`)
  console.log(`snapshot digest: ${snapshotDigest}`)
  console.log(`generation seconds: ${elapsedSeconds.toFixed(1)}`)
  console.log(`typed failures: ${failures.length}`)
  for (const failure of failures.slice(0, 20)) {
    console.log(`  ${failure.stage}/${failure.kind}: ${failure.recordId}`)
  }
  if (failures.length > 0) process.exitCode = 1
}

await main()
