import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type ForumTipRecipientProviderClass,
  type ForumTipRecipientReadiness,
  ForumTipRecipientReadiness as ForumTipRecipientReadinessSchema,
  type ForumTipRecipientReadinessState,
} from './schemas'

export type ForumTipRecipientWalletState = Exclude<
  ForumTipRecipientReadinessState,
  'missing'
>

export type ForumTipRecipientWalletRecord = Readonly<{
  actorRef: string
  sparkAddress: string | null
  bolt12Offer: string | null
  lightningAddress: string | null
  caveatRefs: ReadonlyArray<string>
  claimPolicyRefs: ReadonlyArray<string>
  custodyPolicyRefs: ReadonlyArray<string>
  disabledAt: string | null
  id: string
  payoutTargetApprovalRef: string | null
  providerClass: ForumTipRecipientProviderClass
  readinessRefs: ReadonlyArray<string>
  receiveCapabilityRef: string
  sourceRef: string
  state: ForumTipRecipientWalletState
  walletRef: string
}>

export class ForumTipRecipientWalletUnsafe extends S.TaggedErrorClass<ForumTipRecipientWalletUnsafe>()(
  'ForumTipRecipientWalletUnsafe',
  {
    reason: S.String,
  },
) {}

const decodeReadiness = S.decodeUnknownSync(ForumTipRecipientReadinessSchema)

const publicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const bolt12OfferPattern = /^lno1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,4092}$/i
const unsafeWalletMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|balance[._-]?sats|bearer|bolt11|bolt12|channel[_-]?monitor|checkout[_-]?secret|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|entropy|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof=|secret)|payout[_-]?(address|destination|private|raw)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|backup|balance|channel|invoice|liquidity|payment|payload|payout|target|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const publicRefIsSafe = (value: string): boolean =>
  publicSafeRefPattern.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !unsafeWalletMaterialPattern.test(value) &&
  !rawTimestampPattern.test(value)

// A native Spark address is a bech32m string whose human-readable part is a
// Spark network HRP (`spark`, `sparkt`, `sparkrt`, `sparks`, or the legacy
// `sp`/`spt`/`sprt`/`sps`), e.g. `spark1pgss…`. It is a derived, static,
// registration-free Spark→Spark receive destination the recipient publishes —
// a public payment destination like a BOLT 12 offer, NOT a private payout
// target or wallet secret. See projects/repos/spark-sdk
// (crates/spark/src/address/mod.rs HRP constants).
const sparkAddressPattern =
  /^(?:spark|sparkt|sparkrt|sparks|sp|spt|sprt|sps)1[qpzry9x8gf2tvdw0s3jn54khce6mua7l]{16,512}$/i

const normalizeSparkAddress = (value: string): string =>
  value.trim().toLowerCase()

const sparkAddressIsPublicReceiveInstruction = (value: string): boolean => {
  const normalized = normalizeSparkAddress(value)

  return (
    normalized.length <= 600 &&
    sparkAddressPattern.test(normalized) &&
    !containsProviderSecretMaterial(normalized) &&
    // Reject anything that is actually a raw invoice / lnurl / wallet secret
    // smuggled into the Spark-address field. A plain `spark1…` address is fine.
    !/\s|@|lnbc|lntb|lnbcrt|lnurl|lno1|mnemonic|preimage|payment[_-]?secret|private[_-]?key|wallet[_-]?secret|seed[_-]?phrase|recovery[_-]?phrase|access[_-]?token|api[_-]?key|macaroon/i.test(
      normalized,
    )
  )
}

const assertSparkAddress = (
  label: string,
  value: string | null,
): string | null => {
  if (value === null) {
    return null
  }

  const normalized = normalizeSparkAddress(value)

  if (!sparkAddressIsPublicReceiveInstruction(normalized)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason: `${label} must be a public native Spark address (bech32m, e.g. spark1…), not a BOLT 11 invoice, LNURL, mnemonic, preimage, private key, wallet secret, or provider credential.`,
    })
  }

  return normalized
}

const normalizeBolt12Offer = (value: string): string => value.trim().toLowerCase()

const bolt12OfferIsPublicReceiveInstruction = (value: string): boolean => {
  const normalized = normalizeBolt12Offer(value)

  return (
    normalized.length <= 4096 &&
    bolt12OfferPattern.test(normalized) &&
    !containsProviderSecretMaterial(normalized) &&
    !/\s|@|lnbc|lntb|lnbcrt|lnurl|mnemonic|preimage|payment[_-]?secret|private[_-]?key|wallet[_-]?secret/i.test(
      normalized,
    )
  )
}

// A static Lightning Address is `localpart@domain` (LNURL-pay), e.g.
// `oab38ad12345abcd9@spark.money`. It is a public payment destination the
// recipient chooses to publish, like a BOLT 12 offer, so the `@` is allowed
// here even though the generic unsafe-material pattern flags `@` for refs.
const lightningAddressPattern =
  /^[a-z0-9][a-z0-9._%+-]{0,127}@[a-z0-9](?:[a-z0-9-]{0,62}\.)+[a-z]{2,24}$/i

const lightningAddressIsPublicReceiveInstruction = (value: string): boolean => {
  const normalized = value.trim()

  return (
    normalized.length <= 512 &&
    !/\s/.test(normalized) &&
    lightningAddressPattern.test(normalized) &&
    !containsProviderSecretMaterial(normalized) &&
    // Reject anything that is actually a raw invoice / lnurl / wallet secret
    // smuggled into the address field. A plain email-shaped address is fine.
    !/lnbc|lntb|lnbcrt|lnurl|lno1|mnemonic|preimage|payment[_-]?secret|private[_-]?key|wallet[_-]?secret|seed[_-]?phrase|recovery[_-]?phrase|access[_-]?token|api[_-]?key|macaroon/i.test(
      normalized,
    )
  )
}

const assertLightningAddress = (
  label: string,
  value: string | null | undefined,
): string | null => {
  if (value === null || value === undefined) {
    return null
  }

  const normalized = value.trim()

  if (!lightningAddressIsPublicReceiveInstruction(normalized)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason: `${label} must be a public static Lightning Address of the form name@domain, not a BOLT 11 invoice, LNURL, mnemonic, preimage, private key, wallet secret, or provider credential.`,
    })
  }

  return normalized
}

const assertSafeRef = (label: string, value: string | null): void => {
  if (value !== null && !publicRefIsSafe(value)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason: `${label} must be a public-safe redacted ref without raw wallet, payment, payout, provider, private path, secret, or timestamp material.`,
    })
  }
}

const assertBolt12Offer = (
  label: string,
  value: string | null,
): string | null => {
  if (value === null) {
    return null
  }

  const normalized = normalizeBolt12Offer(value)

  if (!bolt12OfferIsPublicReceiveInstruction(normalized)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason: `${label} must be a public BOLT 12 offer beginning with lno1, not a BOLT 11 invoice, LNURL, mnemonic, preimage, private key, wallet secret, or provider credential.`,
    })
  }

  return normalized
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const cleaned = uniqueRefs(refs)

  for (const ref of cleaned) {
    assertSafeRef(label, ref)
  }

  return cleaned
}

export const forumTipRecipientWalletRecordHasPrivateMaterial = (
  record: ForumTipRecipientWalletRecord,
): boolean => {
  const publicProbe = {
    actorRef: record.actorRef,
    caveatRefs: record.caveatRefs,
    claimPolicyRefs: record.claimPolicyRefs,
    custodyPolicyRefs: record.custodyPolicyRefs,
    payoutTargetApprovalRef: record.payoutTargetApprovalRef,
    providerClass: record.providerClass,
    readinessRefs: record.readinessRefs,
    receiveCapabilityRef: record.receiveCapabilityRef,
    sourceRef: record.sourceRef,
    state: record.state,
    walletRef: record.walletRef,
  }
  const json = JSON.stringify(publicProbe)

  return (
    containsProviderSecretMaterial(json) ||
    unsafeWalletMaterialPattern.test(json) ||
    rawTimestampPattern.test(json)
  )
}

export const assertForumTipRecipientWalletRecordSafe = (
  record: ForumTipRecipientWalletRecord,
): ForumTipRecipientWalletRecord => {
  assertSafeRef('Forum tip recipient actor ref', record.actorRef)
  assertSafeRef('Forum tip recipient wallet ref', record.walletRef)
  assertSafeRef(
    'Forum tip recipient receive capability ref',
    record.receiveCapabilityRef,
  )
  assertSafeRef(
    'Forum tip recipient payout target approval ref',
    record.payoutTargetApprovalRef,
  )
  assertSafeRef('Forum tip recipient source ref', record.sourceRef)
  const sparkAddress = assertSparkAddress(
    'Forum tip recipient Spark address',
    record.sparkAddress,
  )
  const bolt12Offer = assertBolt12Offer(
    'Forum tip recipient BOLT 12 offer',
    record.bolt12Offer,
  )
  const lightningAddress = assertLightningAddress(
    'Forum tip recipient Lightning Address',
    record.lightningAddress,
  )
  const readinessRefs = assertSafeRefs(
    'Forum tip recipient readiness ref',
    record.readinessRefs,
  )
  const caveatRefs = assertSafeRefs(
    'Forum tip recipient caveat ref',
    record.caveatRefs,
  )
  const custodyPolicyRefs = assertSafeRefs(
    'Forum tip recipient custody policy ref',
    record.custodyPolicyRefs,
  )
  const claimPolicyRefs = assertSafeRefs(
    'Forum tip recipient claim policy ref',
    record.claimPolicyRefs,
  )

  if (record.state === 'ready' && readinessRefs.length === 0) {
    throw new ForumTipRecipientWalletUnsafe({
      reason:
        'Ready Forum tip recipients require at least one public-safe readiness ref.',
    })
  }

  if (forumTipRecipientWalletRecordHasPrivateMaterial(record)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason:
        'Forum tip recipient wallet admission contains private wallet, payment, payout, provider, path, secret, or timestamp material.',
    })
  }

  return {
    ...record,
    sparkAddress,
    bolt12Offer,
    lightningAddress,
    caveatRefs,
    claimPolicyRefs,
    custodyPolicyRefs,
    readinessRefs,
  }
}

export const missingForumTipRecipientReadiness = (
  actorRef: string,
): ForumTipRecipientReadiness =>
  decodeReadiness({
    actorRef,
    blockerRef: 'blocker.public.forum_tip_recipient.wallet_missing',
    caveatRefs: ['caveat.public.forum_tip_recipient.wallet_not_admitted'],
    directPayment: null,
    providerClass: null,
    readinessRefs: [],
    sourceRef: 'forum_tip_recipient_wallets',
    state: 'missing',
    tippingAvailable: false,
  })

export const projectForumTipRecipientReadiness = (
  record: ForumTipRecipientWalletRecord,
): ForumTipRecipientReadiness => {
  const safe = assertForumTipRecipientWalletRecordSafe(record)
  const blocked = safe.state !== 'ready'
  // Native Spark address is the preferred rail: Spark→Spark, 0-fee,
  // registration-free, offline-receive. The Lightning rails are fallbacks for
  // external Lightning senders only.
  const directPayment =
    safe.state === 'ready' && safe.sparkAddress !== null
      ? {
          sparkAddress: safe.sparkAddress,
          kind: 'spark_address' as const,
          settlementAuthority: 'recipient_wallet_direct' as const,
        }
      : safe.state === 'ready' && safe.lightningAddress !== null
      ? {
          lightningAddress: safe.lightningAddress,
          kind: 'lightning_address' as const,
          settlementAuthority: 'recipient_wallet_direct' as const,
        }
      : safe.state === 'ready' && safe.bolt12Offer !== null
      ? {
          bolt12Offer: safe.bolt12Offer,
          kind: 'bolt12_offer' as const,
          settlementAuthority: 'recipient_wallet_direct' as const,
        }
      : null
  const missingDirectOffer = safe.state === 'ready' && directPayment === null
  const stateBlockerRef = {
    blocked: 'blocker.public.forum_tip_recipient.actor_blocked',
    disabled: 'blocker.public.forum_tip_recipient.wallet_disabled',
    ready: null,
  }[safe.state]

  return decodeReadiness({
    actorRef: safe.actorRef,
    blockerRef: blocked
      ? stateBlockerRef
      : missingDirectOffer
        ? 'blocker.public.forum_tip_recipient.payment_instruction_missing'
        : null,
    caveatRefs: uniqueRefs([
      ...safe.caveatRefs,
      ...safe.claimPolicyRefs,
      ...safe.custodyPolicyRefs,
      ...(safe.payoutTargetApprovalRef === null
        ? ['caveat.public.forum_tip_recipient.payout_target_unapproved']
        : []),
      ...(missingDirectOffer
        ? ['caveat.public.forum_tip_recipient.payment_instruction_missing']
        : []),
      ...(directPayment === null
        ? []
        : directPayment.kind === 'spark_address'
          ? ['caveat.public.forum_tip_recipient.spark_offline_receive']
          : directPayment.kind === 'lightning_address'
            ? ['caveat.public.forum_tip_recipient.spark_lightning_address_claim_required']
            : ['caveat.public.forum_tip_recipient.daemon_reachability_required']),
    ]),
    directPayment,
    providerClass: safe.providerClass,
    readinessRefs: safe.state === 'ready' ? safe.readinessRefs : [],
    sourceRef: safe.sourceRef,
    state: safe.state,
    tippingAvailable: directPayment !== null,
  })
}

export const forumTipRecipientReadinessIsSafe = (
  readiness: ForumTipRecipientReadiness,
): boolean => {
  const { directPayment, ...genericProbe } = readiness

  return (
    !containsProviderSecretMaterial(JSON.stringify(genericProbe)) &&
    !unsafeWalletMaterialPattern.test(JSON.stringify(genericProbe)) &&
    !rawTimestampPattern.test(JSON.stringify(genericProbe)) &&
    (directPayment === null ||
      (directPayment.settlementAuthority === 'recipient_wallet_direct' &&
        (directPayment.kind === 'spark_address'
          ? sparkAddressIsPublicReceiveInstruction(directPayment.sparkAddress)
          : directPayment.kind === 'bolt12_offer'
          ? bolt12OfferIsPublicReceiveInstruction(directPayment.bolt12Offer) &&
            (directPayment.lightningAddress === undefined ||
              lightningAddressIsPublicReceiveInstruction(
                directPayment.lightningAddress,
              ))
          : lightningAddressIsPublicReceiveInstruction(
              directPayment.lightningAddress,
            ))))
  )
}
