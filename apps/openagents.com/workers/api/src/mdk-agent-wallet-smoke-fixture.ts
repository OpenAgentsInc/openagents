import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  MdkPayoutModeGateProjection,
  localMdkAgentWalletBridgePayoutGate,
} from './mdk-payout-mode-gate'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from './runner-gateway'

export const OpenAgentsMdkAgentWalletSmokeMode = S.Literals([
  'fake_sandbox',
  'live_blocked',
  'signet',
])
export type OpenAgentsMdkAgentWalletSmokeMode =
  typeof OpenAgentsMdkAgentWalletSmokeMode.Type

export const OpenAgentsMdkAgentWalletSmokeStatus = S.Literals([
  'blocked_by_spend_cap',
  'blocked_by_send_capacity',
  'blocked_by_wallet_restore_mode',
  'blocked_until_operator_authority',
  'documentation_only',
  'ready_for_signet',
])
export type OpenAgentsMdkAgentWalletSmokeStatus =
  typeof OpenAgentsMdkAgentWalletSmokeStatus.Type

export const OpenAgentsMdkAgentWalletSmokeStepKind = S.Literals([
  'balance',
  'init_show',
  'paid_retry',
  'receive',
  'send',
  'send_readiness_preflight',
  'status',
  'unpaid_challenge',
])
export type OpenAgentsMdkAgentWalletSmokeStepKind =
  typeof OpenAgentsMdkAgentWalletSmokeStepKind.Type

export const OpenAgentsMdkAgentWalletSmokeInput = S.Struct({
  amountBitcoinSatoshis: S.Number,
  endpointRef: S.String,
  mode: OpenAgentsMdkAgentWalletSmokeMode,
  operatorApprovedPayment: S.Boolean,
  routeStateRef: S.String,
  sendCapacityRef: S.String,
  sendCapacitySufficient: S.Boolean,
  spendCapBitcoinSatoshis: S.Number,
  tokenCacheRef: S.String,
  walletHomeMode: S.optionalKey(
    S.Literals(['mnemonic_restore', 'original_funded_wallet_home', 'unknown']),
  ),
  walletHomeRef: S.String,
})
export type OpenAgentsMdkAgentWalletSmokeInput =
  typeof OpenAgentsMdkAgentWalletSmokeInput.Type

export const OpenAgentsMdkAgentWalletSmokeStep = S.Struct({
  command: S.Array(S.String),
  expectedJsonShapeRefs: S.Array(S.String),
  kind: OpenAgentsMdkAgentWalletSmokeStepKind,
  maySpendBitcoin: S.Boolean,
  noteRefs: S.Array(S.String),
})
export type OpenAgentsMdkAgentWalletSmokeStep =
  typeof OpenAgentsMdkAgentWalletSmokeStep.Type

export const OpenAgentsMdkAgentWalletSmokeProjection = S.Struct({
  amountBitcoinSatoshis: S.Number,
  endpointRef: S.String,
  mode: OpenAgentsMdkAgentWalletSmokeMode,
  payoutModeGate: MdkPayoutModeGateProjection,
  reasonRefs: S.Array(S.String),
  routeStateRef: S.String,
  sendCapacityRef: S.String,
  sendCapacitySufficient: S.Boolean,
  spendCapBitcoinSatoshis: S.Number,
  status: OpenAgentsMdkAgentWalletSmokeStatus,
  steps: S.Array(OpenAgentsMdkAgentWalletSmokeStep),
  tokenCacheRef: S.String,
  walletHomeRef: S.String,
})
export type OpenAgentsMdkAgentWalletSmokeProjection =
  typeof OpenAgentsMdkAgentWalletSmokeProjection.Type

export class OpenAgentsMdkAgentWalletSmokeUnsafe extends S.TaggedErrorClass<OpenAgentsMdkAgentWalletSmokeUnsafe>()(
  'OpenAgentsMdkAgentWalletSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const publicLiteralValues = new Set([
  '@moneydevkit/agent-wallet@latest',
  '--fail-with-body',
  '-H',
  '<bolt11_invoice_from_402_response>',
  '<l402_token_from_402_response>:<payment_preimage_from_wallet_output>',
  '<l402_token_from_402_response>',
  '<payment_preimage_from_wallet_output>',
  'Authorization: L402 <l402_token_from_402_response>:<payment_preimage_from_wallet_output>',
  'balance',
  'blocked_by_spend_cap',
  'blocked_by_wallet_restore_mode',
  'blocked_until_operator_authority',
  'curl',
  'documentation_only',
  'fake_sandbox',
  'init',
  'init_show',
  'json.agent_wallet.balance_sats',
  'json.agent_wallet.config_redacted',
  'json.agent_wallet.payment_hash_redacted',
  'json.agent_wallet.receive_invoice_redacted',
  'json.agent_wallet.status',
  'json.openagents.l402.payment_required',
  'json.openagents.l402.retry_success_redacted',
  'blocker.mdk_agent_wallet.original_wallet_home_unverified',
  'blocker.mdk_agent_wallet.local_bridge_authority_missing',
  'blocker.mdk_agent_wallet.redaction_boundary_missing',
  'blocker.product_promises.mdk_agent_wallet_send_readiness_insufficient_capacity',
  'blocker.mdk_agent_wallet.send_readiness_missing',
  'caveat.mdk_agent_wallet.local_bridge_not_hosted_direct_payout',
  'caveat.mdk_agent_wallet.original_funded_home_required_for_send',
  'caveat.mdk_agent_wallet.bridge_material_redaction_checked',
  'capacity.mdk_agent_wallet.minimum_not_satisfied',
  'capacity.mdk_agent_wallet.minimum_satisfied',
  'evidence.mdk_agent_wallet.local_bridge_authority_recorded',
  'evidence.mdk_agent_wallet.bridge_material_redaction_checked',
  'evidence.mdk_agent_wallet.send_capacity_minimum_satisfied',
  'evidence.mdk_agent_wallet.send_readiness_preflight_ready',
  'Local MDK agent-wallet bridge',
  'Local MDK agent-wallet bridge disabled',
  'local_mdk_agent_wallet_bridge',
  'disabled',
  'blocked',
  'ready',
  'live_blocked',
  'mnemonic_restore',
  'npx',
  'note.agent_wallet.balance_public_summary_only',
  'note.agent_wallet.daemon_autostart',
  'note.agent_wallet.mnemonic_restore_not_send_ready',
  'note.agent_wallet.mnemonic_must_not_be_printed',
  'note.agent_wallet.operator_approved_signet_only',
  'note.agent_wallet.original_funded_home_required_for_send',
  'note.agent_wallet.receive_for_funding_test_only',
  'note.agent_wallet.send_readiness_preflight_shared',
  'paid_retry',
  'receive',
  'ready_for_signet',
  'restart',
  'send',
  'send_readiness_preflight',
  'signet',
  'status',
  'unpaid_challenge',
  'original_funded_wallet_home',
  'unknown',
  'wallet_home.mdk_agent_wallet.mnemonic_restore',
  'wallet_home.mdk_agent_wallet.original_funded_wallet_home',
  'wallet_home.mdk_agent_wallet.unknown',
])
const unsafeKeyPattern =
  /(access[_-]?token|bearer|checkout[_-]?id|cookie|customer[_-]?(email|name|id)|email[_-]?body|invoice|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|method|preimage|proof)|preimage|private[_-]?key|provider[_-]?(account|grant|payload|token)|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|source[_-]?archive|stripe[_-]?(customer|invoice|payment|secret|webhook)|wallet[_-]?(config|mnemonic|secret|state))/i
const unsafeValuePattern =
  /(\/Users\/|\/home\/|\.mdk-wallet|bearer\s+|checkout_id=|cus_[A-Za-z0-9]+|evt_[A-Za-z0-9]+|gho_[a-z0-9_]+|ghp_[a-z0-9_]+|github[_-]?pat_[a-z0-9_]+|in_[A-Za-z0-9]+|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|method|preimage|proof)|pm_[A-Za-z0-9]+|preimage=[A-Za-z0-9_-]+|provider[_-]?token|raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)|secret|sk-[a-z0-9]|wallet[_-]?(config|mnemonic|secret|state)|whsec_[A-Za-z0-9]+|\S+@\S+)/i

const scanForUnsafeSmokeMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    if (publicLiteralValues.has(value)) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      unsafeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForUnsafeSmokeMaterial(item, [...path, String(index)]),
      )
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) =>
      unsafeKeyPattern.test(key) && key !== 'walletHomeMode'
        ? [...path, key].join('.')
        : scanForUnsafeSmokeMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeSmokeValue = (label: string, value: unknown): void => {
  const unsafePath = scanForUnsafeSmokeMaterial(value)

  if (unsafePath !== undefined) {
    throw new OpenAgentsMdkAgentWalletSmokeUnsafe({
      reason: `${label} contains private or payment-secret material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string): string | undefined =>
  value.trim() !== '' &&
  stableRefPattern.test(value) &&
  scanForUnsafeSmokeMaterial(value) === undefined
    ? value
    : undefined

const safeRefs = (refs: ReadonlyArray<string>): string[] =>
  [...new Set(refs)]
    .map(ref => safeRef(ref))
    .filter((ref): ref is string => ref !== undefined)

const statusForInput = (
  input: OpenAgentsMdkAgentWalletSmokeInput,
): OpenAgentsMdkAgentWalletSmokeStatus =>
  input.amountBitcoinSatoshis > input.spendCapBitcoinSatoshis
    ? 'blocked_by_spend_cap'
    : input.operatorApprovedPayment && input.mode === 'signet'
      ? (input.walletHomeMode ?? 'unknown') === 'original_funded_wallet_home'
        ? input.sendCapacitySufficient
          ? 'ready_for_signet'
          : 'blocked_by_send_capacity'
        : 'blocked_by_wallet_restore_mode'
      : input.mode === 'fake_sandbox'
        ? 'documentation_only'
        : 'blocked_until_operator_authority'

const baseSteps = (
  input: OpenAgentsMdkAgentWalletSmokeInput,
): OpenAgentsMdkAgentWalletSmokeStep[] => [
  {
    command: ['npx', '@moneydevkit/agent-wallet@latest', 'status'],
    expectedJsonShapeRefs: ['json.agent_wallet.status'],
    kind: 'status',
    maySpendBitcoin: false,
    noteRefs: safeRefs(['note.agent_wallet.daemon_autostart']),
  },
  {
    command: ['npx', '@moneydevkit/agent-wallet@latest', 'init', '--show'],
    expectedJsonShapeRefs: ['json.agent_wallet.config_redacted'],
    kind: 'init_show',
    maySpendBitcoin: false,
    noteRefs: safeRefs([
      input.walletHomeRef,
      'note.agent_wallet.mnemonic_must_not_be_printed',
    ]),
  },
  {
    command: ['npx', '@moneydevkit/agent-wallet@latest', 'status'],
    expectedJsonShapeRefs: ['json.agent_wallet.status'],
    kind: 'send_readiness_preflight',
    maySpendBitcoin: false,
    noteRefs: safeRefs([
      `wallet_home.mdk_agent_wallet.${input.walletHomeMode ?? 'unknown'}`,
      'note.agent_wallet.original_funded_home_required_for_send',
      'note.agent_wallet.send_readiness_preflight_shared',
      input.sendCapacitySufficient
        ? 'capacity.mdk_agent_wallet.minimum_satisfied'
        : 'capacity.mdk_agent_wallet.minimum_not_satisfied',
      input.sendCapacityRef,
      ...((input.walletHomeMode ?? 'unknown') === 'mnemonic_restore'
        ? ['note.agent_wallet.mnemonic_restore_not_send_ready']
        : []),
    ]),
  },
  {
    command: ['npx', '@moneydevkit/agent-wallet@latest', 'balance'],
    expectedJsonShapeRefs: ['json.agent_wallet.balance_sats'],
    kind: 'balance',
    maySpendBitcoin: false,
    noteRefs: safeRefs(['note.agent_wallet.balance_public_summary_only']),
  },
  {
    command: ['curl', '--fail-with-body', input.endpointRef],
    expectedJsonShapeRefs: ['json.openagents.l402.payment_required'],
    kind: 'unpaid_challenge',
    maySpendBitcoin: false,
    noteRefs: safeRefs([input.routeStateRef, input.tokenCacheRef]),
  },
  {
    command: [
      'npx',
      '@moneydevkit/agent-wallet@latest',
      'receive',
      String(input.amountBitcoinSatoshis),
    ],
    expectedJsonShapeRefs: ['json.agent_wallet.receive_invoice_redacted'],
    kind: 'receive',
    maySpendBitcoin: false,
    noteRefs: safeRefs(['note.agent_wallet.receive_for_funding_test_only']),
  },
]

const paymentSteps = (
  input: OpenAgentsMdkAgentWalletSmokeInput,
  status: OpenAgentsMdkAgentWalletSmokeStatus,
): OpenAgentsMdkAgentWalletSmokeStep[] =>
  status === 'ready_for_signet'
    ? [
        {
          command: [
            'npx',
            '@moneydevkit/agent-wallet@latest',
            'send',
            '<bolt11_invoice_from_402_response>',
          ],
          expectedJsonShapeRefs: ['json.agent_wallet.payment_hash_redacted'],
          kind: 'send',
          maySpendBitcoin: true,
          noteRefs: safeRefs([
            `spend_cap.bitcoin_satoshis.${input.spendCapBitcoinSatoshis}`,
            'evidence.mdk_agent_wallet.send_capacity_minimum_satisfied',
            'note.agent_wallet.operator_approved_signet_only',
          ]),
        },
        {
          command: [
            'curl',
            '--fail-with-body',
            '-H',
            'Authorization: L402 <l402_token_from_402_response>:<payment_preimage_from_wallet_output>',
            input.endpointRef,
          ],
          expectedJsonShapeRefs: [
            'json.openagents.l402.retry_success_redacted',
          ],
          kind: 'paid_retry',
          maySpendBitcoin: false,
          noteRefs: safeRefs([input.tokenCacheRef]),
        },
      ]
    : []

export const openAgentsMdkAgentWalletSmokeHasPrivateMaterial = (
  value: unknown,
): boolean => scanForUnsafeSmokeMaterial(value) !== undefined

export const planOpenAgentsMdkAgentWalletSmoke = (
  input: OpenAgentsMdkAgentWalletSmokeInput,
): OpenAgentsMdkAgentWalletSmokeProjection => {
  assertSafeSmokeValue('MDK agent-wallet smoke input', input)

  const status = statusForInput(input)
  const payoutModeGate = localMdkAgentWalletBridgePayoutGate({
    operatorApproved:
      input.operatorApprovedPayment && status === 'ready_for_signet',
    sendReady: status === 'ready_for_signet',
    walletHomeMode: input.walletHomeMode ?? 'unknown',
  })
  const projection: OpenAgentsMdkAgentWalletSmokeProjection = {
    amountBitcoinSatoshis: Math.max(0, Math.trunc(input.amountBitcoinSatoshis)),
    endpointRef: safeRef(input.endpointRef) ?? 'endpoint.redacted',
    mode: input.mode,
    payoutModeGate,
    reasonRefs: safeRefs([`reason.agent_wallet_smoke.${status}`]),
    routeStateRef: safeRef(input.routeStateRef) ?? 'route_state.redacted',
    sendCapacityRef: safeRef(input.sendCapacityRef) ?? 'send_capacity.redacted',
    sendCapacitySufficient: input.sendCapacitySufficient,
    spendCapBitcoinSatoshis: Math.max(
      0,
      Math.trunc(input.spendCapBitcoinSatoshis),
    ),
    status,
    steps: [...baseSteps(input), ...paymentSteps(input, status)],
    tokenCacheRef: safeRef(input.tokenCacheRef) ?? 'token_cache.redacted',
    walletHomeRef: safeRef(input.walletHomeRef) ?? 'wallet_home.redacted',
  }

  const unsafeProjectionPath = scanForUnsafeSmokeMaterial(projection)

  if (unsafeProjectionPath !== undefined) {
    throw new OpenAgentsMdkAgentWalletSmokeUnsafe({
      reason: `MDK agent-wallet smoke projection is not public-safe at ${unsafeProjectionPath}.`,
    })
  }

  return projection
}
