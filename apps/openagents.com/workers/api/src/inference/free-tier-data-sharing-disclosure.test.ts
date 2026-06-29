import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  FREE_TIER_DATA_SHARING_DISCLOSURE_VERSION,
  FREE_TIER_DATA_SHARING_PROMISE_ID,
  freeTierDataSharingDisclosure,
} from './free-tier-data-sharing-disclosure'
import { handleFreeTierDataSharingDisclosureApi } from './free-tier-data-sharing-routes'

describe('free-tier data-sharing disclosure (#6296)', () => {
  test('is the canonical, code-accurate disclosure object', () => {
    const disclosure = freeTierDataSharingDisclosure()
    expect(disclosure.promiseId).toBe(FREE_TIER_DATA_SHARING_PROMISE_ID)
    expect(disclosure.version).toBe(FREE_TIER_DATA_SHARING_DISCLOSURE_VERSION)
    expect(disclosure.terms.length).toBeGreaterThan(0)
    // The bounded policy facts mirror the runtime capture seams.
    expect(disclosure.policy).toEqual({
      capturedByDefault: true,
      captureDefaultOwnerGated: true,
      captureDefaultGate: 'KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT',
      redacted: true,
      defaultVisibility: 'owner_only',
      mayTrain: true,
      paidPrivacyOptOut: true,
      publicSharingOptIn: true,
      rewardInert: true,
      blockerRefs: [
        'blocker.product_promises.free_tier_capture_default_owner_gated',
        'blocker.product_promises.disclosure_copy_owner_signoff_pending',
      ],
    })
    expect(disclosure.reportPath).toContain('forum/f/product-promises')
  })

  test('terms cover the four honest clauses without overclaiming', () => {
    const text = freeTierDataSharingDisclosure()
      .terms.join(' ')
      .toLowerCase()
    // captured by default, redacted, private
    expect(text).toContain('captured by default')
    expect(text).toContain('owner-gated blocker')
    expect(text).toContain('redacted')
    expect(text).toContain('owner_only')
    // may improve/train
    expect(text).toContain('train')
    // pay for privacy to opt out
    expect(text).toContain('pay')
    expect(text).toContain('opt out')
    // public sharing opt-in only
    expect(text).toContain('opt-in')
    // no payout/reward overclaim
    expect(text).toContain('no payment')
  })

  test('GET serves the disclosure; non-GET is 405', async () => {
    const ok = await Effect.runPromise(
      handleFreeTierDataSharingDisclosureApi(
        new Request('https://openagents.com/api/public/free-tier-data-sharing'),
      ),
    )
    expect(ok.status).toBe(200)
    const body = (await ok.json()) as { promiseId: string }
    expect(body.promiseId).toBe(FREE_TIER_DATA_SHARING_PROMISE_ID)

    const denied = await Effect.runPromise(
      handleFreeTierDataSharingDisclosureApi(
        new Request(
          'https://openagents.com/api/public/free-tier-data-sharing',
          { method: 'POST' },
        ),
      ),
    )
    expect(denied.status).toBe(405)
  })
})
