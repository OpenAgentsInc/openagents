import { describe, expect, test } from 'vitest'

import {
  ForumTipPayerWalletReadinessUnsafe,
  forumTipPayerWalletReadinessHasPrivateMaterial,
  projectForumTipPayerWalletReadiness,
  type ForumTipPayerWalletReadinessInput,
} from './payer-wallet-readiness'

const readinessInput = (
  overrides: Partial<ForumTipPayerWalletReadinessInput> = {},
): ForumTipPayerWalletReadinessInput => ({
  actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  caveatRefs: ['caveat.public.forum_tip_payer.self_custody'],
  configuredRefs: [],
  fundedRefs: [],
  sendReadyRefs: [],
  sourceRef: 'source.public.forum_tip_payer.wallet_preflight',
  ...overrides,
})

describe('Forum tip payer wallet readiness', () => {
  test('keeps missing, configured, funded, and send-ready states distinct', () => {
    const missing = projectForumTipPayerWalletReadiness(readinessInput())
    const configured = projectForumTipPayerWalletReadiness(
      readinessInput({
        configuredRefs: ['readiness.public.mdk_agent.preflight_configured'],
      }),
    )
    const funded = projectForumTipPayerWalletReadiness(
      readinessInput({
        configuredRefs: ['readiness.public.mdk_agent.preflight_configured'],
        fundedRefs: ['readiness.public.mdk_agent.preflight_funded'],
      }),
    )
    const sendReady = projectForumTipPayerWalletReadiness(
      readinessInput({
        configuredRefs: ['readiness.public.mdk_agent.preflight_configured'],
        fundedRefs: ['readiness.public.mdk_agent.preflight_funded'],
        sendReadyRefs: ['readiness.public.mdk_agent.preflight_send_ready'],
      }),
    )

    expect(missing).toMatchObject({
      blockerRefs: ['blocker.public.forum_tip_payer.wallet_missing'],
      state: 'missing',
      tippingSpendAllowed: false,
    })
    expect(configured).toMatchObject({
      blockerRefs: ['blocker.public.forum_tip_payer.wallet_unfunded'],
      state: 'configured',
      tippingSpendAllowed: false,
    })
    expect(funded).toMatchObject({
      blockerRefs: ['blocker.public.forum_tip_payer.send_readiness_missing'],
      state: 'funded',
      tippingSpendAllowed: false,
    })
    expect(sendReady).toMatchObject({
      blockerRefs: [],
      state: 'send_ready',
      tippingSpendAllowed: true,
    })
  })

  test('does not expose balance, invoices, preimages, paths, or wallet secrets', () => {
    const projection = projectForumTipPayerWalletReadiness(
      readinessInput({
        configuredRefs: ['readiness.public.mdk_agent.preflight_configured'],
        fundedRefs: ['readiness.public.mdk_agent.preflight_funded'],
        sendReadyRefs: ['readiness.public.mdk_agent.preflight_send_ready'],
      }),
    )
    const serialized = JSON.stringify(projection)

    expect(forumTipPayerWalletReadinessHasPrivateMaterial(projection)).toBe(
      false,
    )
    expect(serialized).not.toContain('balance_sats')
    expect(serialized).not.toContain('lnbc')
    expect(serialized).not.toContain('preimage')
    expect(serialized).not.toContain('/Users/')
    expect(serialized).not.toContain('mnemonic')
  })

  test('rejects raw payer wallet and payment material before projection', () => {
    for (const input of [
      readinessInput({ configuredRefs: ['/Users/private/.mdk-wallet'] }),
      readinessInput({ fundedRefs: ['balance_sats.100000'] }),
      readinessInput({ sendReadyRefs: ['payment_preimage.private'] }),
      readinessInput({ caveatRefs: ['lnbc1privateinvoice'] }),
      readinessInput({ sourceRef: 'wallet.secret.seed' }),
    ]) {
      expect(() => projectForumTipPayerWalletReadiness(input)).toThrow(
        ForumTipPayerWalletReadinessUnsafe,
      )
    }
  })
})
