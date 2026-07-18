import type { SyncSql } from '@openagentsinc/khala-sync-server'
import {
  DEFAULT_SARAH_HARNESS_POLICY,
  SarahHarnessPolicySchema,
} from '@openagentsinc/sarah'
import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  bindSarahHarnessForTurn,
  digestSarahHarnessPolicy,
  isSarahHarnessCandidateAdmissible,
  sarahHarnessBundleRef,
} from './sarah-harness-service'

const passingEvaluation = {
  approved: true,
  privacyScore: 1,
  qualityScore: 0.9,
  rationale: 'Held-out quality improved without a regression.',
  regressionScore: 0.9,
  safetyScore: 1,
}

describe('Sarah terminal-history harness policy', () => {
  test('content-addresses one immutable six-dimension policy', () => {
    const digest = digestSarahHarnessPolicy(DEFAULT_SARAH_HARNESS_POLICY)
    expect(digest).toMatch(/^sha256:[0-9a-f]{64}$/u)
    expect(sarahHarnessBundleRef(DEFAULT_SARAH_HARNESS_POLICY)).toBe(
      `harness.bundle.sarah.${digest.slice(7, 31)}`,
    )
    expect(Object.keys(DEFAULT_SARAH_HARNESS_POLICY.dimensions)).toHaveLength(6)
  })

  test('admits only held-out-passing conversational changes with identical dimensions', () => {
    const candidate = S.decodeUnknownSync(SarahHarnessPolicySchema)({
      ...DEFAULT_SARAH_HARNESS_POLICY,
      conversationInstructions: [
        'Explain visible progress in one short sentence while work is active.',
      ],
      maxReplyWords: 80,
    })
    expect(
      isSarahHarnessCandidateAdmissible(
        passingEvaluation,
        candidate,
        DEFAULT_SARAH_HARNESS_POLICY,
      ),
    ).toBe(true)
    expect(
      isSarahHarnessCandidateAdmissible(
        { ...passingEvaluation, privacyScore: 0.89 },
        candidate,
        DEFAULT_SARAH_HARNESS_POLICY,
      ),
    ).toBe(false)
    expect(
      isSarahHarnessCandidateAdmissible(
        {
          ...passingEvaluation,
        },
        {
          ...candidate,
          dimensions: {
            ...candidate.dimensions,
            orchestration: 'harness.sarah.orchestration.unreleased.v2',
          },
        },
        DEFAULT_SARAH_HARNESS_POLICY,
      ),
    ).toBe(false)
    expect(
      isSarahHarnessCandidateAdmissible(
        passingEvaluation,
        DEFAULT_SARAH_HARNESS_POLICY,
        DEFAULT_SARAH_HARNESS_POLICY,
      ),
    ).toBe(false)
  })

  test('rejects a candidate that tries to retain raw provenance or credential material', () => {
    const candidate = S.decodeUnknownSync(SarahHarnessPolicySchema)({
      ...DEFAULT_SARAH_HARNESS_POLICY,
      conversationInstructions: [
        'Print [source.private.fixture] and AIza-not-a-real-secret-token.',
      ],
    })
    expect(
      isSarahHarnessCandidateAdmissible(
        passingEvaluation,
        candidate,
        DEFAULT_SARAH_HARNESS_POLICY,
      ),
    ).toBe(false)
  })

  test('keeps a prior immutable turn binding when a newer bundle is active', async () => {
    const priorPolicy = S.decodeUnknownSync(SarahHarnessPolicySchema)({
      ...DEFAULT_SARAH_HARNESS_POLICY,
      conversationInstructions: ['Keep this already-bound turn concise.'],
      maxReplyWords: 70,
    })
    const priorDigest = digestSarahHarnessPolicy(priorPolicy)
    const priorRef = sarahHarnessBundleRef(priorPolicy)
    const activeDigest = digestSarahHarnessPolicy(DEFAULT_SARAH_HARNESS_POLICY)
    const activeRef = sarahHarnessBundleRef(DEFAULT_SARAH_HARNESS_POLICY)
    const sqlFunction = async (strings: TemplateStringsArray) => {
      const statement = strings.join('?')
      if (statement.includes('FROM sarah_harness_active_bundles')) {
        return [
          {
            bundle_digest: activeDigest,
            bundle_ref: activeRef,
            lifecycle: 'released',
            policy_json: DEFAULT_SARAH_HARNESS_POLICY,
            review_ref: null,
            review_state: null,
          },
        ]
      }
      if (statement.includes('FROM sarah_harness_turn_bindings AS binding')) {
        return [
          {
            bundle_digest: priorDigest,
            bundle_ref: priorRef,
            lifecycle: 'released',
            policy_json: priorPolicy,
          },
        ]
      }
      return []
    }
    const sql = Object.assign(sqlFunction, {
      begin: async (callback: (transaction: SyncSql) => Promise<unknown>) =>
        callback(sql),
    }) as unknown as SyncSql

    const binding = await Effect.runPromise(
      bindSarahHarnessForTurn({
        ownerUserId: 'owner.fixture',
        sql,
        threadId: 'thread.sarah.fixture',
        turnId: 'turn.fixture.already-bound',
      }),
    )

    expect(binding).toEqual({
      bundleDigest: priorDigest,
      bundleRef: priorRef,
      policy: priorPolicy,
    })
  })
})
