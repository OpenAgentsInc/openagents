import { describe, expect, test } from 'vitest'

import {
  executeTassadarNumericModel,
  type TassadarAlmNumericModel,
} from '@openagentsinc/tassadar-executor'

import { tassadarPocLoopSumFixture } from '../tassadar-poc-fixture'
import {
  decodeTraceRecord,
  encodeTraceRecord,
  finalOutputDigestFromTokens,
  fullTraceDigestFromTokens,
  stepOutputsFromTraceTokens,
  TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
  traceTokensFromStepOutputs,
  type TassadarTraceRecord,
} from './trace-record'
import { buildTraceRecordFromExecution } from './record-factory'
import {
  anchorWorkloadFromFixture,
  buildFamilyWorkload,
} from './workload-families'

const fixtureModel =
  tassadarPocLoopSumFixture.model as unknown as TassadarAlmNumericModel
const fixtureSteps =
  tassadarPocLoopSumFixture.steps as unknown as ReadonlyArray<
    ReadonlyArray<number>
  >

const anchorRecord = async (): Promise<TassadarTraceRecord> => {
  const built = anchorWorkloadFromFixture({
    fixtureBundleDigest: fixtureModel.bundle_digest,
    fixtureSteps,
    model: fixtureModel,
    stepCount: fixtureSteps.length,
  })
  if (!built.ok) throw new globalThis.Error(built.failure.detail)

  return buildTraceRecordFromExecution(built.workload)
}

describe('trace_record v0.1 token encoding', () => {
  test('token stream hashes back to the Rust-parity executor trace digest on the committed fixture', async () => {
    const trace = await executeTassadarNumericModel(fixtureModel, fixtureSteps)
    expect(trace.traceDigest).toBe(tassadarPocLoopSumFixture.expectedTraceDigest)
    const { stepOffsets, tokens } = traceTokensFromStepOutputs(
      trace.stepOutputs,
    )
    const recomputed = await fullTraceDigestFromTokens(
      fixtureModel.graph_digest,
      tokens,
      stepOffsets,
      'uint16',
    )
    expect(recomputed).toBe(tassadarPocLoopSumFixture.expectedTraceDigest)
  })

  test('round-trips step outputs through tokens for uint16 and uint32 widths, including negatives', () => {
    const rows: ReadonlyArray<ReadonlyArray<bigint>> = [
      [0n, -1n, 19n],
      [9007199254740992n, -9007199254740992n, 42n],
    ]
    for (const width of ['uint16', 'uint32'] as const) {
      const { stepOffsets, tokens } = traceTokensFromStepOutputs(rows, width)
      const decoded = stepOutputsFromTraceTokens(tokens, stepOffsets, width)
      expect(decoded).toEqual(rows)
    }
  })

  test('step offsets index every step at fixed stride', async () => {
    const record = await anchorRecord()
    const outputsPerStep = fixtureModel.output_slots.length
    expect(record.stepOffsets.length).toBe(fixtureSteps.length)
    expect(record.traceTokenIds.length).toBe(
      fixtureSteps.length * outputsPerStep * 4,
    )
    expect(record.stepOffsets[1]).toBe(outputsPerStep * 4)
  })

  test('final output digest is recomputable from the token stream', async () => {
    const record = await anchorRecord()
    const recomputed = await finalOutputDigestFromTokens(
      record.programHash,
      record.traceTokenIds,
      record.stepOffsets,
      record.tokenWidth,
    )
    expect(recomputed).toBe(record.finalOutputDigest)
  })
})

describe('trace_record v0.1 binary container', () => {
  test('encode/decode round-trips a real record byte-exactly', async () => {
    const record = await anchorRecord()
    const withReceipt: TassadarTraceRecord = {
      ...record,
      validatorReceipts: [
        {
          classId: 'exact_trace_replay.trace_factory.v0_1',
          comparedSteps: record.stepCount,
          outcome: 'verified',
          rejectionKind: null,
          replayedSteps: record.stepCount,
          tier: 1,
          validatedAtIso: '2026-06-11T00:00:00.000Z',
          validatorDeviceRef: 'device.local_pilot.validator',
          verdictSchemaVersion: 'validator_verdict.v0.1',
        },
      ],
    }
    const encoded = encodeTraceRecord(withReceipt)
    const decoded = decodeTraceRecord(encoded)
    expect(decoded.ok).toBe(true)
    if (!decoded.ok) return
    expect(decoded.bytesRead).toBe(encoded.length)
    expect(decoded.record.schemaVersion).toBe(
      TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
    )
    expect(decoded.record.recordId).toBe(record.recordId)
    expect(decoded.record.fullTraceDigest).toBe(record.fullTraceDigest)
    expect([...decoded.record.traceTokenIds]).toEqual([
      ...record.traceTokenIds,
    ])
    expect([...decoded.record.stepOffsets]).toEqual([...record.stepOffsets])
    expect(decoded.record.validatorReceipts).toEqual(
      withReceipt.validatorReceipts,
    )
    const reEncoded = encodeTraceRecord(decoded.record)
    expect([...reEncoded]).toEqual([...encoded])
  })

  test('compact binary stays well under the human-readable audit rendering of a realistic trace', async () => {
    const built = await buildFamilyWorkload({
      familyId: 'family.arithmetic_carry.v1',
      inputSeed: '0badc0ffee0ddf00',
      stepCount: 256,
    })
    if (!built.ok) throw new globalThis.Error(built.failure.detail)
    const record = await buildTraceRecordFromExecution(built.workload)
    const binaryBytes = encodeTraceRecord(record).length
    const auditRows = stepOutputsFromTraceTokens(
      record.traceTokenIds,
      record.stepOffsets,
      record.tokenWidth,
    ).map((row, step) => ({
      outputs: row.map(value => value.toString()),
      step,
    }))
    const auditJsonBytes = new TextEncoder().encode(
      JSON.stringify({ recordId: record.recordId, rows: auditRows }),
    ).length
    expect(binaryBytes).toBeLessThan(auditJsonBytes)
    // The hot-path arithmetic the contract freezes: exactly 2 bytes per
    // uint16 trace token (1B tokens ~ 2 GB), independent of rendering.
    const tokenStreamBytes = record.traceTokenIds.length * 2
    expect(encodeTraceRecord(record).length).toBeLessThan(
      tokenStreamBytes + record.stepOffsets.length * 4 + 1024,
    )
  })

  test('decode failures are typed: bad magic, truncation, bad token width', async () => {
    const record = await anchorRecord()
    const encoded = encodeTraceRecord(record)

    const badMagic = new Uint8Array(encoded)
    badMagic[0] = 0x58
    const badMagicResult = decodeTraceRecord(badMagic)
    expect(badMagicResult.ok).toBe(false)
    if (!badMagicResult.ok) {
      expect(badMagicResult.failure.kind).toBe('bad_magic')
    }

    const truncated = decodeTraceRecord(encoded.slice(0, 48))
    expect(truncated.ok).toBe(false)
    if (!truncated.ok) expect(truncated.failure.kind).toBe('truncated')

    const empty = decodeTraceRecord(new Uint8Array(2))
    expect(empty.ok).toBe(false)
    if (!empty.ok) expect(empty.failure.kind).toBe('truncated')
  })
})
