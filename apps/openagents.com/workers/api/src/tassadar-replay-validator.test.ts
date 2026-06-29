import { readFileSync } from 'node:fs'

import { describe, expect, test } from 'vitest'

import {
  TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
  tassadarLinkedDenseProgramFixture,
  type TassadarLinkedDenseProgramFixture,
} from '@openagentsinc/tassadar-executor/linked-dense-module'

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
const cloneLinkedFixture = (): TassadarLinkedDenseProgramFixture =>
  JSON.parse(JSON.stringify(tassadarLinkedDenseProgramFixture))

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

  test('verifies a linked dense fixture only after composition verification clears', async () => {
    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_linked_dense.test',
      claimedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
      pylonDeviceRef: 'device.pylon.constructor.test',
      workload: {
        linkedDenseFixture: tassadarLinkedDenseProgramFixture as Record<
          string,
          unknown
        >,
        model: {},
        steps: tassadarLinkedDenseProgramFixture.steps,
      },
    })

    expect(verdict.outcome).toBe('verified')
    expect(verdict.rejectionReason).toBeNull()
    expect(verdict.replayedTraceDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
    )
    expect(verdict.compositionVerification?.compositionVerificationCleared).toBe(
      true,
    )
    expect(verdict.compositionVerification?.linkCompatibility.verified).toBe(
      true,
    )
    expect(
      verdict.compositionVerification?.constituentVerifications.every(
        item => item.verified,
      ),
    ).toBe(true)
  })

  test('rejects linked dense replay when link compatibility is tampered even if the composed digest matches', async () => {
    const tampered = cloneLinkedFixture()
    ;(
      tampered.linkedModule.linkResolution.dependency_graph.nodes[0] as {
        compatibility_digest: string
      }
    ).compatibility_digest = '0'.repeat(64)

    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_linked_dense.test',
      claimedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
      pylonDeviceRef: 'device.pylon.constructor.test',
      workload: {
        linkedDenseFixture: tampered as Record<string, unknown>,
        model: {},
        steps: tampered.steps,
      },
    })

    expect(verdict.replayedTraceDigest).toBe(
      TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
    )
    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejectionReason).toBe('composition_verification_failed')
    expect(verdict.compositionVerification?.linkCompatibility.verified).toBe(
      false,
    )
    expect(verdict.compositionVerification?.blockerRefs).toContain(
      'blocker.public.tassadar_compiled_module.link_compatibility_digest_mismatch',
    )
  })

  test('rejects linked dense replay when a constituent source verification is tampered', async () => {
    const tampered = cloneLinkedFixture()
    ;(tampered.linkedModule.banks[0] as { expectedTraceDigest: string })
      .expectedTraceDigest = '0'.repeat(64)

    const verdict = await runTassadarReplayValidation({
      assignmentRef: 'assignment.tassadar_linked_dense.test',
      claimedTraceDigest: TASSADAR_ALM_LINKED_DENSE_COMPOSED_TRACE_DIGEST,
      pylonDeviceRef: 'device.pylon.constructor.test',
      workload: {
        linkedDenseFixture: tampered as Record<string, unknown>,
        model: {},
        steps: tampered.steps,
      },
    })

    expect(verdict.outcome).toBe('rejected')
    expect(verdict.rejectionReason).toBe('composition_verification_failed')
    expect(
      verdict.compositionVerification?.constituentVerifications[0]?.verified,
    ).toBe(false)
    expect(verdict.compositionVerification?.blockerRefs).toContain(
      'blocker.public.tassadar_compiled_module.source_trace_mismatch',
    )
  })
})
