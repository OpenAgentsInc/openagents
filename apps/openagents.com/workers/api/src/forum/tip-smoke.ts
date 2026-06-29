import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  OpenAgentsMdkAgentWalletSmokeMode,
  OpenAgentsMdkAgentWalletSmokeProjection,
  openAgentsMdkAgentWalletSmokeHasPrivateMaterial,
  planOpenAgentsMdkAgentWalletSmoke,
} from '../mdk-agent-wallet-smoke-fixture'
import { openAgentsRunnerGatewayPayloadHasPrivateMaterial } from '../runner-gateway'

export const ForumTipSmokeStatus = S.Literals([
  'blocked_by_payer_wallet',
  'blocked_by_recipient_readiness',
  'blocked_by_spend_cap',
  'blocked_until_operator_authority',
  'documentation_only',
  'ready_for_signet',
])
export type ForumTipSmokeStatus = typeof ForumTipSmokeStatus.Type

export const ForumTipSmokeStepKind = S.Literals([
  'creator_earnings_lookup',
  'l402_challenge',
  'paid_retry_redeem',
  'payment_event_linkage',
  'private_payment_payload',
  'public_receipt_lookup',
  'recipient_readiness',
  'redaction_scan',
  'refund_reversal_projection',
  'replay_idempotency',
  'wallet_payment',
  'wallet_preflight',
])
export type ForumTipSmokeStepKind = typeof ForumTipSmokeStepKind.Type

export const ForumTipSmokeInput = S.Struct({
  actorRef: S.String,
  amountBitcoinSatoshis: S.Number,
  challengeRef: S.String,
  endpointRef: S.String,
  idempotencyRef: S.String,
  mode: OpenAgentsMdkAgentWalletSmokeMode,
  moneyActionRef: S.String,
  operatorApprovedPayment: S.Boolean,
  payerWalletReady: S.Boolean,
  paymentEventRef: S.String,
  postRef: S.String,
  receiptRef: S.String,
  recipientActorRef: S.String,
  recipientReadinessReady: S.Boolean,
  recipientReadinessRef: S.String,
  redactedEvidenceRef: S.String,
  routeStateRef: S.String,
  sendCapacityRef: S.optionalKey(S.String),
  sendCapacitySufficient: S.optionalKey(S.Boolean),
  spendCapBitcoinSatoshis: S.Number,
  tokenCacheRef: S.String,
  walletHomeMode: S.optionalKey(
    S.Literals([
      'mnemonic_restore',
      'original_funded_wallet_home',
      'unknown',
    ]),
  ),
  walletHomeRef: S.String,
})
export type ForumTipSmokeInput = typeof ForumTipSmokeInput.Type

export const ForumTipSmokeStep = S.Struct({
  assertionRefs: S.Array(S.String),
  kind: ForumTipSmokeStepKind,
  maySpendBitcoin: S.Boolean,
  sourceRefs: S.Array(S.String),
})
export type ForumTipSmokeStep = typeof ForumTipSmokeStep.Type

export const ForumTipSmokeProjection = S.Struct({
  actorRef: S.String,
  agentWalletSmoke: OpenAgentsMdkAgentWalletSmokeProjection,
  amountBitcoinSatoshis: S.Number,
  mode: OpenAgentsMdkAgentWalletSmokeMode,
  postRef: S.String,
  reasonRefs: S.Array(S.String),
  recipientActorRef: S.String,
  regressionRefs: S.Array(S.String),
  spendCapBitcoinSatoshis: S.Number,
  status: ForumTipSmokeStatus,
  steps: S.Array(ForumTipSmokeStep),
})
export type ForumTipSmokeProjection = typeof ForumTipSmokeProjection.Type

export class ForumTipSmokeUnsafe extends S.TaggedErrorClass<ForumTipSmokeUnsafe>()(
  'ForumTipSmokeUnsafe',
  {
    reason: S.String,
  },
) {}

const stableRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const rawHexMaterialPattern = /^[a-f0-9]{64}$/i
const unsafeForumTipSmokeKeyPattern =
  /(access[_-]?token|bearer|bolt11|bolt12|invoice|lightning[_-]?address|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage|proof|raw|secret)|payout[_-]?(address|destination|raw|target)|preimage|private[_-]?key|provider[_-]?(secret|token)|raw[_-]?(invoice|payment|payout|payload|webhook)|secret|wallet[_-]?(config|mnemonic|path|secret|state)|webhook[_-]?secret)/i
const unsafeForumTipSmokeValuePattern =
  /(\/Users\/|\/home\/|\.mdk-wallet|bearer\s+|checkout_id=|lnbc|lntb|lnbcrt|lno1|lnurl|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?preimage|preimage=|private[_-]?key|provider[_-]?token|raw[_-]?(invoice|payment|payout|payload|webhook)|secret|sk-[a-z0-9]|wallet[_-]?(config|mnemonic|path|secret|state)|whsec_|\S+@\S+)/i
const publicForumTipSmokeLiteralValues = new Set([
  'mnemonic_restore',
  'original_funded_wallet_home',
  'unknown',
])

const uniqueRefs = (refs: ReadonlyArray<string>): string[] => [
  ...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== '')),
]

const scanForumTipSmokeUnsafeMaterial = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): string | undefined => {
  if (typeof value === 'string') {
    if (
      /^(assert|reason|regression)\./.test(value) ||
      publicForumTipSmokeLiteralValues.has(value)
    ) {
      return undefined
    }

    return containsProviderSecretMaterial(value) ||
      rawHexMaterialPattern.test(value) ||
      unsafeForumTipSmokeValuePattern.test(value) ||
      openAgentsRunnerGatewayPayloadHasPrivateMaterial(value)
      ? path.join('.') || '<root>'
      : undefined
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        scanForumTipSmokeUnsafeMaterial(item, [...path, String(index)]),
      )
      .find((unsafePath): unsafePath is string => unsafePath !== undefined)
  }

  if (value === null || typeof value !== 'object') {
    return undefined
  }

  return Object.entries(value)
    .map(([key, item]) =>
      key === 'agentWalletSmoke'
        ? openAgentsMdkAgentWalletSmokeHasPrivateMaterial(item)
          ? [...path, key].join('.')
          : undefined
        : unsafeForumTipSmokeKeyPattern.test(key) && key !== 'walletHomeMode'
          ? [...path, key].join('.')
          : scanForumTipSmokeUnsafeMaterial(item, [...path, key]),
    )
    .find((unsafePath): unsafePath is string => unsafePath !== undefined)
}

const assertSafeForumTipSmokeValue = (label: string, value: unknown): void => {
  const unsafePath = scanForumTipSmokeUnsafeMaterial(value)

  if (unsafePath !== undefined) {
    throw new ForumTipSmokeUnsafe({
      reason: `${label} contains private wallet, payout, provider, invoice, token, preimage, payment, or raw target material at ${unsafePath}.`,
    })
  }
}

const safeRef = (value: string, fallback: string): string =>
  stableRefPattern.test(value.trim()) ? value.trim() : fallback

const statusForInput = (
  input: ForumTipSmokeInput,
  agentWalletStatus:
    | 'blocked_by_spend_cap'
    | 'blocked_by_send_capacity'
    | 'blocked_by_wallet_restore_mode'
    | 'blocked_until_operator_authority'
    | 'documentation_only'
    | 'ready_for_signet',
): ForumTipSmokeStatus => {
  if (input.amountBitcoinSatoshis > input.spendCapBitcoinSatoshis) {
    return 'blocked_by_spend_cap'
  }

  if (!input.payerWalletReady) {
    return 'blocked_by_payer_wallet'
  }

  if (!input.recipientReadinessReady) {
    return 'blocked_by_recipient_readiness'
  }

  if (agentWalletStatus === 'ready_for_signet') {
    return 'ready_for_signet'
  }

  return input.mode === 'fake_sandbox'
    ? 'documentation_only'
    : 'blocked_until_operator_authority'
}

const regressionRefs: ReadonlyArray<string> = [
  'regression.forum_tip.duplicate_provider_event_rejected',
  'regression.forum_tip.duplicate_redemption_idempotent',
  'regression.forum_tip.failed_payment_verification_rejected',
  'regression.forum_tip.insufficient_payer_wallet_readiness',
  'regression.forum_tip.missing_recipient_readiness',
  'regression.forum_tip.over_spend_cap',
  'regression.forum_tip.public_receipt_redacted',
  'regression.forum_tip.stale_challenge_rejected',
]

const smokeSteps = (
  input: ForumTipSmokeInput,
  status: ForumTipSmokeStatus,
): ForumTipSmokeStep[] => [
  {
    assertionRefs: [
      'assert.agent_wallet.status_json',
      'assert.agent_wallet.init_show_mnemonic_redacted',
      'assert.agent_wallet.balance_json',
    ],
    kind: 'wallet_preflight',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.walletHomeRef,
      'npx.moneydevkit.agent_wallet.status',
      'npx.moneydevkit.agent_wallet.init_show',
      'npx.moneydevkit.agent_wallet.balance',
    ]),
  },
  {
    assertionRefs: [
      input.recipientReadinessReady
        ? 'assert.forum_tip.recipient_readiness_ready'
        : 'assert.forum_tip.recipient_readiness_blocks_payment',
    ],
    kind: 'recipient_readiness',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.recipientActorRef,
      input.recipientReadinessRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_l402.challenge_issued',
      'assert.forum_l402.www_authenticate_public_safe',
      'assert.forum_l402.raw_payment_material_absent',
    ],
    kind: 'l402_challenge',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.challengeRef,
      input.endpointRef,
      input.idempotencyRef,
      input.routeStateRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_private_payment.authenticated_payer_only',
      'assert.forum_private_payment.binding_fields_match_challenge',
      'assert.forum_private_payment.payload_available_to_payer',
      'assert.forum_private_payment.payload_absent_from_public_projection',
    ],
    kind: 'private_payment_payload',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.actorRef,
      input.challengeRef,
      'endpoint.forum_paid_actions.private_payment',
      input.tokenCacheRef,
    ]),
  },
  {
    assertionRefs: [
      status === 'ready_for_signet'
        ? 'assert.agent_wallet.send_signet_under_spend_cap'
        : 'assert.agent_wallet.no_send_without_ready_signet_authority',
    ],
    kind: 'wallet_payment',
    maySpendBitcoin: status === 'ready_for_signet',
    sourceRefs: uniqueRefs([
      `spend_cap.bitcoin_satoshis.${Math.max(
        0,
        Math.trunc(input.spendCapBitcoinSatoshis),
      )}`,
      input.tokenCacheRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_paid_actions.redeem_accepts_verified_payment_event',
      'assert.forum_paid_actions.redeem_rejects_failed_verification',
      'assert.forum_paid_actions.redeem_rejects_stale_challenge',
    ],
    kind: 'paid_retry_redeem',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.challengeRef,
      input.redactedEvidenceRef,
      input.tokenCacheRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_payment_events.inserted',
      'assert.forum_money_actions.payment_event_id_linked',
      'assert.forum_tip_settlement.creator_not_spendable_until_settled',
    ],
    kind: 'payment_event_linkage',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([input.moneyActionRef, input.paymentEventRef]),
  },
  {
    assertionRefs: [
      'assert.forum_receipts.lookup_public_safe',
      'assert.forum_tip_settlement.paid_does_not_claim_creator_settled',
      'assert.forum_receipts.target_post_permalink_public_safe',
    ],
    kind: 'public_receipt_lookup',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([input.postRef, input.receiptRef]),
  },
  {
    assertionRefs: [
      'assert.forum_tip_earnings.creator_projection_public_safe',
      'assert.forum_tip_earnings.direct_post_reward_visible',
      'assert.forum_tip_earnings.receipt_and_post_permalink_refs_present',
    ],
    kind: 'creator_earnings_lookup',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.moneyActionRef,
      input.postRef,
      input.receiptRef,
      input.recipientActorRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_tip_settlement.refund_state_public_safe',
      'assert.forum_tip_settlement.reversal_state_public_safe',
      'assert.forum_tip_settlement.no_raw_wallet_or_provider_payload',
    ],
    kind: 'refund_reversal_projection',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([
      input.moneyActionRef,
      input.paymentEventRef,
      input.receiptRef,
    ]),
  },
  {
    assertionRefs: [
      'assert.forum_paid_actions.duplicate_redemption_idempotent',
      'assert.forum_paid_actions.duplicate_provider_event_rejected',
    ],
    kind: 'replay_idempotency',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([input.challengeRef, input.idempotencyRef]),
  },
  {
    assertionRefs: [
      'assert.public_projection.no_invoice',
      'assert.public_projection.no_l402_token',
      'assert.public_projection.no_mnemonic',
      'assert.public_projection.no_payment_hash',
      'assert.public_projection.no_preimage',
      'assert.public_projection.no_provider_secret',
      'assert.public_projection.no_raw_payout_target',
      'assert.public_projection.no_wallet_path',
    ],
    kind: 'redaction_scan',
    maySpendBitcoin: false,
    sourceRefs: uniqueRefs([input.receiptRef, input.redactedEvidenceRef]),
  },
]

export const forumTipSmokeHasPrivateMaterial = (value: unknown): boolean =>
  scanForumTipSmokeUnsafeMaterial(value) !== undefined

export const planForumTipSmoke = (
  input: ForumTipSmokeInput,
): ForumTipSmokeProjection => {
  assertSafeForumTipSmokeValue('Forum tip smoke input', input)

  const amountBitcoinSatoshis = Math.max(
    0,
    Math.trunc(input.amountBitcoinSatoshis),
  )
  const spendCapBitcoinSatoshis = Math.max(
    0,
    Math.trunc(input.spendCapBitcoinSatoshis),
  )
  const higherLevelPrerequisitesReady =
    input.payerWalletReady &&
    input.recipientReadinessReady &&
    amountBitcoinSatoshis <= spendCapBitcoinSatoshis
  const agentWalletSmoke = planOpenAgentsMdkAgentWalletSmoke({
    amountBitcoinSatoshis,
    endpointRef: input.endpointRef,
    mode: input.mode,
    operatorApprovedPayment:
      input.operatorApprovedPayment && higherLevelPrerequisitesReady,
    routeStateRef: input.routeStateRef,
    sendCapacityRef:
      input.sendCapacityRef ?? 'capacity.mdk_agent_wallet.minimum_not_satisfied',
    sendCapacitySufficient: input.sendCapacitySufficient ?? false,
    spendCapBitcoinSatoshis,
    tokenCacheRef: input.tokenCacheRef,
    walletHomeMode: input.walletHomeMode ?? 'unknown',
    walletHomeRef: input.walletHomeRef,
  })
  const status = statusForInput(input, agentWalletSmoke.status)
  const projection: ForumTipSmokeProjection = {
    actorRef: safeRef(input.actorRef, 'actor.forum_tip_smoke.redacted'),
    agentWalletSmoke,
    amountBitcoinSatoshis,
    mode: input.mode,
    postRef: safeRef(input.postRef, 'post.forum_tip_smoke.redacted'),
    reasonRefs: uniqueRefs([
      `reason.forum_tip_smoke.${status}`,
      status === 'documentation_only'
        ? 'reason.forum_tip_smoke.fake_sandbox_no_spend'
        : '',
      status === 'ready_for_signet'
        ? 'reason.forum_tip_smoke.operator_approved_signet_only'
        : '',
    ]),
    recipientActorRef: safeRef(
      input.recipientActorRef,
      'actor.forum_tip_recipient.redacted',
    ),
    regressionRefs: [...regressionRefs],
    spendCapBitcoinSatoshis,
    status,
    steps: smokeSteps(input, status),
  }

  assertSafeForumTipSmokeValue('Forum tip smoke projection', projection)

  return projection
}
