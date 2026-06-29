import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_CONFORMANCE_FIXTURES,
  FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_READ_ONLY_AUTHORITY,
  ForumAcceptedContributionBridgeProjection,
  ForumAcceptedContributionBridgeRecord,
  ForumAcceptedContributionBridgeUnsafe,
  forumAcceptedContributionBridgeCanMutatePayout,
  forumAcceptedContributionBridgeHasNoMutationAuthority,
  forumAcceptedContributionBridgeProjectionHasPrivateMaterial,
  projectForumAcceptedContributionBridge,
} from './accepted-contribution-proof-bridge'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from '../redaction-regression-fixtures'

const nowIso = '2026-06-07T12:20:00.000Z'

const bridgeRecord = (
  index: number,
  overrides: Partial<ForumAcceptedContributionBridgeRecord> = {},
): ForumAcceptedContributionBridgeRecord =>
  S.decodeUnknownSync(ForumAcceptedContributionBridgeRecord)({
    ...FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_CONFORMANCE_FIXTURES[index]!,
    ...overrides,
  })

describe('Forum accepted contribution proof bridge', () => {
  test('keeps ordinary Forum rewards out of accepted-work payout claims', () => {
    const record = bridgeRecord(0)
    const projection = projectForumAcceptedContributionBridge(
      record,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(ForumAcceptedContributionBridgeRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(ForumAcceptedContributionBridgeProjection)(
      projection,
    )).toEqual(projection)
    expect(forumAcceptedContributionBridgeHasNoMutationAuthority(
      record.authority,
    )).toBe(true)
    expect(forumAcceptedContributionBridgeCanMutatePayout(record)).toBe(false)
    expect(projection.contentRewardClaimAllowed).toBe(true)
    expect(projection.acceptedContributionClaimAllowed).toBe(false)
    expect(projection.acceptedWorkClaimAllowed).toBe(false)
    expect(projection.rewardIntentClaimAllowed).toBe(false)
    expect(projection.payoutEligibilityClaimAllowed).toBe(false)
    expect(projection.payoutDispatchClaimAllowed).toBe(false)
    expect(projection.payoutVerificationClaimAllowed).toBe(false)
    expect(projection.settlementClaimAllowed).toBe(false)
    expect(projection.payoutRowRefs).toEqual([])
    expect(projection.proofLinkRefs).toEqual([])
    expect(projection.stateLabel).toBe('Content rewarded')
    expect(projection.updatedAtDisplay).toBe('15 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(forumAcceptedContributionBridgeProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('projects accepted Forum contributions to payout and proof refs without settlement authority', () => {
    const record = bridgeRecord(1)
    const projection = projectForumAcceptedContributionBridge(
      record,
      'public',
      nowIso,
    )

    expect(projection.bridgeKind).toBe('accepted_contribution_reward')
    expect(projection.contentRewardClaimAllowed).toBe(true)
    expect(projection.acceptedContributionClaimAllowed).toBe(true)
    expect(projection.acceptedWorkClaimAllowed).toBe(true)
    expect(projection.rewardIntentClaimAllowed).toBe(true)
    expect(projection.payoutEligibilityClaimAllowed).toBe(true)
    expect(projection.payoutDispatchClaimAllowed).toBe(false)
    expect(projection.payoutVerificationClaimAllowed).toBe(true)
    expect(projection.settlementClaimAllowed).toBe(false)
    expect(projection.liveWalletSpendAllowed).toBe(false)
    expect(projection.payoutDispatchMutationAllowed).toBe(false)
    expect(projection.payoutTargetDisclosureAllowed).toBe(false)
    expect(projection.settlementMutationAllowed).toBe(false)
    expect(projection.providerRef).toBe('provider.redacted')
    expect(projection.payoutRowRefs).toEqual([
      'payout.public.row.forum_research_summary',
    ])
    expect(projection.proofLinkRefs).toEqual([
      'proof_link.public.forum_research_summary',
    ])
    expect(projection.providerJobRefs).toEqual([
      'job.public.forum_research_summary',
    ])
    expect(projection.payoutVerificationRefs).toEqual([
      'verification.public.forum_research_summary',
    ])
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
  })

  test('requires explicit accepted-work evidence before adding payout/proof refs', () => {
    expect(() =>
      projectForumAcceptedContributionBridge(
        bridgeRecord(0, {
          acceptedWorkRefs: ['accepted_work.public.invalid'],
          payoutRowRefs: ['payout.public.invalid'],
          proofLinkRefs: ['proof_link.public.invalid'],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ForumAcceptedContributionBridgeUnsafe)

    expect(() =>
      projectForumAcceptedContributionBridge(
        bridgeRecord(1, {
          acceptedContributionReceiptRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ForumAcceptedContributionBridgeUnsafe)

    expect(() =>
      projectForumAcceptedContributionBridge(
        bridgeRecord(1, {
          acceptedWorkRefs: [],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ForumAcceptedContributionBridgeUnsafe)

    expect(() =>
      projectForumAcceptedContributionBridge(
        bridgeRecord(0, {
          bridgeKind: 'ordinary_content_reward',
          payoutDispatchRefs: ['dispatch.public.invalid'],
          state: 'payout_dispatched',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(ForumAcceptedContributionBridgeUnsafe)
  })

  test('redacts private accepted contribution refs and keeps settlement separate', () => {
    const projection = projectForumAcceptedContributionBridge(
      bridgeRecord(1, {
        payoutDispatchRefs: [
          'dispatch.public.forum_research_summary',
          'dispatch.private.operator_trace',
        ],
        settlementEvidenceRefs: [
          'settlement.public.forum_research_summary',
          'settlement.private.operator_trace',
        ],
        settlementRefs: ['settlement.public.forum_research_summary'],
        state: 'settled',
      }),
      'public',
      nowIso,
    )

    expect(projection.payoutDispatchRefs).toEqual([
      'dispatch.public.forum_research_summary',
    ])
    expect(projection.settlementEvidenceRefs).toEqual([
      'settlement.public.forum_research_summary',
    ])
    expect(projection.settlementClaimAllowed).toBe(true)
    expect(JSON.stringify(projection)).not.toContain('operator_trace')
    expect(JSON.stringify(projection)).not.toContain('dispatch.private')
    expect(JSON.stringify(projection)).not.toContain('settlement.private')
  })

  test('rejects mutable authority and unsafe payment, wallet, payout, provider, and timestamp material', () => {
    const base = bridgeRecord(1)

    expect(() =>
      projectForumAcceptedContributionBridge({
        ...base,
        authority: {
          ...FORUM_ACCEPTED_CONTRIBUTION_BRIDGE_READ_ONLY_AUTHORITY,
          noPayoutDispatch: false,
        },
      }, 'operator', nowIso),
    ).toThrow(ForumAcceptedContributionBridgeUnsafe)

    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'payment id', value: 'payment_id.raw_internal' },
      { label: 'raw payout target', value: 'payout_target.raw_destination' },
      { label: 'invoice', value: 'invoice.lnbc123' },
      { label: 'preimage', value: 'payment_preimage.raw_secret' },
      { label: 'wallet material', value: 'wallet.secret.seed' },
      { label: 'provider token', value: 'provider_token.local' },
      { label: 'channel monitor', value: 'channel_monitor.raw_state' },
    ]) {
      expect(() =>
        projectForumAcceptedContributionBridge({
          ...base,
          evidenceRefs: [fixture.value],
        }, 'operator', nowIso),
      ).toThrow(ForumAcceptedContributionBridgeUnsafe)
    }
  })
})
