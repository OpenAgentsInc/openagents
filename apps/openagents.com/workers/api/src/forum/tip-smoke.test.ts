import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import type { ForumTipSmokeInput } from './tip-smoke'
import {
  ForumTipSmokeProjection,
  ForumTipSmokeUnsafe,
  forumTipSmokeHasPrivateMaterial,
  planForumTipSmoke,
} from './tip-smoke'

const baseInput = (
  overrides: Partial<ForumTipSmokeInput> = {},
): ForumTipSmokeInput => ({
  actorRef: 'actor.alice',
  amountBitcoinSatoshis: 100,
  challengeRef: 'challenge.forum_l402.post_reward.1',
  endpointRef: 'endpoint.forum_paid_action.post_reward',
  idempotencyRef: 'idempotency.forum_tip_smoke.post_reward.1',
  mode: 'fake_sandbox',
  moneyActionRef: 'forum_money_action.redacted.post_reward.1',
  operatorApprovedPayment: false,
  payerWalletReady: true,
  paymentEventRef: 'forum_payment_event.redacted.post_reward.1',
  postRef: 'post.public.forum.tip_smoke.1',
  receiptRef: 'receipt.forum.tip_smoke.1',
  recipientActorRef: 'actor.ben',
  recipientReadinessReady: true,
  recipientReadinessRef: 'readiness.public.forum_tip_recipient.ben',
  redactedEvidenceRef: 'evidence.payment.redacted.forum_tip.1',
  routeStateRef: 'route_state.forum_tip_smoke.l402_available',
  spendCapBitcoinSatoshis: 500,
  tokenCacheRef: 'token_cache.local.redacted.forum_tip_smoke',
  walletHomeMode: 'unknown',
  walletHomeRef: 'wallet_home.local.mdk_wallet',
  ...overrides,
})

const stepKinds = (projection: ReturnType<typeof planForumTipSmoke>) =>
  projection.steps.map(step => step.kind)

describe('Forum tip smoke fixture', () => {
  test('builds a CI-safe fake sandbox no-spend smoke with public-safe refs', () => {
    const projection = planForumTipSmoke(baseInput())

    expect(S.decodeUnknownSync(ForumTipSmokeProjection)(projection)).toEqual(
      projection,
    )
    expect(projection.status).toBe('documentation_only')
    expect(projection.reasonRefs).toEqual([
      'reason.forum_tip_smoke.documentation_only',
      'reason.forum_tip_smoke.fake_sandbox_no_spend',
    ])
    expect(stepKinds(projection)).toEqual([
      'wallet_preflight',
      'recipient_readiness',
      'l402_challenge',
      'private_payment_payload',
      'wallet_payment',
      'paid_retry_redeem',
      'payment_event_linkage',
      'public_receipt_lookup',
      'creator_earnings_lookup',
      'refund_reversal_projection',
      'replay_idempotency',
      'redaction_scan',
    ])
    expect(projection.steps.some(step => step.maySpendBitcoin)).toBe(false)
    expect(
      projection.agentWalletSmoke.steps.some(step => step.maySpendBitcoin),
    ).toBe(false)
    expect(projection.regressionRefs).toEqual([
      'regression.forum_tip.duplicate_provider_event_rejected',
      'regression.forum_tip.duplicate_redemption_idempotent',
      'regression.forum_tip.failed_payment_verification_rejected',
      'regression.forum_tip.insufficient_payer_wallet_readiness',
      'regression.forum_tip.missing_recipient_readiness',
      'regression.forum_tip.over_spend_cap',
      'regression.forum_tip.public_receipt_redacted',
      'regression.forum_tip.stale_challenge_rejected',
    ])
    expect(forumTipSmokeHasPrivateMaterial(projection)).toBe(false)
    const serialized = JSON.stringify(projection)
    expect(serialized).not.toContain('lnbc1privateinvoice')
    expect(serialized).not.toContain('oa-l402-v1.private_token')
    expect(serialized).not.toContain('private_preimage')
    expect(serialized).not.toContain('secret recovery phrase')
    expect(serialized).not.toContain('raw_payout_target_value')
    expect(serialized).not.toContain('/Users/private')
  })

  test('adds a bounded signet wallet payment only with explicit authority', () => {
    const projection = planForumTipSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        sendReadinessCapacityRef:
          'capacity.mdk_agent_wallet.send.sufficient_for_scoped_smoke',
        walletHomeMode: 'original_funded_wallet_home',
      }),
    )

    expect(projection.status).toBe('ready_for_signet')
    expect(projection.reasonRefs).toEqual([
      'reason.forum_tip_smoke.ready_for_signet',
      'reason.forum_tip_smoke.operator_approved_signet_only',
    ])
    expect(projection.steps.filter(step => step.maySpendBitcoin)).toEqual([
      expect.objectContaining({ kind: 'wallet_payment' }),
    ])
    expect(projection.agentWalletSmoke.status).toBe('ready_for_signet')
    expect(projection.agentWalletSmoke.steps.map(step => step.kind)).toEqual([
      'status',
      'init_show',
      'send_readiness_preflight',
      'balance',
      'unpaid_challenge',
      'receive',
      'send',
      'paid_retry',
    ])
  })

  test('blocks mnemonic-restore payer wallet before Forum wallet payment', () => {
    const projection = planForumTipSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        walletHomeMode: 'mnemonic_restore',
      }),
    )

    expect(projection.status).toBe('blocked_until_operator_authority')
    expect(projection.steps.some(step => step.maySpendBitcoin)).toBe(false)
    expect(projection.agentWalletSmoke.status).toBe(
      'blocked_by_wallet_restore_mode',
    )
    expect(projection.agentWalletSmoke.steps.some(step => step.kind === 'send'))
      .toBe(false)
  })

  test('blocks signet spend when payer wallet, recipient readiness, cap, or authority is missing', () => {
    const payerBlocked = planForumTipSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        payerWalletReady: false,
      }),
    )
    const recipientBlocked = planForumTipSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: true,
        recipientReadinessReady: false,
      }),
    )
    const capBlocked = planForumTipSmoke(
      baseInput({
        amountBitcoinSatoshis: 600,
        mode: 'signet',
        operatorApprovedPayment: true,
        spendCapBitcoinSatoshis: 500,
      }),
    )
    const authorityBlocked = planForumTipSmoke(
      baseInput({
        mode: 'signet',
        operatorApprovedPayment: false,
      }),
    )

    expect(payerBlocked.status).toBe('blocked_by_payer_wallet')
    expect(recipientBlocked.status).toBe('blocked_by_recipient_readiness')
    expect(capBlocked.status).toBe('blocked_by_spend_cap')
    expect(authorityBlocked.status).toBe('blocked_until_operator_authority')

    for (const projection of [
      payerBlocked,
      recipientBlocked,
      capBlocked,
      authorityBlocked,
    ]) {
      expect(projection.steps.some(step => step.maySpendBitcoin)).toBe(false)
      expect(
        projection.agentWalletSmoke.steps.some(step => step.maySpendBitcoin),
      ).toBe(false)
    }
  })

  test('projects payment event, money action, receipt, replay, and redaction assertions', () => {
    const projection = planForumTipSmoke(baseInput())

    expect(projection.steps).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_private_payment.authenticated_payer_only',
            'assert.forum_private_payment.binding_fields_match_challenge',
            'assert.forum_private_payment.payload_available_to_payer',
            'assert.forum_private_payment.payload_absent_from_public_projection',
          ]),
          kind: 'private_payment_payload',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_payment_events.inserted',
            'assert.forum_money_actions.payment_event_id_linked',
            'assert.forum_tip_settlement.creator_not_spendable_until_settled',
          ]),
          kind: 'payment_event_linkage',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_receipts.lookup_public_safe',
            'assert.forum_tip_settlement.paid_does_not_claim_creator_settled',
            'assert.forum_receipts.target_post_permalink_public_safe',
          ]),
          kind: 'public_receipt_lookup',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_tip_earnings.creator_projection_public_safe',
            'assert.forum_tip_earnings.direct_post_reward_visible',
            'assert.forum_tip_earnings.receipt_and_post_permalink_refs_present',
          ]),
          kind: 'creator_earnings_lookup',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_tip_settlement.refund_state_public_safe',
            'assert.forum_tip_settlement.reversal_state_public_safe',
            'assert.forum_tip_settlement.no_raw_wallet_or_provider_payload',
          ]),
          kind: 'refund_reversal_projection',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.forum_paid_actions.duplicate_redemption_idempotent',
            'assert.forum_paid_actions.duplicate_provider_event_rejected',
          ]),
          kind: 'replay_idempotency',
        }),
        expect.objectContaining({
          assertionRefs: expect.arrayContaining([
            'assert.public_projection.no_invoice',
            'assert.public_projection.no_l402_token',
            'assert.public_projection.no_mnemonic',
            'assert.public_projection.no_payment_hash',
            'assert.public_projection.no_preimage',
            'assert.public_projection.no_raw_payout_target',
            'assert.public_projection.no_wallet_path',
          ]),
          kind: 'redaction_scan',
        }),
      ]),
    )
  })

  test('rejects wallet, invoice, payment, provider, and raw payout material', () => {
    for (const input of [
      baseInput({ walletHomeRef: '/Users/private/.mdk-wallet/config.json' }),
      baseInput({ redactedEvidenceRef: 'lnbc1rawinvoice' }),
      baseInput({
        paymentEventRef:
          'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
      }),
      baseInput({ recipientActorRef: 'creator@getalby.com' }),
      baseInput({ tokenCacheRef: 'provider_token.private' }),
    ]) {
      expect(() => planForumTipSmoke(input)).toThrow(ForumTipSmokeUnsafe)
    }
  })
})
