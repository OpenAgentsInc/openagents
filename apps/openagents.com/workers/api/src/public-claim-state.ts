import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const PublicClaimState = S.Literals([
  'blocked',
  'planned',
  'modeled',
  'measured',
  'prohibited',
  'verified',
  'settled',
])
export type PublicClaimState = typeof PublicClaimState.Type

export const PublicClaimKind = S.Literals([
  'site_url',
  'research',
  'saved_version',
  'deployment',
  'fulfillment_receipt',
  'provider_settlement',
  'public_beta_billing',
  'agent_challenge',
])
export type PublicClaimKind = typeof PublicClaimKind.Type

export const PublicClaimStateProjection = S.Struct({
  state: PublicClaimState,
  label: S.String,
  description: S.String,
  evidenceRefs: S.Array(S.String),
  caveats: S.Array(S.String),
})
export type PublicClaimStateProjection =
  typeof PublicClaimStateProjection.Type

export const PublicClaimCopyRule = S.Struct({
  state: PublicClaimState,
  copyRuleRef: S.String,
  allowedPublicVerb: S.String,
  evidenceRequired: S.Boolean,
  settlementEvidenceRequired: S.Boolean,
  disallowedClaimRefs: S.Array(S.String),
})
export type PublicClaimCopyRule = typeof PublicClaimCopyRule.Type

export class PublicClaimCopyUnsafe extends S.TaggedErrorClass<PublicClaimCopyUnsafe>()(
  'PublicClaimCopyUnsafe',
  {
    reason: S.String,
  },
) {}

const labelByState: Record<PublicClaimState, string> = {
  blocked: 'Blocked',
  measured: 'Measured',
  modeled: 'Modeled',
  planned: 'Planned',
  prohibited: 'Prohibited',
  settled: 'Settled',
  verified: 'Verified',
}

const descriptionByState: Record<PublicClaimState, string> = {
  blocked: 'Waiting on missing evidence, approval, or reachable authority.',
  measured: 'Observed by OpenAgents records, but not yet independently verified.',
  modeled: 'Estimated or inferred from a bounded model.',
  planned: 'Intended work or capability that is not yet evidenced.',
  prohibited: 'This claim must not be made on public surfaces.',
  settled: 'Backed by settlement or payment evidence.',
  verified: 'Backed by an OpenAgents receipt, deployment, or approved evidence record.',
}

const baseCaveatByState: Record<PublicClaimState, string> = {
  blocked: 'This claim is blocked until the missing evidence or approval exists.',
  measured: 'This is an operational measurement, not a final proof claim.',
  modeled: 'This is a model-backed claim and may change when measured.',
  planned: 'This claim is planned and should not be read as completed.',
  prohibited: 'This claim is prohibited for public use.',
  settled: 'Settlement evidence is present for this claim.',
  verified: 'Verification is limited to the linked evidence refs.',
}

const stateRank: Record<PublicClaimState, number> = {
  blocked: 0,
  planned: 0,
  modeled: 1,
  measured: 2,
  prohibited: 0,
  verified: 3,
  settled: 4,
}

const statesByRank: ReadonlyArray<PublicClaimState> = [
  'planned',
  'modeled',
  'measured',
  'verified',
  'settled',
]

const terminalStates = new Set<PublicClaimState>(['blocked', 'prohibited'])

const evidenceRequiredStates = new Set<PublicClaimState>([
  'measured',
  'verified',
  'settled',
])

const copyRuleByState: Record<PublicClaimState, PublicClaimCopyRule> = {
  blocked: {
    allowedPublicVerb: 'blocked',
    copyRuleRef: 'copy_rule.public_claim.blocked',
    disallowedClaimRefs: [
      'copy_claim.completed',
      'copy_claim.verified',
      'copy_claim.settled',
    ],
    evidenceRequired: false,
    settlementEvidenceRequired: false,
    state: 'blocked',
  },
  measured: {
    allowedPublicVerb: 'measured',
    copyRuleRef: 'copy_rule.public_claim.measured',
    disallowedClaimRefs: [
      'copy_claim.verified',
      'copy_claim.settled',
      'copy_claim.guaranteed',
    ],
    evidenceRequired: true,
    settlementEvidenceRequired: false,
    state: 'measured',
  },
  modeled: {
    allowedPublicVerb: 'modeled',
    copyRuleRef: 'copy_rule.public_claim.modeled',
    disallowedClaimRefs: [
      'copy_claim.completed',
      'copy_claim.verified',
      'copy_claim.settled',
    ],
    evidenceRequired: false,
    settlementEvidenceRequired: false,
    state: 'modeled',
  },
  planned: {
    allowedPublicVerb: 'planned',
    copyRuleRef: 'copy_rule.public_claim.planned',
    disallowedClaimRefs: [
      'copy_claim.completed',
      'copy_claim.verified',
      'copy_claim.settled',
    ],
    evidenceRequired: false,
    settlementEvidenceRequired: false,
    state: 'planned',
  },
  prohibited: {
    allowedPublicVerb: 'not_public',
    copyRuleRef: 'copy_rule.public_claim.prohibited',
    disallowedClaimRefs: [
      'copy_claim.any_public_assertion',
      'copy_claim.completed',
      'copy_claim.verified',
      'copy_claim.settled',
    ],
    evidenceRequired: false,
    settlementEvidenceRequired: false,
    state: 'prohibited',
  },
  settled: {
    allowedPublicVerb: 'settled',
    copyRuleRef: 'copy_rule.public_claim.settled',
    disallowedClaimRefs: ['copy_claim.guaranteed'],
    evidenceRequired: true,
    settlementEvidenceRequired: true,
    state: 'settled',
  },
  verified: {
    allowedPublicVerb: 'verified',
    copyRuleRef: 'copy_rule.public_claim.verified',
    disallowedClaimRefs: [
      'copy_claim.settled',
      'copy_claim.guaranteed',
    ],
    evidenceRequired: true,
    settlementEvidenceRequired: false,
    state: 'verified',
  },
}

const hasEvidence = (evidenceRefs: ReadonlyArray<string>): boolean =>
  evidenceRefs.length > 0

const hasSettlementEvidence = (
  evidenceRefs: ReadonlyArray<string>,
): boolean =>
  evidenceRefs.some(ref =>
    /(^settlement:|^payout:|^billing:|^receipt:settlement|settled)/i.test(ref),
  )

export const maxAllowedClaimState = (
  evidenceRefs: ReadonlyArray<string>,
): PublicClaimState =>
  hasSettlementEvidence(evidenceRefs)
    ? 'settled'
    : hasEvidence(evidenceRefs)
      ? 'verified'
      : 'planned'

export const clampPublicClaimState = (
  desired: PublicClaimState,
  evidenceRefs: ReadonlyArray<string>,
): PublicClaimState => {
  if (terminalStates.has(desired)) {
    return desired
  }

  const allowed = maxAllowedClaimState(evidenceRefs)

  return statesByRank[Math.min(stateRank[desired], stateRank[allowed])] ?? 'planned'
}

const forbiddenCopyPatterns = [
  /autonomous agent economy/i,
  /claim is done/i,
  /agents earn bitcoin/i,
  /provider settlement is live/i,
  /settled payout/i,
  /verified proof/i,
  /guaranteed/i,
  /opencode_auth_content/i,
  /auth\.json/i,
  /provider_account/i,
  /auth_grant/i,
  /raw[_-]?runner/i,
  /wallet[_-]?state/i,
]

const forbiddenRefPatterns = [
  /@/,
  /auth\.json/i,
  /bearer/i,
  /checkout_id=/i,
  /customer[_-]?(email|name|value)/i,
  /gh[op]_[A-Za-z0-9_]+/,
  /invoice/i,
  /lnbc|lntb|lnbcrt|lno1/i,
  /mnemonic/i,
  /opencode_auth_content/i,
  /payment[_-]?(hash|preimage)/i,
  /preimage/i,
  /private[_-]?key/i,
  /provider[_-]?(account|grant|payload|token)/i,
  /raw[_-]?(invoice|payment|payload|prompt|runner|run[_-]?log|webhook)/i,
  /secret/i,
  /source[_-]?archive/i,
  /token/i,
  /wallet/i,
]

const refShapePattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/

const assertPublicClaimRefSafe = (
  label: string,
  ref: string,
): void => {
  const trimmed = ref.trim()

  if (
    trimmed === '' ||
    !refShapePattern.test(trimmed) ||
    containsProviderSecretMaterial(trimmed) ||
    forbiddenRefPatterns.some(pattern => pattern.test(trimmed))
  ) {
    throw new PublicClaimCopyUnsafe({
      reason: `${label} contains private, secret, payment, wallet, provider, or customer material.`,
    })
  }
}

export const assertPublicClaimCopySafe = (
  copy: string,
): void => {
  if (containsProviderSecretMaterial(copy)) {
    throw new PublicClaimCopyUnsafe({
      reason: 'Public claim copy contains secret-shaped material.',
    })
  }

  const forbidden = forbiddenCopyPatterns.find(pattern => pattern.test(copy))

  if (forbidden !== undefined) {
    throw new PublicClaimCopyUnsafe({
      reason: 'Public claim copy overstates evidence or settlement state.',
    })
  }
}

export const publicClaimCopyRuleForState = (
  state: PublicClaimState,
): PublicClaimCopyRule => copyRuleByState[state]

export const publicClaimStateProjection = (
  input: Readonly<{
    desiredState: PublicClaimState
    evidenceRefs?: ReadonlyArray<string> | undefined
    kind: PublicClaimKind
    caveats?: ReadonlyArray<string> | undefined
  }>,
): PublicClaimStateProjection => {
  const evidenceRefs = [...new Set(input.evidenceRefs ?? [])]
    .filter(ref => ref.trim() !== '')
    .sort()

  for (const evidenceRef of evidenceRefs) {
    assertPublicClaimRefSafe('Public claim evidence ref', evidenceRef)
  }

  const state = clampPublicClaimState(input.desiredState, evidenceRefs)
  const caveats = [
    baseCaveatByState[state],
    ...(input.desiredState !== state
      ? [`Requested ${input.desiredState} claim was lowered to ${state} because required evidence is missing.`]
      : []),
    ...(input.caveats ?? []),
  ]

  for (const caveat of caveats) {
    assertPublicClaimCopySafe(caveat)
  }

  if (
    evidenceRequiredStates.has(state) &&
    evidenceRefs.length === 0
  ) {
    throw new PublicClaimCopyUnsafe({
      reason: `${state} public claims require evidence refs.`,
    })
  }

  return {
    caveats,
    description: descriptionByState[state],
    evidenceRefs,
    label: labelByState[state],
    state,
  }
}
