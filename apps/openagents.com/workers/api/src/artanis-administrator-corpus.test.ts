import { describe, expect, it } from 'vitest'

import { buildTassadarCorpusDispatchBody } from './artanis-administrator-tick'
import { runTassadarReplayValidation } from './tassadar-replay-validator'

const tassadarPayloadFrom = (
  body: Record<string, unknown>,
): Record<string, unknown> => {
  const codingAssignment = body.codingAssignment as Record<string, unknown>
  return codingAssignment.tassadar as Record<string, unknown>
}

describe('Artanis Tassadar corpus dispatch', () => {
  it('dispatches four distinct psionic-derived compiled programs by workload slot', async () => {
    const payloads = [0, 1, 2, 3].map(index =>
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
    ])
    expect(new Set(payloads.map(payload => payload.expectedTraceDigest)).size).toBe(4)
    expect(new Set(payloads.map(payload => payload.expectedModelDigest)).size).toBe(4)
    expect(payloads.map(payload => payload.expectedOutputs)).toEqual([
      [15],
      [47],
      [42],
      [24],
    ])
    expect(payloads.every(payload => payload.corpusDigest === payloads[0]?.corpusDigest)).toBe(true)
  })

  it('replay-verifies each selected corpus workload through the Worker validator', async () => {
    await Promise.all(
      [0, 1, 2, 3].map(async index => {
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
