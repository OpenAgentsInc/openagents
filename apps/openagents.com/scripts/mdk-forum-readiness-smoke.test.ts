import { describe, expect, test, vi } from 'vitest'

const smoke = await import('./mdk-forum-readiness-smoke.mjs')

describe('MDK Forum readiness smoke', () => {
  test('separates hosted payout authority from wallet send readiness', async () => {
    const runner = vi.fn(async argv => {
      if (argv[0] === 'wallet-status') {
        return `${JSON.stringify({
          blocker: {
            reasonRef: 'reason.public.agent_wallet_insufficient_balance',
          },
          checks: [
            { commandRef: 'mdk_agent_wallet.status', status: 'passed' },
            { commandRef: 'mdk_agent_wallet.init_show', status: 'passed' },
            {
              commandRef: 'mdk_agent_wallet.balance',
              reasonRef: 'reason.public.agent_wallet_insufficient_balance',
              status: 'blocked',
            },
          ],
          livePaymentAttempted: false,
          ready: false,
          status: 'blocked',
        })}\n`
      }

      if (argv[0] === 'reward-post') {
        return `${JSON.stringify({
          challenge: {
            challengeId: 'challenge_public_1',
            l402: {
              environment: 'production',
              provider: 'mdk_hosted',
              providerPayoutAuthority: false,
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
        reasonRef: 'reason.public.agent_wallet_insufficient_balance',
        status: 'blocked',
      })}\n`
    })

    const output = await smoke.runMdkForumReadinessSmoke(
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

    expect(output.gates).toMatchObject({
      hostedPayout: {
        blockerRefs: [
          'blocker.product_promises.hosted_mdk_direct_payout_authority_disabled',
        ],
        directPayoutEnabled: false,
        state: 'evidence_only',
      },
      restoreSend: {
        blockerRefs: ['reason.public.agent_wallet_insufficient_balance'],
        sendReady: false,
        state: 'blocked',
      },
    })
    expect(JSON.stringify(output)).not.toContain('oa_agent_secret_123')
    expect(JSON.stringify(output)).not.toMatch(
      /lnbc|lntb|preimage|payment_hash|mnemonic|secret/i,
    )
  })
})
