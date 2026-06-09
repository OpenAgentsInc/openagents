import { containsProviderSecretMaterial } from '@openagents/provider-account-schema'
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

const assertSafeRef = (label: string, value: string | null): void => {
  if (value !== null && !publicRefIsSafe(value)) {
    throw new ForumTipRecipientWalletUnsafe({
      reason: `${label} must be a public-safe redacted ref without raw wallet, payment, payout, provider, private path, secret, or timestamp material.`,
    })
  }
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
  const stateBlockerRef = {
    blocked: 'blocker.public.forum_tip_recipient.actor_blocked',
    disabled: 'blocker.public.forum_tip_recipient.wallet_disabled',
    ready: null,
  }[safe.state]

  return decodeReadiness({
    actorRef: safe.actorRef,
    blockerRef: blocked ? stateBlockerRef : null,
    caveatRefs: uniqueRefs([
      ...safe.caveatRefs,
      ...safe.claimPolicyRefs,
      ...safe.custodyPolicyRefs,
      ...(safe.payoutTargetApprovalRef === null
        ? ['caveat.public.forum_tip_recipient.payout_target_unapproved']
        : []),
    ]),
    providerClass: safe.providerClass,
    readinessRefs: safe.state === 'ready' ? safe.readinessRefs : [],
    sourceRef: safe.sourceRef,
    state: safe.state,
    tippingAvailable: safe.state === 'ready',
  })
}

export const forumTipRecipientReadinessIsSafe = (
  readiness: ForumTipRecipientReadiness,
): boolean =>
  !containsProviderSecretMaterial(JSON.stringify(readiness)) &&
  !unsafeWalletMaterialPattern.test(JSON.stringify(readiness)) &&
  !rawTimestampPattern.test(JSON.stringify(readiness))
