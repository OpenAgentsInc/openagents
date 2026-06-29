import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY,
  OpenAgentsSiteMdkSmokeProjection,
  OpenAgentsSiteMdkSmokeRecord,
  OpenAgentsSiteMdkSmokeUnsafe,
  exampleOpenAgentsSiteMdkSmokeRecord,
  projectOpenAgentsSiteMdkSmoke,
} from './site-mdk-smoke'

const smokeRecord = (
  overrides: Partial<OpenAgentsSiteMdkSmokeRecord> = {},
): OpenAgentsSiteMdkSmokeRecord =>
  S.decodeUnknownSync(OpenAgentsSiteMdkSmokeRecord)({
    ...exampleOpenAgentsSiteMdkSmokeRecord(),
    ...overrides,
  })

describe('Site MDK smoke projection', () => {
  test('projects fake-provider smoke as CI evidence, not production payment evidence', () => {
    const projection = projectOpenAgentsSiteMdkSmoke(
      exampleOpenAgentsSiteMdkSmokeRecord(),
      'public',
    )

    expect(S.decodeUnknownSync(OpenAgentsSiteMdkSmokeProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      acceptedWorkPayoutClaimAllowed: false,
      authority: OPENAGENTS_SITE_MDK_SMOKE_RECORD_ONLY_AUTHORITY,
      implementationState: 'fake_provider',
      implementationStateLabel: 'Fake provider CI smoke',
      notProductionPaymentEvidence: true,
      passedCheckCount: 11,
      providerPayoutClaimAllowed: false,
      settlementClaimAllowed: false,
      smokeState: 'passed',
      walletSpendAllowed: false,
    })
    expect(projection.checkRecords.map(check => check.checkName).sort())
      .toEqual([
        'checkout_intent',
        'clean_return_status',
        'discovery',
        'l402_challenge',
        'l402_redemption',
        'payment_proof',
        'provider_reconciliation',
        'provider_replay',
        'redaction',
        'spend_cap_rejection',
        'stale_rejection',
      ])
    expect(JSON.stringify(projection)).not.toMatch(
      /(2026-\d{2}-\d{2}T|lnbc|payment_hash|preimage|mnemonic|MDK_ACCESS_TOKEN|wallet_secret|webhook_secret)/i,
    )
  })

  test('distinguishes skipped and failed smoke checks honestly', () => {
    const skipped = projectOpenAgentsSiteMdkSmoke(
      smokeRecord({
        checkRecords: exampleOpenAgentsSiteMdkSmokeRecord()
          .checkRecords.map(check =>
            check.checkName === 'provider_reconciliation'
              ? { ...check, status: 'skipped' }
              : check,
          ),
      }),
      'operator',
    )
    const failed = projectOpenAgentsSiteMdkSmoke(
      smokeRecord({
        checkRecords: exampleOpenAgentsSiteMdkSmokeRecord()
          .checkRecords.map(check =>
            check.checkName === 'redaction'
              ? {
                  ...check,
                  blockerRefs: ['blocker.public.site_mdk_smoke.redaction'],
                  status: 'failed',
                }
              : check,
          ),
      }),
      'operator',
    )

    expect(skipped.smokeState).toBe('skipped')
    expect(failed.smokeState).toBe('failed')
  })

  test('rejects raw payment, credential, wallet, customer, and timestamp material', () => {
    for (const unsafe of [
      smokeRecord({ checkoutIntentRefs: ['checkout_id=raw'] }),
      smokeRecord({ paymentProofRefs: ['payment_hash=secret'] }),
      smokeRecord({ l402ChallengeRefs: ['invoice.lnbc123'] }),
      smokeRecord({ receiptRefs: ['wallet.secret.seed'] }),
      smokeRecord({ sourceRefs: ['customer_email_ben@example.com'] }),
      smokeRecord({ sourceRefs: ['source.public.2026-06-07T02:00:00.000Z'] }),
    ]) {
      expect(() =>
        projectOpenAgentsSiteMdkSmoke(unsafe, 'operator'),
      ).toThrow(OpenAgentsSiteMdkSmokeUnsafe)
    }
  })
})
