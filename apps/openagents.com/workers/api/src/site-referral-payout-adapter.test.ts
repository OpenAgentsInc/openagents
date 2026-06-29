import { describe, expect, test } from 'vitest'

import {
  type ReferralPayoutProgrammaticClient,
  makeSiteReferralPayoutAdapter,
} from './site-referral-payout-adapter'

// A recording mock of the hosted-MDK programmatic-payout rail. It moves NO
// money: it records the call and returns a deterministic payment id/hash.
const makeRecordingClient = (
  result: {
    paymentHash?: string
    paymentId: string
    status: 'REQUESTED' | 'SUCCESS' | 'FAILED'
  } = { paymentHash: 'rawhash', paymentId: 'pay_1', status: 'SUCCESS' },
): {
  client: ReferralPayoutProgrammaticClient
  calls: Array<{ amountSats: number; destination: string; idempotencyKey: string }>
} => {
  const calls: Array<{
    amountSats: number
    destination: string
    idempotencyKey: string
  }> = []

  return {
    calls,
    client: {
      programmaticPayout: input => {
        calls.push(input)
        return Promise.resolve(result)
      },
    },
  }
}

const REUSABLE_DESTINATION = 'lno1qsgqaaaaaaaaaaa'

describe('site referral payout adapter (RL-1 settle wire, #5511)', () => {
  test('settles through the rail and returns a redacted receipt ref (no raw material)', async () => {
    const { client, calls } = makeRecordingClient()
    const adapter = makeSiteReferralPayoutAdapter({
      client,
      resolveDestination: async () => REUSABLE_DESTINATION,
    })

    const result = await adapter.dispatch({
      amountSats: 125,
      idempotencyKey: 'site_referral_payout.adapter.payout_1',
      payoutRef: 'site_referral_payout_attribution_1',
    })

    expect(adapter.adapterKind).toBe('hosted_mdk')
    expect(calls).toHaveLength(1)
    expect(calls[0]).toEqual({
      amountSats: 125,
      destination: REUSABLE_DESTINATION,
      idempotencyKey: 'site_referral_payout.adapter.payout_1',
    })
    // Receipt is a stable redacted ref derived from the rail payment hash; it
    // contains NO raw payment material.
    expect(result.receiptRef).toMatch(
      /^receipt\.site_referral_payout\.hosted_mdk\.[0-9a-f]{32}$/,
    )
    expect(result.receiptRef).not.toContain('rawhash')
    expect(result.receiptRef).not.toContain('pay_1')
    expect(result.receiptRef).not.toContain(REUSABLE_DESTINATION)
  })

  test('the redacted receipt is deterministic for the same payment material', async () => {
    const adapterFor = () =>
      makeSiteReferralPayoutAdapter({
        client: makeRecordingClient().client,
        resolveDestination: async () => REUSABLE_DESTINATION,
      })

    const input = {
      amountSats: 125,
      idempotencyKey: 'k',
      payoutRef: 'site_referral_payout_attribution_1',
    }
    const first = await adapterFor().dispatch(input)
    const second = await adapterFor().dispatch(input)

    expect(first.receiptRef).toBe(second.receiptRef)
  })

  test('fails closed (throws) when no payout client is armed -- inert/owner-gated path', async () => {
    const adapter = makeSiteReferralPayoutAdapter({
      client: null,
      resolveDestination: async () => REUSABLE_DESTINATION,
    })

    await expect(
      adapter.dispatch({
        amountSats: 125,
        idempotencyKey: 'k',
        payoutRef: 'site_referral_payout_attribution_1',
      }),
    ).rejects.toMatchObject({ reason: expect.stringContaining('unconfigured') })
  })

  test('fails closed (throws) when the referrer has no registered destination', async () => {
    const { client, calls } = makeRecordingClient()
    const adapter = makeSiteReferralPayoutAdapter({
      client,
      resolveDestination: async () => null,
    })

    await expect(
      adapter.dispatch({
        amountSats: 125,
        idempotencyKey: 'k',
        payoutRef: 'site_referral_payout_attribution_1',
      }),
    ).rejects.toMatchObject({ reason: expect.stringContaining('destination_unavailable') })
    // No rail call: no money moves.
    expect(calls).toHaveLength(0)
  })

  test('refuses a non-reusable (bolt11) destination -- a single-use invoice is not a payout target', async () => {
    const { client, calls } = makeRecordingClient()
    const adapter = makeSiteReferralPayoutAdapter({
      client,
      resolveDestination: async () => 'lnbc1singleuseinvoice',
    })

    await expect(
      adapter.dispatch({
        amountSats: 125,
        idempotencyKey: 'k',
        payoutRef: 'site_referral_payout_attribution_1',
      }),
    ).rejects.toMatchObject({ reason: expect.stringContaining('destination_invalid') })
    expect(calls).toHaveLength(0)
  })

  test('throws (records no settled state) when the rail reports FAILED', async () => {
    const { client } = makeRecordingClient({
      paymentId: 'pay_1',
      status: 'FAILED',
    })
    const adapter = makeSiteReferralPayoutAdapter({
      client,
      resolveDestination: async () => REUSABLE_DESTINATION,
    })

    await expect(
      adapter.dispatch({
        amountSats: 125,
        idempotencyKey: 'k',
        payoutRef: 'site_referral_payout_attribution_1',
      }),
    ).rejects.toMatchObject({ reason: expect.stringContaining('rail_failed') })
  })

  test('accepts an LN-address (reusable) destination', async () => {
    const { client, calls } = makeRecordingClient()
    const adapter = makeSiteReferralPayoutAdapter({
      client,
      resolveDestination: async () => 'referrer@openagents.com',
    })

    const result = await adapter.dispatch({
      amountSats: 50,
      idempotencyKey: 'k',
      payoutRef: 'site_referral_payout_attribution_1',
    })

    expect(calls).toHaveLength(1)
    expect(result.receiptRef).toMatch(/^receipt\.site_referral_payout\.hosted_mdk\./)
  })
})
