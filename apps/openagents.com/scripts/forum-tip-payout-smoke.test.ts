import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./forum-tip-payout-smoke.mjs')

describe('Forum tip payout smoke', () => {
  test('combines payable preview with precise wallet blocker without leaking secrets', async () => {
    const runner = vi.fn(async argv => {
      if (argv[0] === 'reward-post') {
        return `${JSON.stringify({
          challenge: {
            challengeId: 'challenge_public_1',
            l402: {
              environment: 'production',
              provider: 'mdk_hosted',
              sandbox: false,
              settlementAuthority: 'buyer_payment_evidence_only',
            },
            recipientReadinessRef: 'readiness.public.receive_ready',
          },
          paymentRequired: true,
          writeDenial: { denialRef: 'forum_paid_action:post_reward' },
        })}\n`
      }

      return `${JSON.stringify({
        livePaymentAttempted: false,
        preflight: {
          blocker: {
            reasonRef: 'reason.public.agent_wallet_insufficient_balance',
          },
        },
        reasonRef: 'reason.public.agent_wallet_insufficient_balance',
        receipt: null,
        status: 'blocked',
      })}\n`
    })

    const output = await smoke.runForumTipPayoutSmoke(
      {
        approveLiveSpend: true,
        baseUrl: 'https://openagents.com',
        post: 'post_1',
        spendCapAmount: '100',
        spendCapAsset: 'sats',
      },
      { OPENAGENTS_AGENT_TOKEN: 'oa_agent_secret_123' },
      runner,
    )

    expect(output).toMatchObject({
      challenge: {
        paymentRequired: true,
        provider: 'mdk_hosted',
        recipientReadinessRef: 'readiness.public.receive_ready',
      },
      payment: {
        reasonRef: 'reason.public.agent_wallet_insufficient_balance',
        receiptRef: null,
        status: 'blocked',
      },
    })
    expect(JSON.stringify(output)).not.toContain('oa_agent_secret_123')
    expect(JSON.stringify(output)).not.toMatch(/lnbc|preimage|payment_hash/i)
  })
})
