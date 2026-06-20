import { describe, expect, it } from 'vitest'

import {
  type Cs336A5DpoGradingChallengeSpec,
  buildCs336A5DpoGradingChallengeSpec,
} from './cs336-a5-dpo-grading-challenge'
import { runCs336A5DpoGradingChallengeLifecycle } from './cs336-a5-dpo-grading-lifecycle'

const trainingRunRef = 'run.cs336.a5.alignment.demo'

const buildSpec = (): Promise<Cs336A5DpoGradingChallengeSpec> =>
  buildCs336A5DpoGradingChallengeSpec({ splitRef: 'split_a' })

describe('CS336 A5 DPO grading challenge lifecycle', () => {
  it('drives an honest claim Queued -> Leased -> Verified through the real state machine', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec,
      trainingRunRef,
    })

    expect(result.verdict.state).toBe('Verified')
    expect(result.finalState).toBe('Verified')
    expect(result.records.queued.state).toBe('Queued')
    expect(result.records.leased.state).toBe('Leased')
    expect(result.records.finalized.state).toBe('Verified')
    expect(result.records.finalized.verifiedAt).not.toBeNull()
    expect(result.records.finalized.rejectedAt).toBeNull()
    expect(result.events.map(event => event.stateTo)).toEqual([
      'Queued',
      'Leased',
      'Verified',
    ])
    expect(result.events.map(event => event.transitionKind)).toEqual([
      'challenge_queued',
      'challenge_leased',
      'challenge_verified',
    ])
  })

  it('records the pair count from the recompute on a Verified verdict', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex, pairCount: spec.pairCount },
      spec,
      trainingRunRef,
    })

    expect(result.verdict.state).toBe('Verified')
    expect(result.verdict.publicDetails?.pairCount).toBe(spec.pairCount)
  })

  it('finalizes a forged claim to Rejected with a DigestMismatch', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: '0'.repeat(64) },
      spec,
      trainingRunRef,
    })

    expect(result.verdict.state).toBe('Rejected')
    expect(result.verdict.failureCodes).toContain('DigestMismatch')
    expect(result.finalState).toBe('Rejected')
    expect(result.records.finalized.failureCodes).toContain('DigestMismatch')
    expect(result.records.finalized.rejectedAt).not.toBeNull()
    expect(result.records.finalized.verifiedAt).toBeNull()
    expect(result.events.map(event => event.stateTo)).toEqual([
      'Queued',
      'Leased',
      'Rejected',
    ])
  })

  it('rejects a claim whose pair count disagrees with the recompute', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: {
        claimedDigestHex: spec.expectedDigestHex,
        pairCount: spec.pairCount + 1,
      },
      spec,
      trainingRunRef,
    })

    expect(result.verdict.state).toBe('Rejected')
    expect(result.verdict.failureCodes).toContain('DimensionMismatch')
    expect(result.finalState).toBe('Rejected')
  })

  it('carries the create-request through with a public-safe validatorRef and windowRef', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec,
      trainingRunRef,
      windowRef: 'window.cs336_a5_dpo_grading.split_a.0',
    })

    expect(result.createRequest.trainingRunRef).toBe(trainingRunRef)
    expect(result.createRequest.windowRef).toBe(
      'window.cs336_a5_dpo_grading.split_a.0',
    )
    expect(result.validatorRef).toBe('validator.cs336_a5_dpo_grading.split_a')
    expect(result.records.leased.leasedToRef).toBe(
      'validator.cs336_a5_dpo_grading.split_a',
    )
    expect(result.records.leased.windowRef).toBe(
      'window.cs336_a5_dpo_grading.split_a.0',
    )
  })

  it('is deterministic across repeated runs with the same inputs', async () => {
    const spec = await buildSpec()
    const first = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec,
      trainingRunRef,
    })
    const second = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec,
      trainingRunRef,
    })

    expect(second.challengeRef).toBe(first.challengeRef)
    expect(second.finalState).toBe(first.finalState)
    expect(second.events.map(event => event.id)).toEqual(
      first.events.map(event => event.id),
    )
    expect(second.records.finalized.verdictRefs).toEqual(
      first.records.finalized.verdictRefs,
    )
  })

  it('keeps the lifecycle result public-safe (no secrets or local paths)', async () => {
    const spec = await buildSpec()
    const result = await runCs336A5DpoGradingChallengeLifecycle({
      claim: { claimedDigestHex: spec.expectedDigestHex },
      spec,
      trainingRunRef,
    })

    const unsafe =
      /(mnemonic|preimage|bolt11|lnbc|sk-[a-z0-9]|\/Users\/|\/home\/|api[_-]?key|bearer)/i
    expect(unsafe.test(JSON.stringify(result))).toBe(false)
  })
})
