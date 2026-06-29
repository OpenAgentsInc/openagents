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
      defaultCaptureGate: 'owner_gated',
      redacted: true,
      defaultVisibility: 'owner_only',
      mayTrain: true,
      paidPrivacyOptOut: true,
      publicSharingOptIn: true,
      rewardInert: true,
    })
    expect(disclosure.reportPath).toContain('forum/f/product-promises')
    expect(disclosure.blockerRefs).toEqual([
      'blocker.product_promises.free_tier_capture_default_owner_gated',
      'blocker.product_promises.disclosure_copy_owner_signoff_pending',
      'blocker.product_promises.trace_capture_public_disclosure_alignment_required',
      'blocker.product_promises.trace_capture_reward_marker_inert',
      'blocker.product_promises.paid_privacy_owner_signoff_pending',
      'blocker.product_promises.paid_khala_business_loop_not_green',
    ])
    expect(disclosure.gates.defaultCapture).toEqual({
      state: 'owner_gated',
      envFlag: 'KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT',
      blockerRef:
        'blocker.product_promises.free_tier_capture_default_owner_gated',
    })
    expect(disclosure.gates.paidPrivacyOptOut).toEqual({
      state: 'wired_yellow',
      failClosed: true,
      blockerRefs: [
        'blocker.product_promises.paid_privacy_owner_signoff_pending',
        'blocker.product_promises.paid_khala_business_loop_not_green',
      ],
    })
    expect(disclosure.gates.traceRewards).toEqual({
      state: 'inert',
      payoutClaimAllowed: false,
      blockerRef:
        'blocker.product_promises.trace_capture_reward_marker_inert',
    })
  })

  test('terms cover the four honest clauses without overclaiming', () => {
    const text = freeTierDataSharingDisclosure()
      .terms.join(' ')
      .toLowerCase()
    // captured by default, redacted, private
    expect(text).toContain('captured by default')
    expect(text).toContain('owner-gated')
    expect(text).toContain('khala_free_tier_trace_capture_default')
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
    const body = (await ok.json()) as {
      promiseId: string
      gates: { defaultCapture: { state: string } }
      blockerRefs: ReadonlyArray<string>
    }
    expect(body.promiseId).toBe(FREE_TIER_DATA_SHARING_PROMISE_ID)
    expect(body.gates.defaultCapture.state).toBe('owner_gated')
    expect(body.blockerRefs).toContain(
      'blocker.product_promises.free_tier_capture_default_owner_gated',
    )

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
