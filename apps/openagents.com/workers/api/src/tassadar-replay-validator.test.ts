import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

import { runTassadarReplayValidation } from './tassadar-replay-validator'

const fixture = JSON.parse(
  readFileSync(
    new URL(
      '../../../../../packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json',
      import.meta.url,
    ),
    'utf8',
  ),
)

const { seed_writes, ...rest } = fixture.model
const transitModel = { ...rest, initialChannelWrites: seed_writes }

describe('tassadar replay validator (worker as separate device)', () => {
  test('verifies an honest claimed digest by re-executing the workload', async () => {
    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_poc.test',
      claimedTraceDigest: fixture.expectedTraceDigest,
      pylonDeviceRef: 'device.pylon.test',
      workload: { model: transitModel, steps: fixture.steps },
    })
    expect(verdict.outcome).toBe('verified')
    expect(verdict.replayedTraceDigest).toBe(fixture.expectedTraceDigest)
    expect(verdict.validatorDeviceRef).toBe('device.cloudflare_worker.openagents_api')
    expect(verdict.halted).toBe(true)
  })

  test('rejects a tampered claimed digest with the replayed truth attached', async () => {
    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_poc.test',
      claimedTraceDigest: 'tampered',
      pylonDeviceRef: 'device.pylon.test',
      workload: { model: transitModel, steps: fixture.steps },
    })
    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejectionReason).toBe('trace_digest_mismatch')
    expect(verdict.replayedTraceDigest).toBe(fixture.expectedTraceDigest)
  })

  test('typed execution refusals reject without a replayed digest', async () => {
    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_poc.test',
      claimedTraceDigest: fixture.expectedTraceDigest,
      pylonDeviceRef: 'device.pylon.test',
      workload: { model: transitModel, steps: [[0, 1]] },
    })
    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejectionReason).toBe('execution_refused')
  })
})
