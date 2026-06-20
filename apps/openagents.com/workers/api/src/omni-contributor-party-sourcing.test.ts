import { describe, expect, test } from 'vitest'

import type { OmniAcceptedOutcomeEconomicsRecord } from './omni-accepted-outcome-economics'
import {
  buildOmniContributorAccrualBundleFromRecord,
  OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY,
  OmniContributorPartySourcingError,
  resolveOmniContributorPartiesFromRecord,
} from './omni-contributor-party-sourcing'

const recordWith = (
  contributors: Record<string, unknown> | undefined,
  overrides: Partial<OmniAcceptedOutcomeEconomicsRecord> = {},
): OmniAcceptedOutcomeEconomicsRecord => ({
  acceptedOutcomeContractId: 'omni_accepted_outcome_contract_1',
  acceptedValueCents: 5000,
  archivedAt: null,
  artifactCostCents: 100,
  buyerPriceAsset: 'usd',
  buyerPriceCents: 5000,
  createdAt: '2026-06-20T00:00:00.000Z',
  creditsCharged: 0,
  fundingMode: 'credit_funded',
  grossMarginCents: 4400,
  id: 'omni_outcome_economics_1',
  idempotencyKey: 'idem-1',
  internalCaveatRef: null,
  metadata:
    contributors === undefined
      ? {}
      : { [OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY]: contributors },
  noSettlementImplication: true,
  providerCostCents: 300,
  publicCaveatRef: 'caveat.no_settlement',
  retryCostCents: 0,
  reviewCostCents: 100,
  reviewMinutes: 5,
  runnerCostCents: 100,
  satsCharged: 0,
  totalCostCents: 600,
  updatedAt: '2026-06-20T00:00:00.000Z',
  workKind: 'coding',
  workroomId: 'omni_workroom_coding_1',
  ...overrides,
})

describe('resolveOmniContributorPartiesFromRecord', () => {
  test('sources a runner-only record', () => {
    const parties = resolveOmniContributorPartiesFromRecord(
      recordWith({ runnerId: 'runner-1' }),
    )
    expect(parties).toEqual({ runnerId: 'runner-1' })
  })

  test('sources all optional roles when present', () => {
    const parties = resolveOmniContributorPartiesFromRecord(
      recordWith({
        runnerId: 'runner-1',
        reviewerId: 'reviewer-1',
        originatorId: 'origin-1',
        referrerId: 'ref-1',
        platformId: 'platform-eu',
      }),
    )
    expect(parties).toEqual({
      runnerId: 'runner-1',
      reviewerId: 'reviewer-1',
      originatorId: 'origin-1',
      referrerId: 'ref-1',
      platformId: 'platform-eu',
    })
  })

  test('omits absent optional fields rather than emitting undefined keys', () => {
    const parties = resolveOmniContributorPartiesFromRecord(
      recordWith({ runnerId: 'runner-1', reviewerId: 'reviewer-1' }),
    )
    expect(Object.keys(parties).sort()).toEqual(['reviewerId', 'runnerId'])
  })

  test('is deterministic for the same record', () => {
    const record = recordWith({ runnerId: 'runner-1', originatorId: 'origin-1' })
    expect(resolveOmniContributorPartiesFromRecord(record)).toEqual(
      resolveOmniContributorPartiesFromRecord(record),
    )
  })

  test('fails when metadata.contributors is missing', () => {
    expect(() =>
      resolveOmniContributorPartiesFromRecord(recordWith(undefined)),
    ).toThrow(OmniContributorPartySourcingError)
  })

  test('fails when metadata.contributors is not an object', () => {
    const record = recordWith(undefined, {
      metadata: { [OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY]: 'runner-1' },
    })
    expect(() => resolveOmniContributorPartiesFromRecord(record)).toThrow(
      OmniContributorPartySourcingError,
    )
  })

  test('fails when metadata.contributors is an array', () => {
    const record = recordWith(undefined, {
      metadata: { [OMNI_CONTRIBUTOR_PARTIES_METADATA_KEY]: ['runner-1'] },
    })
    expect(() => resolveOmniContributorPartiesFromRecord(record)).toThrow(
      OmniContributorPartySourcingError,
    )
  })

  test('fails when runnerId is absent', () => {
    expect(() =>
      resolveOmniContributorPartiesFromRecord(
        recordWith({ reviewerId: 'reviewer-1' }),
      ),
    ).toThrow(OmniContributorPartySourcingError)
  })

  test('fails when a present id is the wrong type', () => {
    expect(() =>
      resolveOmniContributorPartiesFromRecord(
        recordWith({ runnerId: 'runner-1', reviewerId: 42 }),
      ),
    ).toThrow(OmniContributorPartySourcingError)
  })

  test('fails when a present id is an empty string', () => {
    expect(() =>
      resolveOmniContributorPartiesFromRecord(recordWith({ runnerId: '' })),
    ).toThrow(OmniContributorPartySourcingError)
  })

  test('exposes a tagged error type', () => {
    const error = new OmniContributorPartySourcingError({ reason: 'test' })
    expect(error._tag).toBe('OmniContributorPartySourcingError')
  })
})

describe('buildOmniContributorAccrualBundleFromRecord', () => {
  test('builds a reconciled bundle sourcing parties from the record', () => {
    const bundle = buildOmniContributorAccrualBundleFromRecord(
      recordWith({ runnerId: 'runner-1', reviewerId: 'reviewer-1' }),
    )
    expect(bundle.economicsId).toBe('omni_outcome_economics_1')
    expect(bundle.reconciledGrossMarginCents).toBe(4400)
    const ids = bundle.contributorAccrualLedger.entries.map(
      entry => entry.contributorId,
    )
    expect(ids).toContain('runner-1')
    expect(ids).toContain('reviewer-1')
    expect(ids).toContain('platform')
    expect(bundle.contributorAccrualLedger.totalAccruedCents).toBe(4400)
  })

  test('propagates sourcing failure for a record with no parties', () => {
    expect(() =>
      buildOmniContributorAccrualBundleFromRecord(recordWith(undefined)),
    ).toThrow(OmniContributorPartySourcingError)
  })

  test('keeps settlement disclaimed across both halves', () => {
    const bundle = buildOmniContributorAccrualBundleFromRecord(
      recordWith({ runnerId: 'runner-1' }),
    )
    expect(bundle.grossMarginReceipt.noSettlementImplication).toBe(true)
    expect(bundle.contributorAccrualLedger.noSettlementImplication).toBe(true)
  })
})
