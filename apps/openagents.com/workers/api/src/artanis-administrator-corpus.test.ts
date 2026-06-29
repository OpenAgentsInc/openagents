import { describe, expect, it } from 'vitest'

import { tassadarDenseWeightModuleTraceDigest } from '@openagentsinc/tassadar-executor/dense-weight-module'

import { buildTassadarCorpusDispatchBody } from './artanis-administrator-tick'
import { runTassadarReplayValidation } from './tassadar-replay-validator'

const tassadarPayloadFrom = (
  body: Record<string, unknown>,
): Record<string, unknown> => {
  const codingAssignment = body.codingAssignment as Record<string, unknown>
  return codingAssignment.tassadar as Record<string, unknown>
}

describe('Artanis Tassadar corpus dispatch', () => {
  it('dispatches five distinct psionic-derived compiled programs by workload slot', async () => {
    const payloads = [0, 1, 2, 3, 4].map(index =>
      tassadarPayloadFrom(
        buildTassadarCorpusDispatchBody({
          assignmentRef: `assignment.artanis_admin.2026061801000${index}.w${index}`,
          pylonRef: 'pylon.test.executor',
        }),
      ),
    )

    expect(payloads.map(payload => payload.programId)).toEqual([
      'tassadar_corpus.loop_sum_v1',
      'tassadar_corpus.mul_add_v1',
      'tassadar_corpus.memory_roundtrip_v1',
      'tassadar_corpus.factorial_loop_v1',
      'tassadar_corpus.w1_1_window_v1',
    ])
    expect(new Set(payloads.map(payload => payload.expectedTraceDigest)).size).toBe(5)
    expect(new Set(payloads.map(payload => payload.expectedModelDigest)).size).toBe(5)
    expect(payloads.map(payload => payload.expectedOutputs)).toEqual([
      [15],
      [47],
      [42],
      [24],
      [1],
    ])
    expect(payloads[0]?.modelArtifactKind).toBe(
      'tassadar_alm_dense_weight_module.v1',
    )
    expect(payloads[0]?.denseModuleDigest).toBe(
      'cfda0fe5dcf42e16db9e18696731427f0f30915fd3100d38da2dcc8411433e2c',
    )
    expect(payloads[0]?.expectedTraceDigest).toBe(
      tassadarDenseWeightModuleTraceDigest,
    )
    expect(payloads.slice(1).map(payload => payload.modelArtifactKind)).toEqual([
      'tassadar_alm_numeric_model.v1',
      'tassadar_alm_numeric_model.v1',
      'tassadar_alm_numeric_model.v1',
      'tassadar_alm_numeric_model.v1',
    ])
    expect(payloads.every(payload => payload.corpusDigest === payloads[0]?.corpusDigest)).toBe(true)
  })

  it('replay-verifies each selected corpus workload through the Worker validator', async () => {
    await Promise.all(
      [0, 1, 2, 3, 4].map(async index => {
        const assignmentRef = `assignment.artanis_admin.2026061802000${index}.w${index}`
        const payload = tassadarPayloadFrom(
          buildTassadarCorpusDispatchBody({
            assignmentRef,
            pylonRef: 'pylon.test.executor',
          }),
        )

        const verdict = await runTassadarReplayValidation({
          assignmentRef,
          claimedTraceDigest: String(payload.expectedTraceDigest),
          pylonDeviceRef: 'device.pylon.test',
          workload: {
            ...(payload.denseModule === undefined
              ? {}
              : { denseModule: payload.denseModule as Record<string, unknown> }),
            model: payload.model as Record<string, unknown>,
            steps: payload.steps as ReadonlyArray<ReadonlyArray<number>>,
          },
        })

        expect(verdict.outcome).toBe('verified')
        expect(verdict.replayedTraceDigest).toBe(payload.expectedTraceDigest)
        expect(verdict.outputCount).toBe(1)
      }),
    )
  })
})
