import { sha256Hex } from '@openagents/nip90'
import type { NostrEvent } from 'nostr-effect'
import {
  BADGE_DEFINITION_KIND,
  generateBadgeAwardEventTemplate,
  generateBadgeDefinitionEventTemplate,
  validateBadgeAwardEvent,
  validateBadgeDefinitionEvent,
} from 'nostr-effect/nip58'

import type { OrangeCheckEntitlement } from './orange-check-entitlements'

const DEFAULT_ORANGE_CHECK_RELAY = 'wss://openagents-market-relay.openagents.workers.dev'
const OPENAGENTS_ORANGE_CHECK_BADGE_D = 'openagents-orange-check'
const hexPubkeyPattern = /^[a-f0-9]{64}$/i
const unsafePublicMaterialPattern =
  /(access[_-]?token|bearer\s+|cookie|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?key|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(invoice|payment|payload|prompt|runner|state)|secret|seed[_-]?phrase|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

export type OrangeCheckNostrEventTemplate = Readonly<{
  content: string
  created_at: number
  kind: number
  tags: ReadonlyArray<ReadonlyArray<string>>
}>

export type OrangeCheckNostrExport = Readonly<{
  authorityBoundary: string
  badgeAward: OrangeCheckNostrEventTemplate
  badgeDefinition: OrangeCheckNostrEventTemplate
  badgeDefinitionAddress: string
  exportDigestRef: string
  exportKind: 'nostr_nip58_badge_templates'
  relayUrls: ReadonlyArray<string>
  recipientPubkey: string
  receiptRef: string
  state: 'ready_to_sign_and_publish'
}>

export class OrangeCheckNostrExportError extends Error {}

const unixTimestampFromIso = (iso: string): number => {
  const parsed = Date.parse(iso)

  if (Number.isNaN(parsed)) {
    throw new OrangeCheckNostrExportError(
      'entitlement timestamp is not a parseable ISO date',
    )
  }

  return Math.floor(parsed / 1000)
}

const appendTags = (
  template: OrangeCheckNostrEventTemplate,
  tags: ReadonlyArray<ReadonlyArray<string>>,
  createdAt: number,
): OrangeCheckNostrEventTemplate => ({
  content: template.content,
  created_at: createdAt,
  kind: template.kind,
  tags: [...template.tags, ...tags],
})

const normalizeRelayUrls = (
  relayUrls: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> => {
  const values =
    relayUrls === undefined || relayUrls.length === 0
      ? [DEFAULT_ORANGE_CHECK_RELAY]
      : relayUrls

  return [...new Set(values.map(value => value.trim()).filter(Boolean))]
}

const validationEvent = (
  template: OrangeCheckNostrEventTemplate,
  pubkey: string,
): NostrEvent =>
  ({
    ...template,
    id: '00'.repeat(32),
    pubkey,
    sig: '00'.repeat(64),
  }) as NostrEvent

export const buildOrangeCheckNostrExport = async (input: {
  entitlement: OrangeCheckEntitlement
  issuerPubkey: string
  nowIso?: string
  recipientPubkey: string
  relayUrls?: ReadonlyArray<string>
}): Promise<OrangeCheckNostrExport> => {
  const issuerPubkey = input.issuerPubkey.toLowerCase()
  const recipientPubkey = input.recipientPubkey.toLowerCase()

  if (!hexPubkeyPattern.test(issuerPubkey)) {
    throw new OrangeCheckNostrExportError('issuerPubkey must be 64 hex chars')
  }

  if (!hexPubkeyPattern.test(recipientPubkey)) {
    throw new OrangeCheckNostrExportError(
      'recipientPubkey must be 64 hex chars',
    )
  }

  const relayUrls = normalizeRelayUrls(input.relayUrls)
  const createdAt = unixTimestampFromIso(input.nowIso ?? input.entitlement.updatedAt)
  const badgeDefinitionAddress = `${BADGE_DEFINITION_KIND}:${issuerPubkey}:${OPENAGENTS_ORANGE_CHECK_BADGE_D}`
  const definition = appendTags(
    generateBadgeDefinitionEventTemplate({
      d: OPENAGENTS_ORANGE_CHECK_BADGE_D,
      description:
        'Owner-claimed OpenAgents account with a recent Bitcoin-backed OpenAgents participation receipt. Economic participation signal only; not identity verification, moderation immunity, or settlement authority.',
      name: 'OpenAgents Orange Check',
    }),
    [
      ['claim', 'identity.orange_check_forum_signal.v1'],
      ['receipt', input.entitlement.receiptRef],
      ['amount', String(input.entitlement.paidAmountCents), 'USD_CENTS'],
    ],
    createdAt,
  )
  const award = appendTags(
    generateBadgeAwardEventTemplate({
      a: badgeDefinitionAddress,
      p: [[recipientPubkey, relayUrls[0] ?? DEFAULT_ORANGE_CHECK_RELAY]],
    }),
    [
      ['receipt', input.entitlement.receiptRef],
      ['actor', input.entitlement.actorRef],
      ['claim', 'identity.orange_check_forum_signal.v1'],
      ['amount', String(input.entitlement.paidAmountCents), 'USD_CENTS'],
    ],
    createdAt,
  )
  const publicProjection = JSON.stringify({
    award,
    badgeDefinitionAddress,
    definition,
    recipientPubkey,
    receiptRef: input.entitlement.receiptRef,
  })

  if (unsafePublicMaterialPattern.test(publicProjection)) {
    throw new OrangeCheckNostrExportError(
      'orange-check Nostr export contains unsafe public material',
    )
  }

  if (
    !validateBadgeDefinitionEvent(validationEvent(definition, issuerPubkey)) ||
    !validateBadgeAwardEvent(validationEvent(award, issuerPubkey))
  ) {
    throw new OrangeCheckNostrExportError(
      'nostr-effect rejected the orange-check badge templates',
    )
  }

  return {
    authorityBoundary:
      'This export is an unsigned NIP-58 badge template for an orange-check economic participation signal. It is not identity verification, moderation immunity, payout authority, or settlement evidence by itself.',
    badgeAward: award,
    badgeDefinition: definition,
    badgeDefinitionAddress,
    exportDigestRef: `nostr_export.orange_check.${(await sha256Hex(publicProjection)).slice(0, 32)}`,
    exportKind: 'nostr_nip58_badge_templates',
    relayUrls,
    recipientPubkey,
    receiptRef: input.entitlement.receiptRef,
    state: 'ready_to_sign_and_publish',
  }
}
