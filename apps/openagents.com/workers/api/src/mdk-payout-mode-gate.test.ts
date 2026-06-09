import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MdkPayoutModeGateProjection,
  hostedMdkDirectPayoutDisabledGate,
  hostedMdkSandboxPayoutGate,
  localMdkAgentWalletBridgePayoutGate,
  projectMdkPayoutModeGate,
} from './mdk-payout-mode-gate'

describe('MDK payout mode gate', () => {
  test('blocks hosted direct payouts when programmatic payouts are disabled', () => {
    const gate = hostedMdkDirectPayoutDisabledGate()

    expect(S.decodeUnknownSync(MdkPayoutModeGateProjection)(gate)).toEqual(gate)
    expect(gate).toMatchObject({
      activeMode: 'disabled',
      hostedDirectPayoutClaimAllowed: false,
      livePayoutClaimAllowed: false,
      state: 'blocked',
    })
    expect(gate.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.mdk.hosted_programmatic_payouts_disabled',
        'blocker.mdk.hosted_funded_key_unverified',
      ]),
    )
  })

  test('allows hosted direct payout claims only with enabled payouts and verified funded key', () => {
    const gate = projectMdkPayoutModeGate({
      hostedFundedKeyVerified: true,
      hostedProgrammaticPayoutsEnabled: true,
      requestedMode: 'hosted_mdk_direct_payout',
    })

    expect(gate).toMatchObject({
      activeMode: 'hosted_mdk_direct_payout',
      hostedDirectPayoutClaimAllowed: true,
      livePayoutClaimAllowed: true,
      state: 'ready',
    })
    expect(gate.blockerRefs).toEqual([])
    expect(gate.evidenceRefs).toEqual(
      expect.arrayContaining([
        'evidence.mdk.hosted_programmatic_payouts_enabled',
        'evidence.mdk.hosted_funded_key_verified',
      ]),
    )
  })

  test('keeps hosted enabled sandbox evidence separate from live payout claims', () => {
    const gate = hostedMdkSandboxPayoutGate()

    expect(gate).toMatchObject({
      activeMode: 'disabled',
      hostedDirectPayoutClaimAllowed: false,
      livePayoutClaimAllowed: false,
      modeLabel: 'Hosted MDK sandbox fixture only',
      state: 'sandbox_ready',
    })
    expect(gate.evidenceRefs).toContain(
      'evidence.mdk.hosted_sandbox_fixture_verified',
    )
  })

  test('allows the local bridge only after send readiness, original wallet home, and redaction guards', () => {
    const blocked = localMdkAgentWalletBridgePayoutGate({
      operatorApproved: true,
      sendReady: true,
      walletHomeMode: 'mnemonic_restore',
    })
    const ready = localMdkAgentWalletBridgePayoutGate({
      operatorApproved: true,
      sendReady: true,
      walletHomeMode: 'original_funded_wallet_home',
    })

    expect(blocked).toMatchObject({
      activeMode: 'disabled',
      localBridgePayoutClaimAllowed: false,
      state: 'blocked',
    })
    expect(blocked.blockerRefs).toContain(
      'blocker.mdk_agent_wallet.original_wallet_home_unverified',
    )
    expect(ready).toMatchObject({
      activeMode: 'local_mdk_agent_wallet_bridge',
      hostedDirectPayoutClaimAllowed: false,
      localBridgePayoutClaimAllowed: true,
      livePayoutClaimAllowed: true,
      state: 'ready',
    })
    expect(ready.caveatRefs).toContain(
      'caveat.mdk_agent_wallet.local_bridge_not_hosted_direct_payout',
    )
  })
})
