import { describe, expect, test } from 'vitest'

import {
  type ForumTipRecipientWalletRecord,
  ForumTipRecipientWalletUnsafe,
  assertForumTipRecipientWalletRecordSafe,
  forumTipRecipientReadinessIsSafe,
  forumTipRecipientWalletRecordHasPrivateMaterial,
  missingForumTipRecipientReadiness,
  projectForumTipRecipientReadiness,
} from './recipient-wallet-readiness'

const readyRecord = (
  overrides: Partial<ForumTipRecipientWalletRecord> = {},
): ForumTipRecipientWalletRecord => ({
  actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  bolt12Offer: 'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
  lightningAddress: null,
  caveatRefs: ['caveat.public.forum_tip_recipient.claim_required'],
  claimPolicyRefs: ['policy.public.forum_tip_recipient.agent_claimed'],
  custodyPolicyRefs: ['policy.public.forum_tip_recipient.self_custody'],
  disabledAt: null,
  id: 'forum_tip_recipient_wallet_1',
  payoutTargetApprovalRef: 'approval.public.forum_tip_recipient.agent_aaaaaaaa',
  providerClass: 'mdk_agent_wallet',
  readinessRefs: ['readiness.public.forum_tip_recipient.receive_ready'],
  receiveCapabilityRef:
    'receive_capability.public.forum_tip_recipient.agent_aaaaaaaa',
  sourceRef: 'source.public.pylon_api_registration.agent_aaaaaaaa',
  state: 'ready',
  walletRef: 'wallet.public.forum_tip_recipient.agent_aaaaaaaa',
  ...overrides,
})

describe('Forum tip recipient wallet readiness', () => {
  test('projects a ready recipient without exposing raw wallet material', () => {
    const projection = projectForumTipRecipientReadiness(readyRecord())
    const serialized = JSON.stringify(projection)

    expect(projection).toMatchObject({
      actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
      blockerRef: null,
      directPayment: {
        bolt12Offer:
          'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
        kind: 'bolt12_offer',
        settlementAuthority: 'recipient_wallet_direct',
      },
      providerClass: 'mdk_agent_wallet',
      readinessRefs: ['readiness.public.forum_tip_recipient.receive_ready'],
      state: 'ready',
      tippingAvailable: true,
    })
    expect(forumTipRecipientReadinessIsSafe(projection)).toBe(true)
    expect(serialized).not.toContain('wallet.public.forum_tip_recipient')
    expect(serialized).not.toContain('receive_capability.public')
    expect(serialized).not.toContain('approval.public.forum_tip_recipient')
  })

  test('projects a static Lightning Address fallback alongside the BOLT 12 offer (#5078)', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({ lightningAddress: 'oab38ad12345abcd9@spark.money' }),
    )

    expect(projection.directPayment).toMatchObject({
      bolt12Offer: 'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
      lightningAddress: 'oab38ad12345abcd9@spark.money',
      kind: 'bolt12_offer',
      settlementAuthority: 'recipient_wallet_direct',
    })
    expect(forumTipRecipientReadinessIsSafe(projection)).toBe(true)
    expect(projection.tippingAvailable).toBe(true)
  })

  test('a ready recipient with only a Lightning Address (no BOLT 12) projects no directPayment', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: null,
        lightningAddress: 'oab38ad12345abcd9@spark.money',
      }),
    )
    // directPayment is gated on the BOLT 12 online rail; the lightning address
    // is a payout fallback held on file, not a standalone tip-payable rail.
    expect(projection.directPayment).toBeNull()
    // The raw lightning address never leaks into the public readiness when
    // there is no directPayment to carry it.
    expect(JSON.stringify(projection)).not.toContain('@spark.money')
  })

  test('rejects a Lightning Address field carrying a raw invoice or wallet secret (#5078)', () => {
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({ lightningAddress: 'lnbc1privateinvoice' }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({ lightningAddress: 'not a lightning address' }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
  })

  test('names the daemon-reachability constraint on every direct-payment projection', () => {
    const withOffer = projectForumTipRecipientReadiness(readyRecord())

    expect(withOffer.directPayment?.kind).toBe('bolt12_offer')
    expect(withOffer.caveatRefs).toContain(
      'caveat.public.forum_tip_recipient.daemon_reachability_required',
    )

    const withoutOffer = projectForumTipRecipientReadiness(
      readyRecord({ bolt12Offer: null }),
    )

    expect(withoutOffer.directPayment).toBeNull()
    expect(withoutOffer.caveatRefs).not.toContain(
      'caveat.public.forum_tip_recipient.daemon_reachability_required',
    )
  })

  test('represents missing readiness as a first-class public blocker', () => {
    expect(missingForumTipRecipientReadiness('actor.missing')).toStrictEqual({
      actorRef: 'actor.missing',
      blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
      caveatRefs: ['caveat.public.forum_tip_recipient.wallet_not_admitted'],
      directPayment: null,
      providerClass: null,
      readinessRefs: [],
      sourceRef: 'forum_tip_recipient_wallets',
      state: 'missing',
      tippingAvailable: false,
    })
  })

  test('projects disabled and blocked recipients as unavailable', () => {
    expect(
      projectForumTipRecipientReadiness(
        readyRecord({
          caveatRefs: ['caveat.public.forum_tip_recipient.owner_disabled'],
          disabledAt: '2026-06-07T10:00:00.000Z',
          state: 'disabled',
        }),
      ),
    ).toMatchObject({
      blockerRef: 'blocker.public.forum_tip_recipient.wallet_disabled',
      readinessRefs: [],
      state: 'disabled',
      tippingAvailable: false,
    })
    expect(
      projectForumTipRecipientReadiness(
        readyRecord({
          caveatRefs: ['caveat.public.forum_tip_recipient.actor_blocked'],
          state: 'blocked',
        }),
      ),
    ).toMatchObject({
      blockerRef: 'blocker.public.forum_tip_recipient.actor_blocked',
      readinessRefs: [],
      state: 'blocked',
      tippingAvailable: false,
    })
  })

  test('rejects raw wallet, invoice, preimage, payout, provider, path, and secret material', () => {
    const unsafeRecords = [
      readyRecord({ walletRef: 'wallet.secret.seed' }),
      readyRecord({ receiveCapabilityRef: 'lnbc1privateinvoice' }),
      readyRecord({ readinessRefs: ['payment_hash=abc123'] }),
      readyRecord({ caveatRefs: ['preimage=abc123'] }),
      readyRecord({ payoutTargetApprovalRef: 'payout_address.bc1qprivate' }),
      readyRecord({ sourceRef: '/Users/private/.mdk-wallet/config.json' }),
      readyRecord({ claimPolicyRefs: ['provider_token.private'] }),
      readyRecord({ custodyPolicyRefs: ['secret.recovery_phrase'] }),
    ]

    for (const record of unsafeRecords) {
      expect(forumTipRecipientWalletRecordHasPrivateMaterial(record)).toBe(true)
      expect(() => assertForumTipRecipientWalletRecordSafe(record)).toThrow(
        ForumTipRecipientWalletUnsafe,
      )
    }

    expect(
      forumTipRecipientWalletRecordHasPrivateMaterial(
        readyRecord({ bolt12Offer: 'lnbc1privateinvoice' }),
      ),
    ).toBe(false)
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({ bolt12Offer: 'lnbc1privateinvoice' }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
  })
})
