import { Schema as S } from 'effect'

import { MdkAgentWalletHomeMode } from './treasury-payment-mdk-agent-wallet-adapter'

export const MdkPayoutMode = S.Literals([
  'disabled',
  'hosted_mdk_direct_payout',
  'local_mdk_agent_wallet_bridge',
])
export type MdkPayoutMode = typeof MdkPayoutMode.Type

export const MdkPayoutModeGateState = S.Literals([
  'blocked',
  'ready',
  'sandbox_ready',
])
export type MdkPayoutModeGateState = typeof MdkPayoutModeGateState.Type

export const MdkPayoutModeGateInput = S.Struct({
  bridgeRawPaymentMaterialRedacted: S.optionalKey(S.Boolean),
  hostedFundedKeyVerified: S.Boolean,
  hostedProgrammaticPayoutsEnabled: S.Boolean,
  hostedSandboxVerified: S.optionalKey(S.Boolean),
  localBridgeOperatorApproved: S.optionalKey(S.Boolean),
  localBridgeSendReady: S.optionalKey(S.Boolean),
  localBridgeWalletHomeMode: S.optionalKey(MdkAgentWalletHomeMode),
  requestedMode: MdkPayoutMode,
})
export type MdkPayoutModeGateInput = typeof MdkPayoutModeGateInput.Type

export const MdkPayoutModeGateProjection = S.Struct({
  activeMode: MdkPayoutMode,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  hostedDirectPayoutClaimAllowed: S.Boolean,
  localBridgePayoutClaimAllowed: S.Boolean,
  livePayoutClaimAllowed: S.Boolean,
  modeLabel: S.String,
  state: MdkPayoutModeGateState,
})
export type MdkPayoutModeGateProjection =
  typeof MdkPayoutModeGateProjection.Type

const uniqueRefs = (refs: ReadonlyArray<string>): string[] => [
  ...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== '')),
]

const hostedBlockers = (
  input: MdkPayoutModeGateInput,
): ReadonlyArray<string> => [
  ...(input.hostedProgrammaticPayoutsEnabled
    ? []
    : ['blocker.mdk.hosted_programmatic_payouts_disabled']),
  ...(input.hostedFundedKeyVerified
    ? []
    : ['blocker.mdk.hosted_funded_key_unverified']),
]

const localBridgeBlockers = (
  input: MdkPayoutModeGateInput,
): ReadonlyArray<string> => [
  ...(input.localBridgeOperatorApproved === true
    ? []
    : ['blocker.mdk_agent_wallet.local_bridge_authority_missing']),
  ...(input.localBridgeSendReady === true
    ? []
    : ['blocker.mdk_agent_wallet.send_readiness_missing']),
  ...((input.localBridgeWalletHomeMode ?? 'unknown') ===
  'original_funded_wallet_home'
    ? []
    : ['blocker.mdk_agent_wallet.original_wallet_home_unverified']),
  ...(input.bridgeRawPaymentMaterialRedacted === false
    ? ['blocker.mdk_agent_wallet.redaction_boundary_missing']
    : []),
]

export const projectMdkPayoutModeGate = (
  input: MdkPayoutModeGateInput,
): MdkPayoutModeGateProjection => {
  const hostedRefs = hostedBlockers(input)
  const localRefs = localBridgeBlockers(input)
  const hostedReady = hostedRefs.length === 0
  const localBridgeReady = localRefs.length === 0

  if (input.requestedMode === 'hosted_mdk_direct_payout') {
    return {
      activeMode: hostedReady ? 'hosted_mdk_direct_payout' : 'disabled',
      blockerRefs: uniqueRefs(hostedRefs),
      caveatRefs: uniqueRefs([
        'caveat.mdk.hosted_direct_requires_enabled_programmatic_payouts',
        'caveat.mdk.hosted_direct_requires_verified_funded_key',
      ]),
      evidenceRefs: uniqueRefs([
        ...(hostedReady
          ? [
              'evidence.mdk.hosted_programmatic_payouts_enabled',
              'evidence.mdk.hosted_funded_key_verified',
            ]
          : []),
      ]),
      hostedDirectPayoutClaimAllowed: hostedReady,
      localBridgePayoutClaimAllowed: false,
      livePayoutClaimAllowed: hostedReady,
      modeLabel: hostedReady
        ? 'Hosted MDK direct payout'
        : 'Hosted MDK direct payout disabled',
      state: hostedReady ? 'ready' : 'blocked',
    }
  }

  if (input.requestedMode === 'local_mdk_agent_wallet_bridge') {
    return {
      activeMode: localBridgeReady
        ? 'local_mdk_agent_wallet_bridge'
        : 'disabled',
      blockerRefs: uniqueRefs(localRefs),
      caveatRefs: uniqueRefs([
        'caveat.mdk_agent_wallet.local_bridge_not_hosted_direct_payout',
        'caveat.mdk_agent_wallet.bridge_material_redaction_checked',
        'caveat.mdk_agent_wallet.original_funded_home_required_for_send',
      ]),
      evidenceRefs: uniqueRefs([
        ...(localBridgeReady
          ? [
              'evidence.mdk_agent_wallet.local_bridge_authority_recorded',
              'evidence.mdk_agent_wallet.send_readiness_preflight_ready',
              'evidence.mdk_agent_wallet.bridge_material_redaction_checked',
            ]
          : []),
      ]),
      hostedDirectPayoutClaimAllowed: false,
      localBridgePayoutClaimAllowed: localBridgeReady,
      livePayoutClaimAllowed: localBridgeReady,
      modeLabel: localBridgeReady
        ? 'Local MDK agent-wallet bridge'
        : 'Local MDK agent-wallet bridge disabled',
      state: localBridgeReady ? 'ready' : 'blocked',
    }
  }

  return {
    activeMode: 'disabled',
    blockerRefs: uniqueRefs([
      'blocker.mdk.payout_mode_disabled',
      ...hostedRefs,
      ...localRefs,
    ]),
    caveatRefs: uniqueRefs(['caveat.mdk.no_live_payout_mode_selected']),
    evidenceRefs: uniqueRefs([
      ...(input.hostedSandboxVerified === true
        ? ['evidence.mdk.hosted_sandbox_fixture_verified']
        : []),
    ]),
    hostedDirectPayoutClaimAllowed: false,
    localBridgePayoutClaimAllowed: false,
    livePayoutClaimAllowed: false,
    modeLabel:
      input.hostedSandboxVerified === true
        ? 'Hosted MDK sandbox fixture only'
        : 'MDK payouts disabled',
    state: input.hostedSandboxVerified === true ? 'sandbox_ready' : 'blocked',
  }
}

export const hostedMdkDirectPayoutDisabledGate =
  (): MdkPayoutModeGateProjection =>
    projectMdkPayoutModeGate({
      hostedFundedKeyVerified: false,
      hostedProgrammaticPayoutsEnabled: false,
      requestedMode: 'hosted_mdk_direct_payout',
    })

export const hostedMdkSandboxPayoutGate = (): MdkPayoutModeGateProjection =>
  projectMdkPayoutModeGate({
    hostedFundedKeyVerified: true,
    hostedProgrammaticPayoutsEnabled: false,
    hostedSandboxVerified: true,
    requestedMode: 'disabled',
  })

export const localMdkAgentWalletBridgePayoutGate = (input: {
  operatorApproved: boolean
  sendReady: boolean
  walletHomeMode: typeof MdkAgentWalletHomeMode.Type
}): MdkPayoutModeGateProjection =>
  projectMdkPayoutModeGate({
    bridgeRawPaymentMaterialRedacted: true,
    hostedFundedKeyVerified: false,
    hostedProgrammaticPayoutsEnabled: false,
    localBridgeOperatorApproved: input.operatorApproved,
    localBridgeSendReady: input.sendReady,
    localBridgeWalletHomeMode: input.walletHomeMode,
    requestedMode: 'local_mdk_agent_wallet_bridge',
  })
