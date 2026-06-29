import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  type ForumTipPayerWalletReadiness,
  ForumTipPayerWalletReadiness as ForumTipPayerWalletReadinessSchema,
  type ForumTipPayerWalletReadinessState,
} from './schemas'

export type ForumTipPayerWalletReadinessInput = Readonly<{
  actorRef: string
  caveatRefs: ReadonlyArray<string>
  configuredRefs: ReadonlyArray<string>
  fundedRefs: ReadonlyArray<string>
  sendReadyRefs: ReadonlyArray<string>
  sourceRef: string
}>

export class ForumTipPayerWalletReadinessUnsafe extends S.TaggedErrorClass<ForumTipPayerWalletReadinessUnsafe>()(
  'ForumTipPayerWalletReadinessUnsafe',
  {
    reason: S.String,
  },
) {}

const decodePayerWalletReadiness = S.decodeUnknownSync(
  ForumTipPayerWalletReadinessSchema,
)

const publicSafeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const unsafeWalletMaterialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|balance[._-]?sats|bearer|bolt11|bolt12|channel[_-]?monitor|cookie|entropy|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|preimage|proof=|secret)|preimage|private[_-]?(channel|key|wallet)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|balance|channel|invoice|liquidity|payment|payload|webhook)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[._-]?(config|key|material|mnemonic|payment|preimage|secret|seed|state))/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const publicRefIsSafe = (value: string): boolean =>
  publicSafeRefPattern.test(value) &&
  !containsProviderSecretMaterial(value) &&
  !unsafeWalletMaterialPattern.test(value) &&
  !rawTimestampPattern.test(value)

const assertSafeRef = (label: string, value: string): void => {
  if (!publicRefIsSafe(value)) {
    throw new ForumTipPayerWalletReadinessUnsafe({
      reason: `${label} must be a public-safe redacted ref without raw wallet, payment, provider, path, secret, balance, invoice, preimage, or timestamp material.`,
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

const readinessState = (input: {
  configuredRefs: ReadonlyArray<string>
  fundedRefs: ReadonlyArray<string>
  sendReadyRefs: ReadonlyArray<string>
}): ForumTipPayerWalletReadinessState => {
  if (input.sendReadyRefs.length > 0) {
    return 'send_ready'
  }

  if (input.fundedRefs.length > 0) {
    return 'funded'
  }

  return input.configuredRefs.length > 0 ? 'configured' : 'missing'
}

const blockerRefsForState = (
  state: ForumTipPayerWalletReadinessState,
): ReadonlyArray<string> => {
  if (state === 'missing') {
    return ['blocker.public.forum_tip_payer.wallet_missing']
  }

  if (state === 'configured') {
    return ['blocker.public.forum_tip_payer.wallet_unfunded']
  }

  if (state === 'funded') {
    return ['blocker.public.forum_tip_payer.send_readiness_missing']
  }

  return []
}

export const projectForumTipPayerWalletReadiness = (
  input: ForumTipPayerWalletReadinessInput,
): ForumTipPayerWalletReadiness => {
  assertSafeRef('Forum tip payer actor ref', input.actorRef)
  assertSafeRef('Forum tip payer source ref', input.sourceRef)
  const caveatRefs = assertSafeRefs(
    'Forum tip payer caveat ref',
    input.caveatRefs,
  )
  const configuredRefs = assertSafeRefs(
    'Forum tip payer configured ref',
    input.configuredRefs,
  )
  const fundedRefs = assertSafeRefs(
    'Forum tip payer funded ref',
    input.fundedRefs,
  )
  const sendReadyRefs = assertSafeRefs(
    'Forum tip payer send-ready ref',
    input.sendReadyRefs,
  )
  const state = readinessState({ configuredRefs, fundedRefs, sendReadyRefs })

  return decodePayerWalletReadiness({
    actorRef: input.actorRef,
    blockerRefs: blockerRefsForState(state),
    caveatRefs,
    configuredRefs,
    fundedRefs: state === 'missing' ? [] : fundedRefs,
    sendReadyRefs: state === 'send_ready' ? sendReadyRefs : [],
    sourceRef: input.sourceRef,
    state,
    tippingSpendAllowed: state === 'send_ready',
  })
}

export const forumTipPayerWalletReadinessHasPrivateMaterial = (
  readiness: ForumTipPayerWalletReadiness,
): boolean => {
  const serialized = JSON.stringify(readiness)

  return (
    containsProviderSecretMaterial(serialized) ||
    unsafeWalletMaterialPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
  )
}
