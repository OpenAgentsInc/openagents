import { describe, expect, it } from 'vitest'

import {
  buildPublicTassadarCompiledModuleMarketplaceEnvelope,
} from './tassadar-compiled-module-marketplace'

describe('Tassadar compiled module marketplace projection', () => {
  it('exposes the linked dense module as a digest-pinned replay-verified listing', async () => {
    const envelope = await buildPublicTassadarCompiledModuleMarketplaceEnvelope(
      '2026-06-18T12:00:00.000Z',
    )
    const listing = envelope.listings[0]

    expect(envelope.generatedAt).toBe('2026-06-18T12:00:00.000Z')
    expect(listing).toBeDefined()
    if (listing === undefined) {
      throw new Error('missing compiled module listing')
    }
    expect(envelope.staleness.maxStalenessSeconds).toBe(0)
    expect(envelope.authority).toEqual({
      listingMutationAuthority: false,
      purchaseMutationAuthority: false,
      realSettlementEnabled: false,
      settlementMutationAuthority: false,
    })
    expect(listing).toMatchObject({
      compositionVerificationCleared: true,
      dependencyEdgeCount: 1,
      linkCompatibilityVerified: true,
      linkedModuleDigest:
        'cc1403674fc0d38892610d9e9c6c9230075494061f720c45bfa4f7b5a961756a',
      replayVerificationCleared: true,
      settlementClaimAllowed: false,
      sourceBankCount: 2,
      state: 'replay_verified_listed',
    })
    expect(listing.purchaseSettlementAllowed).toBe(false)
    expect(listing.blockerRefs).toContain(
      'blocker.public.tassadar_compiled_module.purchase_receipt_missing',
    )
    expect(listing.blockerRefs).toContain(
      'blocker.public.tassadar_compiled_module.settlement_receipt_missing',
    )
  })

  it('keeps the public projection free of raw private material', async () => {
    const envelope = await buildPublicTassadarCompiledModuleMarketplaceEnvelope(
      '2026-06-18T12:00:00.000Z',
    )
    const json = JSON.stringify(envelope)

    expect(json).not.toMatch(
      /trace_raw|raw_prompt|provider_payload|customer_email|payment_preimage|payment_invoice|lnbc|@|github\.com\/[^:/]+\/private/i,
    )
  })
})
