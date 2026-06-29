/**
 * W3 student-program data preparation (issue #4749).
 *
 * Converts the verified `corpus.tassadar_trace.v0_2.w3_100m` snapshot
 * into the `student_prep.v0.1` binary format consumed by the psionic
 * trainer (psionic crates/psionic-tassadar-student). For every record:
 *
 *   1. decode the TTRC record from its shard (Tier-0 container decode);
 *   2. enforce the W2 iron rule: training records must pass
 *      `trainingEligibility` (verified Tier 0 + full-replay receipt) —
 *      no unverified record ever reaches a trainer file;
 *   3. deterministically regenerate the workload (familyId, inputSeed)
 *      and assert the program hash matches the record;
 *   4. emit raw i64 input rows + verified i64 output rows (decoded from
 *      the verified token stream) plus seed writes, digests, and — for
 *      eval records — the numeric model JSON used by the Rust replay
 *      acceptance check.
 *
 * The student serialization protocol (preamble = seed writes, then per
 * step: input values then output values, all as 4xuint16 LE limbs) is
 * reconstructed limb-exactly on the Rust side; the corpus tokens are
 * exactly the output-row limbs of the verified records.
 *
 * Run: bun scripts/tassadar-w3-student-prep.ts
 */
import { mkdirSync, readFileSync, writeFileSync, appendFileSync } from 'node:fs'
import { join } from 'node:path'

import type { TassadarAlmNumericModel } from '@openagents/tassadar-executor'

import { tassadarPocLoopSumFixture } from '../src/tassadar-poc-fixture'
import {
  decodeTraceRecord,
  sha256HexOfBytes,
  stepOutputsFromTraceTokens,
  type TassadarTraceRecord,
} from '../src/tassadar-trace-factory/trace-record'
import { trainingEligibility } from '../src/tassadar-trace-factory/validation-policy'
import {
  anchorWorkloadFromFixture,
  buildFamilyWorkload,
  type TassadarFamilyWorkload,
} from '../src/tassadar-trace-factory/workload-families'

const CORPUS_DIR_NAME = 'tassadar-trace-corpus.v0_2.w3_100m'
const PREP_VERSION = 1
const PREP_MAGIC = 'TSPREP1\0'

const FAMILY_INDEX: Record<string, number> = {
  'family.arithmetic_carry.v1': 0,
  'family.memory_load_store.v1': 1,
  'family.branch_gated_control.v1': 2,
  'family.application_state_machine.v1': 3,
  'family.near_miss_lookup.v1': 4,
  'family.stack_loop_sum.compiled.v1': 5,
}

const SPLIT_INDEX: Record<string, number> = {
  train: 0,
  eval_heldout_family: 1,
  eval_long_horizon: 2,
  eval_adversarial: 3,
}

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
  trainingEligible: boolean
}>

class PrepWriter {
  private chunks: Array<Uint8Array> = []
  private path: string
  bytesWritten = 0
  recordCount = 0

  constructor(path: string) {
    this.path = path
    writeFileSync(path, new Uint8Array(0))
  }

  push(bytes: Uint8Array): void {
    this.chunks.push(bytes)
    this.bytesWritten += bytes.length
    if (this.chunksLength() > 16_000_000) this.flush()
  }

  private chunksLength(): number {
    let total = 0
    for (const chunk of this.chunks) total += chunk.length
    return total
  }

  flush(): void {
    if (this.chunks.length === 0) return
    const joined = new Uint8Array(this.chunksLength())
    let offset = 0
    for (const chunk of this.chunks) {
      joined.set(chunk, offset)
      offset += chunk.length
    }
    appendFileSync(this.path, joined)
    this.chunks = []
  }
}

const textBytes = (value: string): Uint8Array => new TextEncoder().encode(value)

class RecordEncoder {
  private parts: Array<Uint8Array> = []

  u8(value: number): void {
    this.parts.push(Uint8Array.of(value & 0xff))
  }

  u16(value: number): void {
    const bytes = new Uint8Array(2)
    new DataView(bytes.buffer).setUint16(0, value, true)
    this.parts.push(bytes)
  }

  u32(value: number): void {
    const bytes = new Uint8Array(4)
    new DataView(bytes.buffer).setUint32(0, value >>> 0, true)
    this.parts.push(bytes)
  }

  i64(value: bigint): void {
    const bytes = new Uint8Array(8)
    new DataView(bytes.buffer).setBigInt64(0, BigInt.asIntN(64, value), true)
    this.parts.push(bytes)
  }

  str(value: string): void {
    const encoded = textBytes(value)
    this.u32(encoded.length)
    this.parts.push(encoded)
  }

  finish(): Uint8Array {
    let total = 0
    for (const part of this.parts) total += part.length
    const joined = new Uint8Array(total)
    let offset = 0
    for (const part of this.parts) {
      joined.set(part, offset)
      offset += part.length
    }
    return joined
  }
}

const encodeRecord = (
  entry: RecordEntry,
  record: TassadarTraceRecord,
  workload: TassadarFamilyWorkload,
  includeModel: boolean,
): Uint8Array => {
  const familyIdx = FAMILY_INDEX[entry.familyId]
  const splitIdx = SPLIT_INDEX[entry.split]
  if (familyIdx === undefined || splitIdx === undefined) {
    throw new globalThis.Error(
      `unknown family/split for record ${entry.recordId}: ${entry.familyId} ${entry.split}`,
    )
  }
  const model = workload.model
  const outputs = stepOutputsFromTraceTokens(
    record.traceTokenIds,
    record.stepOffsets,
    record.tokenWidth,
  )
  const F = model.input_field_count
  const S = model.output_slots.length
  const enc = new RecordEncoder()
  enc.u8(familyIdx)
  enc.u8(splitIdx)
  enc.u16(0)
  enc.u32(record.stepCount)
  enc.u8(F)
  enc.u8(S)
  enc.u16(model.seed_writes.length)
  enc.str(entry.recordId)
  enc.str(entry.programHash)
  enc.str(entry.fullTraceDigest)
  enc.str(entry.finalOutputDigest)
  for (const [channel, key, value] of model.seed_writes) {
    enc.u32(channel)
    enc.i64(BigInt(key))
    enc.i64(BigInt(value))
  }
  for (const step of workload.steps) {
    if (step.length !== F) {
      throw new globalThis.Error(
        `record ${entry.recordId}: step arity ${step.length} != ${F}`,
      )
    }
    for (const field of step) enc.i64(BigInt(field))
  }
  if (outputs.length !== record.stepCount) {
    throw new globalThis.Error(
      `record ${entry.recordId}: decoded ${outputs.length} output rows, expected ${record.stepCount}`,
    )
  }
  for (const row of outputs) {
    if (row.length !== S) {
      throw new globalThis.Error(
        `record ${entry.recordId}: output row arity ${row.length} != ${S}`,
      )
    }
    for (const value of row) enc.i64(value)
  }
  if (includeModel) {
    enc.str(JSON.stringify(model))
  } else {
    enc.u32(0)
  }
  return enc.finish()
}

const main = async (): Promise<void> => {
  const corpusRoot = join(import.meta.dirname, '..', 'corpus')
  const dataDir = join(corpusRoot, CORPUS_DIR_NAME)
  const prepDir = join(dataDir, 'prep')
  mkdirSync(prepDir, { recursive: true })

  const manifest = JSON.parse(
    readFileSync(
      join(corpusRoot, `${CORPUS_DIR_NAME}.manifest.json`),
      'utf8',
    ),
  ) as {
    corpusId: string
    snapshotDigest: string
    executorHash: string
    records: { sha256: string }
  }
  const recordsText = readFileSync(join(dataDir, 'records.jsonl'), 'utf8')
  const recordsDigest = await sha256HexOfBytes(textBytes(recordsText))
  if (recordsDigest !== manifest.records.sha256) {
    throw new globalThis.Error(
      `records.jsonl digest ${recordsDigest} does not match manifest ${manifest.records.sha256}`,
    )
  }
  const entries: Array<RecordEntry> = recordsText
    .split('\n')
    .filter(line => line.length > 0)
    .map(line => JSON.parse(line) as RecordEntry)

  const shardBytes = new Map<string, Uint8Array>()
  const shardFor = (shard: string): Uint8Array => {
    const existing = shardBytes.get(shard)
    if (existing !== undefined) return existing
    const bytes = new Uint8Array(readFileSync(join(dataDir, shard)))
    shardBytes.set(shard, bytes)
    return bytes
  }

  const fixtureModel =
    tassadarPocLoopSumFixture.model as unknown as TassadarAlmNumericModel
  const fixtureSteps = tassadarPocLoopSumFixture.steps as unknown as ReadonlyArray<
    ReadonlyArray<number>
  >

  const header = (writerPath: string): PrepWriter => {
    const writer = new PrepWriter(writerPath)
    const enc = new RecordEncoder()
    enc.str(manifest.corpusId)
    enc.str(manifest.snapshotDigest)
    enc.str(manifest.executorHash)
    const head = new Uint8Array(textBytes(PREP_MAGIC).length + 4)
    head.set(textBytes(PREP_MAGIC), 0)
    new DataView(head.buffer).setUint32(head.length - 4, PREP_VERSION, true)
    writer.push(head)
    writer.push(enc.finish())
    return writer
  }

  const trainWriter = header(join(prepDir, 'train.tsprep'))
  const evalWriter = header(join(prepDir, 'eval.tsprep'))

  let trainTokens = 0
  let evalTokens = 0
  let processed = 0

  for (const entry of entries) {
    const bytes = shardFor(entry.shard)
    const decoded = decodeTraceRecord(bytes, entry.byteOffset)
    if (!decoded.ok) {
      throw new globalThis.Error(
        `record ${entry.recordId} failed to decode: ${JSON.stringify(decoded.failure)}`,
      )
    }
    const record = decoded.record
    if (record.recordId !== entry.recordId) {
      throw new globalThis.Error(
        `record id mismatch at offset ${entry.byteOffset}: ${record.recordId} != ${entry.recordId}`,
      )
    }
    const eligibility = trainingEligibility(record)
    if (entry.split === 'train' && !eligibility.eligible) {
      throw new globalThis.Error(
        `iron rule: train record ${entry.recordId} is not training-eligible: ${JSON.stringify(eligibility)}`,
      )
    }
    const built =
      entry.familyId === 'family.stack_loop_sum.compiled.v1'
        ? anchorWorkloadFromFixture({
            fixtureBundleDigest: fixtureModel.bundle_digest,
            fixtureSteps,
            model: fixtureModel,
            stepCount: record.stepCount,
          })
        : await buildFamilyWorkload({
            familyId: entry.familyId,
            inputSeed: entry.inputSeed,
            stepCount: record.stepCount,
          })
    if (!built.ok) {
      throw new globalThis.Error(
        `workload rebuild failed for ${entry.recordId}: ${JSON.stringify(built.failure)}`,
      )
    }
    if (built.workload.model.graph_digest !== record.programHash) {
      throw new globalThis.Error(
        `program hash mismatch for ${entry.recordId}: rebuilt ${built.workload.model.graph_digest} != ${record.programHash}`,
      )
    }
    const isTrain = entry.split === 'train'
    const encoded = encodeRecord(entry, record, built.workload, !isTrain)
    if (isTrain) {
      trainWriter.push(encoded)
      trainWriter.recordCount += 1
      trainTokens += record.traceTokenIds.length
    } else {
      evalWriter.push(encoded)
      evalWriter.recordCount += 1
      evalTokens += record.traceTokenIds.length
    }
    processed += 1
    if (processed % 2000 === 0) {
      console.log(`processed ${processed}/${entries.length} records`)
    }
  }
  trainWriter.flush()
  evalWriter.flush()

  const trainDigest = await sha256HexOfBytes(
    new Uint8Array(readFileSync(join(prepDir, 'train.tsprep'))),
  )
  const evalDigest = await sha256HexOfBytes(
    new Uint8Array(readFileSync(join(prepDir, 'eval.tsprep'))),
  )
  const prepManifest = {
    corpusId: manifest.corpusId,
    eval: {
      bytes: evalWriter.bytesWritten,
      file: 'prep/eval.tsprep',
      records: evalWriter.recordCount,
      sha256: evalDigest,
      tokens: evalTokens,
    },
    executorHash: manifest.executorHash,
    prepFormatVersion: `student_prep.v0.${PREP_VERSION}`,
    snapshotDigest: manifest.snapshotDigest,
    train: {
      bytes: trainWriter.bytesWritten,
      file: 'prep/train.tsprep',
      records: trainWriter.recordCount,
      sha256: trainDigest,
      tokens: trainTokens,
    },
  }
  writeFileSync(
    join(prepDir, 'prep-manifest.json'),
    `${JSON.stringify(prepManifest, null, 2)}\n`,
  )
  console.log(JSON.stringify(prepManifest, null, 2))
}

await main()
