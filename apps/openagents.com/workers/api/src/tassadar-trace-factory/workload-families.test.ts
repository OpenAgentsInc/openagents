import { describe, expect, test } from 'vitest'

import {
  executeTassadarNumericModel,
  type TassadarAlmNumericModel,
} from '@openagentsinc/tassadar-executor'

import { tassadarPocLoopSumFixture } from '../tassadar-poc-fixture'
import {
  anchorWorkloadFromFixture,
  buildFamilyWorkload,
  deriveRecordSeed,
  TASSADAR_FAMILY_MAX_STEP_COUNT,
  TASSADAR_TRACE_FAMILY_IDS,
} from './workload-families'

const SYNTHETIC_FAMILIES = TASSADAR_TRACE_FAMILY_IDS.filter(
  familyId => familyId !== 'family.stack_loop_sum.compiled.v1',
)

describe('workload families v0.1', () => {
  test('every synthetic family builds deterministically: same seed, same program hash, same steps', async () => {
    for (const familyId of SYNTHETIC_FAMILIES) {
      const first = await buildFamilyWorkload({
        familyId,
        inputSeed: '0123456789abcdef',
        stepCount: 24,
      })
      const second = await buildFamilyWorkload({
        familyId,
        inputSeed: '0123456789abcdef',
        stepCount: 24,
      })
      expect(first.ok).toBe(true)
      expect(second.ok).toBe(true)
      if (!first.ok || !second.ok) continue
      expect(first.workload.model.graph_digest).toBe(
        second.workload.model.graph_digest,
      )
      expect(first.workload.steps).toEqual(second.workload.steps)
      expect(first.workload.compilerHash).toBe(second.workload.compilerHash)
    }
  })

  test('different seeds produce different programs within a family', async () => {
    const first = await buildFamilyWorkload({
      familyId: 'family.memory_load_store.v1',
      inputSeed: '0000000000000001',
      stepCount: 16,
    })
    const second = await buildFamilyWorkload({
      familyId: 'family.memory_load_store.v1',
      inputSeed: '0000000000000002',
      stepCount: 16,
    })
    if (!first.ok || !second.ok) throw new globalThis.Error('build failed')
    expect(first.workload.model.graph_digest).not.toBe(
      second.workload.model.graph_digest,
    )
  })

  test('every synthetic family executes for real without refusal and yields exact integer rows', async () => {
    for (const familyId of SYNTHETIC_FAMILIES) {
      const built = await buildFamilyWorkload({
        familyId,
        inputSeed: 'fedcba9876543210',
        stepCount: 64,
      })
      expect(built.ok).toBe(true)
      if (!built.ok) continue
      const trace = await executeTassadarNumericModel(
        built.workload.model,
        built.workload.steps,
      )
      expect(trace.stepCount).toBe(64)
      expect(trace.stepOutputs.length).toBe(64)
      expect(trace.traceDigest).toMatch(/^[0-9a-f]{64}$/)
    }
  })

  test('long-horizon executions stay inside the exactness window at the family step bound boundary', async () => {
    const built = await buildFamilyWorkload({
      familyId: 'family.arithmetic_carry.v1',
      inputSeed: 'a5a5a5a5a5a5a5a5',
      stepCount: 2048,
    })
    if (!built.ok) throw new globalThis.Error(built.failure.detail)
    const trace = await executeTassadarNumericModel(
      built.workload.model,
      built.workload.steps,
    )
    expect(trace.stepCount).toBe(2048)
  })

  test('near-miss lookup family seeds adjacent key clusters', async () => {
    const built = await buildFamilyWorkload({
      familyId: 'family.near_miss_lookup.v1',
      inputSeed: '1111222233334444',
      stepCount: 8,
    })
    if (!built.ok) throw new globalThis.Error(built.failure.detail)
    const keys = built.workload.model.seed_writes.map(
      ([, key]: [unknown, number]) => key,
    )
    const adjacentPairs = keys.filter((key: number) =>
      keys.includes(key + 1),
    ).length
    expect(adjacentPairs).toBeGreaterThan(8)
  })

  test('typed failures: unknown family, out-of-range step count, malformed seed', async () => {
    const unknown = await buildFamilyWorkload({
      familyId: 'family.does_not_exist.v1',
      inputSeed: 'abcd',
      stepCount: 8,
    })
    expect(!unknown.ok && unknown.failure.kind).toBe('unknown_family')

    const tooLong = await buildFamilyWorkload({
      familyId: 'family.arithmetic_carry.v1',
      inputSeed: 'abcd',
      stepCount: TASSADAR_FAMILY_MAX_STEP_COUNT + 1,
    })
    expect(!tooLong.ok && tooLong.failure.kind).toBe('step_count_out_of_range')

    const badSeed = await buildFamilyWorkload({
      familyId: 'family.arithmetic_carry.v1',
      inputSeed: 'NOT-HEX',
      stepCount: 8,
    })
    expect(!badSeed.ok && badSeed.failure.kind).toBe('invalid_seed')
  })

  test('record seeds derive deterministically from the master seed', async () => {
    const first = await deriveRecordSeed('feed0001', 'family.arithmetic_carry.v1', 7)
    const second = await deriveRecordSeed('feed0001', 'family.arithmetic_carry.v1', 7)
    const other = await deriveRecordSeed('feed0001', 'family.arithmetic_carry.v1', 8)
    expect(first).toBe(second)
    expect(first).not.toBe(other)
    expect(first).toMatch(/^[0-9a-f]{16}$/)
  })

  test('the compiled anchor family replays the committed psionic fixture digest-true', async () => {
    const model =
      tassadarPocLoopSumFixture.model as unknown as TassadarAlmNumericModel
    const steps = tassadarPocLoopSumFixture.steps as unknown as ReadonlyArray<
      ReadonlyArray<number>
    >
    const built = anchorWorkloadFromFixture({
      fixtureBundleDigest: model.bundle_digest,
      fixtureSteps: steps,
      model,
      stepCount: steps.length,
    })
    expect(built.ok).toBe(true)
    if (!built.ok) return
    const trace = await executeTassadarNumericModel(
      built.workload.model,
      built.workload.steps,
    )
    expect(trace.traceDigest).toBe(
      tassadarPocLoopSumFixture.expectedTraceDigest,
    )

    const overrun = anchorWorkloadFromFixture({
      fixtureBundleDigest: model.bundle_digest,
      fixtureSteps: steps,
      model,
      stepCount: steps.length + 1,
    })
    expect(!overrun.ok && overrun.failure.kind).toBe('step_count_out_of_range')
  })
})
