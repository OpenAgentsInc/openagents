import { describe, expect, test } from 'vitest'

import {
  buildOrangeCheckNostrAttestation,
  buildOrangeCheckNostrExport,
} from './orange-check-nostr-export'

const issuerPubkey = '11'.repeat(32)
const recipientPubkey = '22'.repeat(32)

const sampleEntitlement = {
  actionRef: 'forum_paid_action.orange_check.challenge_one',
  actorRef: 'agent:orange-owner',
  agentUserId: 'orange-owner',
  createdAt: '2026-06-10T10:00:00.000Z',
  id: 'orange_check_orange-owner',
  paidAmountCents: 500,
  receiptRef: 'orange_check_receipt.challenge_one',
  state: 'active' as const,
  updatedAt: '2026-06-10T10:01:00.000Z',
}

describe('orange-check Nostr export', () => {
  test('builds public-safe NIP-58 badge definition and award templates', async () => {
    const exported = await buildOrangeCheckNostrExport({
      entitlement: {
        actionRef: 'forum_paid_action.orange_check.challenge_one',
        actorRef: 'agent:orange-owner',
        agentUserId: 'orange-owner',
        createdAt: '2026-06-10T10:00:00.000Z',
        id: 'orange_check_orange-owner',
        paidAmountCents: 500,
        receiptRef: 'orange_check_receipt.challenge_one',
        state: 'active',
        updatedAt: '2026-06-10T10:01:00.000Z',
      },
      issuerPubkey,
      nowIso: '2026-06-10T10:02:00.000Z',
      recipientPubkey,
      relayUrls: ['wss://relay.openagents.example'],
    })

    expect(exported).toMatchObject({
      badgeDefinitionAddress: `30009:${issuerPubkey}:openagents-orange-check`,
      exportKind: 'nostr_nip58_badge_templates',
      receiptRef: 'orange_check_receipt.challenge_one',
      recipientPubkey,
      state: 'ready_to_sign_and_publish',
    })
    expect(exported.badgeDefinition.kind).toBe(30009)
    expect(exported.badgeAward.kind).toBe(8)
    expect(exported.badgeDefinition.created_at).toBe(1781085720)
    expect(exported.badgeAward.tags).toContainEqual([
      'p',
      recipientPubkey,
      'wss://relay.openagents.example',
    ])
    expect(exported.badgeAward.tags).toContainEqual([
      'receipt',
      'orange_check_receipt.challenge_one',
    ])
    expect(exported.authorityBoundary).toContain(
      'not identity verification',
    )
    expect(JSON.stringify(exported)).not.toMatch(/lnbc|preimage|mnemonic|wallet/i)
  })

  test('rejects malformed pubkeys before building templates', async () => {
    await expect(
      buildOrangeCheckNostrExport({
        entitlement: {
          actionRef: 'forum_paid_action.orange_check.challenge_one',
          actorRef: 'agent:orange-owner',
          agentUserId: 'orange-owner',
          createdAt: '2026-06-10T10:00:00.000Z',
          id: 'orange_check_orange-owner',
          paidAmountCents: 500,
          receiptRef: 'orange_check_receipt.challenge_one',
          state: 'active',
          updatedAt: '2026-06-10T10:01:00.000Z',
        },
        issuerPubkey,
        recipientPubkey: 'not-a-pubkey',
      }),
    ).rejects.toThrow('recipientPubkey must be 64 hex chars')
  })

  test('refuses an unparseable entitlement timestamp instead of inventing one', async () => {
    await expect(
      buildOrangeCheckNostrExport({
        entitlement: {
          actionRef: 'forum_paid_action.orange_check.challenge_one',
          actorRef: 'agent:orange-owner',
          agentUserId: 'orange-owner',
          createdAt: '2026-06-10T10:00:00.000Z',
          id: 'orange_check_orange-owner',
          paidAmountCents: 500,
          receiptRef: 'orange_check_receipt.challenge_one',
          state: 'active',
          updatedAt: 'not-a-date',
        },
        issuerPubkey,
        recipientPubkey,
      }),
    ).rejects.toThrow('entitlement timestamp is not a parseable ISO date')
  })
})

describe('orange-check Nostr attestation (NIP-01, owned-relay publishable)', () => {
  test('builds a public-safe kind-1 attestation note the owned relay accepts', async () => {
    const attestation = await buildOrangeCheckNostrAttestation({
      entitlement: sampleEntitlement,
      issuerPubkey,
      nowIso: '2026-06-10T10:02:00.000Z',
      recipientPubkey,
      relayUrls: ['wss://relay.openagents.example'],
    })

    expect(attestation).toMatchObject({
      badgeDefinitionAddress: `30009:${issuerPubkey}:openagents-orange-check`,
      exportKind: 'nostr_nip01_orange_check_attestation',
      receiptRef: 'orange_check_receipt.challenge_one',
      recipientPubkey,
      state: 'ready_to_sign_and_publish',
    })
    // The owned relay write allowlist covers NIP-01 kind 1 (general
    // coordination), not the NIP-58 badge kinds 8/30009.
    expect(attestation.note.kind).toBe(1)
    expect(attestation.note.created_at).toBe(1781085720)
    expect(attestation.note.tags).toContainEqual([
      'claim',
      'identity.orange_check_forum_signal.v1',
    ])
    expect(attestation.note.tags).toContainEqual([
      'a',
      `30009:${issuerPubkey}:openagents-orange-check`,
    ])
    expect(attestation.note.tags).toContainEqual([
      'receipt',
      'orange_check_receipt.challenge_one',
    ])
    expect(attestation.authorityBoundary).toContain('not identity verification')
    expect(JSON.stringify(attestation)).not.toMatch(
      /lnbc|preimage|mnemonic|wallet/i,
    )
  })

  test('rejects malformed pubkeys before building the attestation', async () => {
    await expect(
      buildOrangeCheckNostrAttestation({
        entitlement: sampleEntitlement,
        issuerPubkey,
        recipientPubkey: 'not-a-pubkey',
      }),
    ).rejects.toThrow('recipientPubkey must be 64 hex chars')
  })
})

