import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { OpenAgentsMdkAgentWalletSmokeInput } from './mdk-agent-wallet-smoke-fixture'
import {
  OpenAgentsMdkAgentWalletSmokeProjection,
  OpenAgentsMdkAgentWalletSmokeUnsafe,
  openAgentsMdkAgentWalletSmokeHasPrivateMaterial,
  planOpenAgentsMdkAgentWalletSmoke,
} from './mdk-agent-wallet-smoke-fixture'

const baseInput = (
  overrides: Partial<OpenAgentsMdkAgentWalletSmokeInput> = {},
): OpenAgentsMdkAgentWalletSmokeInput => ({
  amountBitcoinSatoshis: 1000,
  endpointRef: 'endpoint.openagents.site_otec.paid_summary',
  mode: 'fake_sandbox',
  operatorApprovedPayment: false,
  routeStateRef: 'route_state.fake_sandbox.l402_available',
  spendCapBitcoinSatoshis: 10_000,
  sendReadinessCapacity: 'unknown',
  tokenCacheRef: 'token_cache.local.redacted',
  walletHomeMode: 'unknown',
  walletHomeRef: 'wallet_home.local.mdk_wallet',
  ...overrides,
})

describe('OpenAgents MDK agent-wallet smoke fixture', () => {
  test('builds a documentation-only no-spend plan by default', () => {
    const projection = planOpenAgentsMdkAgentWalletSmoke(baseInput())

    expect(
      S.decodeUnknownSync(OpenAgentsMdkAgentWalletSmokeProjection)(projection),
    ).toEqual(projection)
    expect(projection.status).toBe('documentation_only')
    expect(projection.payoutModeGate).toMatchObject({
      activeMode: 'disabled',
      livePayoutClaimAllowed: false,
      state: 'blocked',
    })
    expect(projection.steps.some(step => step.maySpendBitcoin)).toBe(false)
    expect(projection.steps.map(step => step.kind)).toEqual([
      'status',
      'init_show',
      'send_readiness_preflight',
      'balance',
      'unpaid_challenge',
      'receive',
    ])
    expect(openAgentsMdkAgentWalletSmokeHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('adds bounded signet send and paid retry steps only after operator approval', () => {
    const projection = planOpenAgentsMdkAgentWalletSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        sendReadinessCapacity: 'sufficient',
        walletHomeMode: 'original_funded_wallet_home',
      }),
    )

    expect(projection.status).toBe('ready_for_signet')
    expect(projection.payoutModeGate).toMatchObject({
      activeMode: 'local_mdk_agent_wallet_bridge',
      hostedDirectPayoutClaimAllowed: false,
      localBridgePayoutClaimAllowed: true,
      livePayoutClaimAllowed: true,
      state: 'ready',
    })
    expect(projection.steps.map(step => step.kind)).toEqual([
      'status',
      'init_show',
      'send_readiness_preflight',
      'balance',
      'unpaid_challenge',
      'receive',
      'send',
      'paid_retry',
    ])
    expect(projection.steps.filter(step => step.maySpendBitcoin)).toHaveLength(
      1,
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /lnbc[0-9a-z]+|preimage=|mdk_access_token|word word word/i,
    )
  })

  test('blocks payment plans that exceed the spend cap or lack live authority', () => {
    const overCap = planOpenAgentsMdkAgentWalletSmoke(
      baseInput({
        amountBitcoinSatoshis: 20_000,
        mode: 'signet',
        operatorApprovedPayment: true,
        sendReadinessCapacity: 'sufficient',
        spendCapBitcoinSatoshis: 10_000,
        walletHomeMode: 'original_funded_wallet_home',
      }),
    )
    const liveBlocked = planOpenAgentsMdkAgentWalletSmoke(
      baseInput({
        mode: 'live_blocked',
        operatorApprovedPayment: true,
      }),
    )

    expect(overCap.status).toBe('blocked_by_spend_cap')
    expect(overCap.steps.some(step => step.maySpendBitcoin)).toBe(false)
    expect(liveBlocked.status).toBe('blocked_until_operator_authority')
    expect(liveBlocked.steps.some(step => step.kind === 'send')).toBe(false)
  })

  test('blocks original funded homes until send capacity is public-safe sufficient', () => {
    const projection = planOpenAgentsMdkAgentWalletSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        sendReadinessCapacity: 'insufficient',
        walletHomeMode: 'original_funded_wallet_home',
      }),
    )

    expect(projection.status).toBe('blocked_by_insufficient_capacity')
    expect(projection.sendReadinessCapacity).toBe('insufficient')
    expect(projection.payoutModeGate.blockerRefs).toContain(
      'blocker.mdk_agent_wallet.send_readiness_missing',
    )
    expect(projection.steps.some(step => step.kind === 'send')).toBe(false)
    expect(
      projection.steps.find(step => step.kind === 'send_readiness_preflight')
        ?.noteRefs,
    ).toContain('note.agent_wallet.send_capacity_required_for_amount')
  })

  test('blocks mnemonic-restore mode before any send step', () => {
    const projection = planOpenAgentsMdkAgentWalletSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        walletHomeMode: 'mnemonic_restore',
      }),
    )

    expect(projection.status).toBe('blocked_by_wallet_restore_mode')
    expect(projection.payoutModeGate.blockerRefs).toContain(
      'blocker.mdk_agent_wallet.original_wallet_home_unverified',
    )
    expect(projection.reasonRefs).toEqual([
      'reason.agent_wallet_smoke.blocked_by_wallet_restore_mode',
    ])
    expect(projection.steps.some(step => step.kind === 'send')).toBe(false)
    expect(
      projection.steps.find(step => step.kind === 'send_readiness_preflight')
        ?.noteRefs,
    ).toContain('note.agent_wallet.mnemonic_restore_not_send_ready')
  })

  test('rejects wallet, invoice, preimage, and customer-private material', () => {
    expect(() =>
      planOpenAgentsMdkAgentWalletSmoke(
        baseInput({
          walletHomeRef: '/Users/chris/.mdk-wallet/config.json',
        }),
      ),
    ).toThrow(OpenAgentsMdkAgentWalletSmokeUnsafe)
    expect(() =>
      planOpenAgentsMdkAgentWalletSmoke(
        baseInput({
          endpointRef: 'lnbc10n1rawinvoice',
        }),
      ),
    ).toThrow(OpenAgentsMdkAgentWalletSmokeUnsafe)
    expect(() =>
      planOpenAgentsMdkAgentWalletSmoke(
        baseInput({
          tokenCacheRef: 'payment_preimage=abc123',
        }),
      ),
    ).toThrow(OpenAgentsMdkAgentWalletSmokeUnsafe)
    expect(() =>
      planOpenAgentsMdkAgentWalletSmoke(
        baseInput({
          routeStateRef: 'ben@example.com',
        }),
      ),
    ).toThrow(OpenAgentsMdkAgentWalletSmokeUnsafe)
  })
})
