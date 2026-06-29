import { describe, expect, it } from 'vitest'

import {
  Cs336A3DefaultCellsPerBudget,
  Cs336A3DefaultSweepBudgetsFlops,
  Cs336A3FlopsPerParameterDataUnit,
  planCs336A3SweepGrid,
  runCs336A3SweepCell,
} from './cs336-a3-sweep-workload'
import {
  buildTrainingVerificationChallengeRecord,
  runTrainingVerificationClass,
} from './training-verification'

describe('cs336 a3 sweep workload', () => {
  it('plans the IsoFLOP grid with geometric N spacing and D = C / (6 N) per realized cell', () => {
    const cells = planCs336A3SweepGrid()

    expect(cells).toHaveLength(
      Cs336A3DefaultSweepBudgetsFlops.length * Cs336A3DefaultCellsPerBudget,
    )

    for (const cell of cells) {
      expect(cell.parameterCount).toBeGreaterThan(0)
      expect(cell.tokenCount).toBeGreaterThan(0)
      expect(
        (Cs336A3FlopsPerParameterDataUnit *
          cell.parameterCount *
          cell.tokenCount) /
          cell.computeBudgetFlops,
      ).toBeCloseTo(1, 2)
    }

    const firstBudget = cells.filter(cell => cell.budgetIndex === 0)

    for (let index = 1; index < firstBudget.length; index += 1) {
      expect(firstBudget[index]!.parameterCount).toBeGreaterThan(
        firstBudget[index - 1]!.parameterCount,
      )
    }
  })

  it('refuses invalid planner bounds', () => {
    expect(() => planCs336A3SweepGrid({ budgetsFlops: [] })).toThrow()
    expect(() => planCs336A3SweepGrid({ cellsPerBudget: 2 })).toThrow()
    expect(() =>
      planCs336A3SweepGrid({ parametersMax: 10, parametersMin: 20 }),
    ).toThrow()
  })

  it('trains a cell deterministically: identical re-runs produce identical losses and digests', async () => {
    const first = await runCs336A3SweepCell({
      computeBudgetFlops: 4_000_000,
      rank: 2,
    })
    const second = await runCs336A3SweepCell({
      computeBudgetFlops: 4_000_000,
      rank: 2,
    })

    expect(first.finalLoss).toBe(second.finalLoss)
    expect(first.outputDigestHex).toBe(second.outputDigestHex)
    expect(first.finalLoss).toBeLessThan(first.initialLoss)
    expect(Number.isFinite(first.finalLoss)).toBe(true)
  })

  it('feeds the deterministic_recompute class: matching digests verify and tampered digests reject', async () => {
    const result = await runCs336A3SweepCell({
      computeBudgetFlops: 4_000_000,
      rank: 2,
    })
    const recomputed = await runCs336A3SweepCell({
      computeBudgetFlops: 4_000_000,
      rank: 2,
    })
    const challenge = (payload: Record<string, unknown>) =>
      buildChallenge(payload).challenge
    const buildChallenge = (payload: Record<string, unknown>) =>
      buildTrainingVerificationChallengeRecord({
        makeId: () => 'a3-cell-challenge',
        nowIso: '2026-06-11T08:00:00.000Z',
        request: {
          commitmentRefs: [
            `commitment.cs336_a3.cell.sha256_${result.outputDigestHex.slice(0, 16)}`,
          ],
          contributionRef: 'contribution.cs336_a3.assignment.test.sweep',
          homeworkKind: 'admin_dispatched_homework',
          payload,
          trainingRunRef: 'run.cs336.a3.scaling_sweep.demo',
          verificationClass: 'deterministic_recompute',
        },
      })

    const verified = await runTrainingVerificationClass({
      challenge: challenge({
        expectedDigestRef: `digest.cs336_a3.${result.outputDigestHex}`,
        recomputedDigestRef: `digest.cs336_a3.${recomputed.outputDigestHex}`,
      }),
    })

    expect(verified.state).toBe('Verified')
    expect(verified.failureCodes).toHaveLength(0)

    const tampered = await runTrainingVerificationClass({
      challenge: challenge({
        expectedDigestRef: `digest.cs336_a3.${result.outputDigestHex}`,
        recomputedDigestRef: 'digest.cs336_a3.tampered',
      }),
    })

    expect(tampered.state).toBe('Rejected')
    expect(tampered.failureCodes).toContain('DigestMismatch')
  })
})
