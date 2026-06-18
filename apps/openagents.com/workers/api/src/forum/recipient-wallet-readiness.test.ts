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

const SPARK_ADDRESS =
  'spark1pgssyuuuhnrrdjswal5c3s3rafw9w3y5dd4cjy3duxlf7hjzkp0rqx6dj6mrhu'

const readyRecord = (
  overrides: Partial<ForumTipRecipientWalletRecord> = {},
): ForumTipRecipientWalletRecord => ({
  actorRef: 'agent:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
  sparkAddress: null,
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

  test('a ready recipient with only a native Spark address is tippable Spark→Spark (#5345)', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: null,
        lightningAddress: null,
        sparkAddress: SPARK_ADDRESS,
      }),
    )

    expect(projection.directPayment).toMatchObject({
      sparkAddress: SPARK_ADDRESS,
      kind: 'spark_address',
      settlementAuthority: 'recipient_wallet_direct',
    })
    expect(projection.tippingAvailable).toBe(true)
    expect(projection.blockerRef).toBe(null)
    // The Spark rail is offline-receive: no LSP, no LN-address claim required.
    expect(projection.caveatRefs).toContain(
      'caveat.public.forum_tip_recipient.spark_offline_receive',
    )
    expect(projection.caveatRefs).not.toContain(
      'caveat.public.forum_tip_recipient.spark_lightning_address_claim_required',
    )
    expect(forumTipRecipientReadinessIsSafe(projection)).toBe(true)
  })

  test('prefers the native Spark address over Lightning rails (#5345)', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: 'lno1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3j',
        lightningAddress: 'oab38ad12345abcd9@spark.money',
        sparkAddress: SPARK_ADDRESS,
      }),
    )

    expect(projection.directPayment?.kind).toBe('spark_address')
    expect(projection.tippingAvailable).toBe(true)
  })

  test('rejects a Spark-address field carrying a raw invoice or wallet secret (#5345)', () => {
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({ sparkAddress: 'lnbc1privateinvoice' }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({ sparkAddress: 'not a spark address' }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
    // A Lightning Address must not be accepted as a native Spark address.
    expect(() =>
      assertForumTipRecipientWalletRecordSafe(
        readyRecord({
          bolt12Offer: null,
          sparkAddress: 'oab38ad12345abcd9@spark.money',
        }),
      ),
    ).toThrow(ForumTipRecipientWalletUnsafe)
  })

  test('keeps the native Spark address out of generic public refs (redaction, #5345)', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: null,
        sparkAddress: SPARK_ADDRESS,
      }),
    )
    const serialized = JSON.stringify(projection)

    // The Spark address is a public tip destination: it appears only inside the
    // typed directPayment instruction, never smuggled into refs, and no seed or
    // wallet material leaks.
    expect(serialized).toContain(SPARK_ADDRESS)
    expect(projection.directPayment).toMatchObject({ kind: 'spark_address' })
    expect(serialized).not.toContain('wallet.public.forum_tip_recipient')
    expect(serialized).not.toContain('mnemonic')
    expect(serialized).not.toContain('seed')
    expect(
      forumTipRecipientWalletRecordHasPrivateMaterial(
        readyRecord({ bolt12Offer: null, sparkAddress: SPARK_ADDRESS }),
      ),
    ).toBe(false)
  })

  test('prefers a static Spark Lightning Address over a legacy BOLT 12 offer (#5181)', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({ lightningAddress: 'oab38ad12345abcd9@spark.money' }),
    )

    expect(projection.directPayment).toMatchObject({
      lightningAddress: 'oab38ad12345abcd9@spark.money',
      kind: 'lightning_address',
      settlementAuthority: 'recipient_wallet_direct',
    })
    expect(forumTipRecipientReadinessIsSafe(projection)).toBe(true)
    expect(projection.tippingAvailable).toBe(true)
  })

  test('a ready recipient with only a Spark Lightning Address projects directPayment', () => {
    const projection = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: null,
        lightningAddress: 'oab38ad12345abcd9@spark.money',
      }),
    )
    expect(projection.directPayment).toMatchObject({
      lightningAddress: 'oab38ad12345abcd9@spark.money',
      kind: 'lightning_address',
      settlementAuthority: 'recipient_wallet_direct',
    })
    expect(projection.tippingAvailable).toBe(true)
    expect(JSON.stringify(projection)).toContain('@spark.money')
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

  test('names the right reachability constraint for each direct-payment rail', () => {
    const withOffer = projectForumTipRecipientReadiness(readyRecord())

    expect(withOffer.directPayment?.kind).toBe('bolt12_offer')
    expect(withOffer.caveatRefs).toContain(
      'caveat.public.forum_tip_recipient.daemon_reachability_required',
    )

    const withLightningAddress = projectForumTipRecipientReadiness(
      readyRecord({
        bolt12Offer: null,
        lightningAddress: 'oab38ad12345abcd9@spark.money',
      }),
    )

    expect(withLightningAddress.directPayment?.kind).toBe('lightning_address')
    expect(withLightningAddress.caveatRefs).toContain(
      'caveat.public.forum_tip_recipient.spark_lightning_address_claim_required',
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
