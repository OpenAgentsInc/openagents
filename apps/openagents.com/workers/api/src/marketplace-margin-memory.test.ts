import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MARKETPLACE_MARGIN_MEMORY_NO_AUTHORITY,
  MarketplaceMarginMemoryProjection,
  MarketplaceMarginMemoryRecord,
  MarketplaceMarginMemoryUnsafe,
  exampleMarketplaceMarginMemory,
  marketplaceMarginMemoryHasMutationAuthority,
  marketplaceMarginMemoryProjectionHasPrivateMaterial,
  marketplaceMarginMemoryPublicRankCandidateAllowed,
  projectMarketplaceMarginMemory,
} from './marketplace-margin-memory'

const nowIso = '2026-06-06T22:50:00.000Z'

const memory = (
  overrides: Partial<MarketplaceMarginMemoryRecord> = {},
): MarketplaceMarginMemoryRecord =>
  S.decodeUnknownSync(MarketplaceMarginMemoryRecord)({
    ...exampleMarketplaceMarginMemory(),
    ...overrides,
  })

describe('Marketplace margin memory', () => {
  test('projects accepted outcome attribution without mutation authority', () => {
    const record = memory()
    const publicProjection = projectMarketplaceMarginMemory(
      record,
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(MarketplaceMarginMemoryRecord)(record))
      .toEqual(record)
    expect(S.decodeUnknownSync(MarketplaceMarginMemoryProjection)(
      publicProjection,
    )).toEqual(publicProjection)
    expect(marketplaceMarginMemoryHasMutationAuthority(record)).toBe(false)
    expect(publicProjection).toMatchObject({
      acceptedCount: 3,
      acceptanceRateBps: 7500,
      acceptedGrossProfitCents: 18_000,
      acceptedOutcomeClaimAllowed: true,
      acceptedRevenueCents: 30_000,
      automaticPublicRankMutationAllowed: false,
      grossMarginBps: 6000,
      grossMarginClaimAllowed: true,
      modeledMarketplaceValueClaimAllowed: true,
      modulePromotionAllowed: false,
      payoutMutationAllowed: false,
      providerPayableCents: 6_000,
      publicRankCandidateAllowed: true,
      rankingScoreBps: 4925,
      rejectedOutcomeClaimAllowed: true,
      repeatBuyerClaimAllowed: true,
      repeatBuyerCount: 2,
      repeatBuyerRateBps: 6667,
      retryCount: 2,
      revenueClaimAllowed: true,
      routingMutationAllowed: false,
      settledProviderCents: 0,
      settlementClaimAllowed: true,
      settlementMutationAllowed: false,
      settlementState: 'payable',
      settlementStateLabel: 'Payable',
      totalBuyerCount: 3,
      updatedAtDisplay: '10 minutes ago',
    })
    expect(publicProjection.acceptedOutcomeRefs).toEqual([
      'accepted.outcome.site_revision_4',
    ])
    expect(publicProjection.providerRefs).toEqual([
      'provider.public.openagents_runner',
    ])
    expect(marketplaceMarginMemoryProjectionHasPrivateMaterial(
      publicProjection,
    )).toBe(false)
  })

  test('keeps measured outcomes, modeled value, revenue, gross margin, refunds, repeat buyers, and settlement separate', () => {
    const projection = projectMarketplaceMarginMemory(
      memory({
        acceptedGrossProfitCents: 0,
        acceptedRevenueCents: 0,
        grossMarginEvidenceRefs: [],
        refundCount: 0,
        refundedOutcomeRefs: [],
        refundedRevenueCents: 0,
        rejectedCount: 0,
        rejectedOutcomeRefs: [],
        repeatBuyerCount: 0,
        repeatBuyerSignalRefs: [],
        revenueEvidenceRefs: [],
        settlementStateRefs: [],
        totalBuyerCount: 0,
      }),
      'team',
      nowIso,
    )

    expect(projection.acceptedOutcomeClaimAllowed).toBe(true)
    expect(projection.modeledMarketplaceValueClaimAllowed).toBe(true)
    expect(projection.revenueClaimAllowed).toBe(false)
    expect(projection.grossMarginClaimAllowed).toBe(false)
    expect(projection.refundClaimAllowed).toBe(false)
    expect(projection.refundRateBps).toBe(0)
    expect(projection.rejectedOutcomeClaimAllowed).toBe(false)
    expect(projection.repeatBuyerClaimAllowed).toBe(false)
    expect(projection.repeatBuyerRateBps).toBe(0)
    expect(projection.settlementClaimAllowed).toBe(false)
  })

  test('blocks drafts, unreviewed modules, and authoritative records from rank or routing mutation', () => {
    const draft = memory({ reviewState: 'draft' })
    const unreviewed = memory({ reviewState: 'unreviewed' })
    const authoritative = memory({
      authority: {
        ...MARKETPLACE_MARGIN_MEMORY_NO_AUTHORITY,
        noRoutingMutation: false,
      },
    })

    expect(marketplaceMarginMemoryPublicRankCandidateAllowed(draft))
      .toBe(false)
    expect(marketplaceMarginMemoryPublicRankCandidateAllowed(unreviewed))
      .toBe(false)
    expect(marketplaceMarginMemoryPublicRankCandidateAllowed(authoritative))
      .toBe(false)
    expect(() =>
      projectMarketplaceMarginMemory(authoritative, 'operator', nowIso),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
  })

  test('requires count and economic evidence before claims can be projected', () => {
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({ acceptedCount: 1, acceptedOutcomeRefs: [] }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({ rejectedCount: 1, rejectedOutcomeRefs: [] }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({ refundCount: 1, refundedOutcomeRefs: [] }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({ retryCount: 1, retryEvidenceRefs: [] }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({
          acceptedRevenueCents: 0,
          acceptedCount: 0,
          acceptedOutcomeRefs: [],
          grossMarginEvidenceRefs: [],
          revenueEvidenceRefs: ['revenue.evidence.without_acceptance'],
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({
          acceptedGrossProfitCents: 10_000,
          grossMarginEvidenceRefs: ['gross_margin.evidence.without_revenue'],
          revenueEvidenceRefs: [],
        }),
        'operator',
        nowIso,
      ),
      ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({
          providerPayableCents: 1000,
          settledProviderCents: 1000,
          settlementState: 'accepted',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({
          providerPayableCents: 1000,
          settledProviderCents: 1200,
          settlementState: 'settled',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
    expect(() =>
      projectMarketplaceMarginMemory(
        memory({
          repeatBuyerCount: 2,
          repeatBuyerSignalRefs: [],
          totalBuyerCount: 3,
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(MarketplaceMarginMemoryUnsafe)
  })

  test('redacts audience-private refs and rejects unsafe marketplace memory material', () => {
    const projection = projectMarketplaceMarginMemory(
      memory({
        providerRefs: [
          'provider.private.runner_account',
          'provider.public.runner_pool',
        ],
        reviewerRefs: [
          'reviewer.private.operator_account',
          'reviewer.public.operator_pool',
        ],
        settlementStateRefs: [
          'settlement.private.internal_batch',
          'settlement.public.pending_receipt',
        ],
        sourceRefs: [
          'source.private.raw_archive_pointer',
          'source.public.exa_summary',
        ],
      }),
      'public',
      nowIso,
    )

    expect(projection.providerRefs).toEqual(['provider.public.runner_pool'])
    expect(projection.reviewerRefs).toEqual(['reviewer.public.operator_pool'])
    expect(projection.settlementStateRefs).toEqual([
      'settlement.public.pending_receipt',
    ])
    expect(projection.sourceRefs).toEqual(['source.public.exa_summary'])
    expect(marketplaceMarginMemoryProjectionHasPrivateMaterial(projection))
      .toBe(false)

    for (const unsafeRef of [
      'ben@example.com',
      'raw_source_archive.customer_zip',
      'provider_payload.full_response',
      'wallet.mnemonic.local',
      'payout_address.bc1qsecret',
      'github.com/acme/private',
      'raw_runner_log.full',
      '2026-06-06T22:45:00.000Z',
    ]) {
      expect(() =>
        projectMarketplaceMarginMemory(
          memory({ evidenceRefs: [unsafeRef] }),
          'public',
          nowIso,
        ),
      ).toThrow(MarketplaceMarginMemoryUnsafe)
    }
  })
})
