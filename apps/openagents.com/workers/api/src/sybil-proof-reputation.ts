import { Schema as S } from 'effect'

export const SybilProofReputationOutcome = S.Literals([
  'accepted',
  'rejected',
])
export type SybilProofReputationOutcome =
  typeof SybilProofReputationOutcome.Type

export const SybilProofReputationAuthorityBoundary = S.Literals([
  'read_only_reputation_projection',
])
export type SybilProofReputationAuthorityBoundary =
  typeof SybilProofReputationAuthorityBoundary.Type

export class SybilProofReputationAuthority extends S.Class<SybilProofReputationAuthority>(
  'SybilProofReputationAuthority',
)({
  authorityBoundary: SybilProofReputationAuthorityBoundary,
  noDispatchAuthority: S.Boolean,
  noManualOverrideWriteAuthority: S.Boolean,
  noPayoutAuthority: S.Boolean,
  noPublicClaimUpgrade: S.Boolean,
  noSettlementAuthority: S.Boolean,
}) {}

export class SybilProofReputationPaymentGraphEdge extends S.Class<SybilProofReputationPaymentGraphEdge>(
  'SybilProofReputationPaymentGraphEdge',
)({
  amountMsat: S.Number,
  createdAtIso: S.String,
  edgeRef: S.String,
  outcome: SybilProofReputationOutcome,
  payerRef: S.String,
  settlementRefs: S.Array(S.String),
  subjectRef: S.String,
  verificationRefs: S.Array(S.String),
  workRef: S.String,
}) {}

export class SybilProofReputationManualOverride extends S.Class<SybilProofReputationManualOverride>(
  'SybilProofReputationManualOverride',
)({
  createdAtIso: S.String,
  expiresAtIso: S.optional(S.String),
  issuedByRef: S.String,
  overrideRef: S.String,
  reasonRef: S.String,
  scoreDelta: S.optional(S.Number),
  scoreFloor: S.optional(S.Number),
  subjectRef: S.String,
}) {}

export class SybilProofReputationSubjectProjection extends S.Class<SybilProofReputationSubjectProjection>(
  'SybilProofReputationSubjectProjection',
)({
  acceptedSettledMsat: S.Number,
  acceptedVerifiedOutcomeCount: S.Number,
  authority: SybilProofReputationAuthority,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contributionRefs: S.Array(S.String),
  generatedAtIso: S.String,
  manualOverrideApplied: S.Boolean,
  manualOverrideRefs: S.Array(S.String),
  rejectedVerifiedOutcomeCount: S.Number,
  score: S.Number,
  scoreBasisRefs: S.Array(S.String),
  subjectRef: S.String,
}) {}

export class SybilProofReputationProjection extends S.Class<SybilProofReputationProjection>(
  'SybilProofReputationProjection',
)({
  authority: SybilProofReputationAuthority,
  caveatRefs: S.Array(S.String),
  generatedAtIso: S.String,
  projectionRef: S.String,
  scoreModelRef: S.String,
  subjects: S.Array(SybilProofReputationSubjectProjection),
}) {}

export class SybilProofReputationUnsafe extends S.TaggedErrorClass<SybilProofReputationUnsafe>()(
  'SybilProofReputationUnsafe',
  {
    reason: S.String,
  },
) {}

export const SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY:
  SybilProofReputationAuthority = {
    authorityBoundary: 'read_only_reputation_projection',
    noDispatchAuthority: true,
    noManualOverrideWriteAuthority: true,
    noPayoutAuthority: true,
    noPublicClaimUpgrade: true,
    noSettlementAuthority: true,
  }

const unsafeMaterialPattern =
  /(\/Users\/|\/home\/|access[_-]?token|bearer\s+|cookie|file:\/\/|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private[_-]?(key|repo)|provider[_-]?(credential|grant|payload|secret|token)|raw[_-]?(command|content|invoice|payment|payload|prompt|repo|runner|state)|secret|seed[_-]?phrase|\bsk-[A-Za-z0-9_-]{16,}\b|wallet[._-]?(key|material|mnemonic|preimage|secret|seed)|xprv)/i

const clampScore = (value: number): number =>
  Math.max(0, Math.min(1000, Math.round(value)))

const assertPublicSafe = (value: unknown, field: string): void => {
  if (unsafeMaterialPattern.test(JSON.stringify(value))) {
    throw new SybilProofReputationUnsafe({
      reason: `${field} contains private, payment, credential, wallet, or raw material`,
    })
  }
}

const assertAuthority = (authority: SybilProofReputationAuthority): void => {
  if (
    authority.authorityBoundary !== 'read_only_reputation_projection' ||
    !authority.noDispatchAuthority ||
    !authority.noManualOverrideWriteAuthority ||
    !authority.noPayoutAuthority ||
    !authority.noPublicClaimUpgrade ||
    !authority.noSettlementAuthority
  ) {
    throw new SybilProofReputationUnsafe({
      reason: 'sybil-proof reputation projection must remain read-only',
    })
  }
}

const isActiveOverride = (
  override: SybilProofReputationManualOverride,
  generatedAtIso: string,
): boolean =>
  override.expiresAtIso === undefined ||
  Date.parse(override.expiresAtIso) > Date.parse(generatedAtIso)

const settledVerified = (
  edge: SybilProofReputationPaymentGraphEdge,
): boolean =>
  edge.amountMsat > 0 &&
  edge.verificationRefs.length > 0 &&
  edge.settlementRefs.length > 0

const acceptedWeight = (amountMsat: number): number =>
  Math.log10(1 + amountMsat / 1000)

export const projectSybilProofReputation = (input: {
  edges: ReadonlyArray<SybilProofReputationPaymentGraphEdge>
  generatedAtIso: string
  manualOverrides?: ReadonlyArray<SybilProofReputationManualOverride>
  projectionRef: string
  scoreModelRef?: string
}): SybilProofReputationProjection => {
  assertAuthority(SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY)
  assertPublicSafe(input, 'sybil-proof reputation input')

  const scoreModelRef =
    input.scoreModelRef ?? 'model.public.reputation.tracerank_eigentrust.beta.v1'
  const subjects = new Map<
    string,
    {
      acceptedSettledMsat: number
      acceptedVerifiedOutcomeCount: number
      blockerRefs: string[]
      contributionRefs: string[]
      rejectedVerifiedOutcomeCount: number
      weight: number
    }
  >()

  const ensureSubject = (subjectRef: string) => {
    const existing = subjects.get(subjectRef)
    if (existing !== undefined) return existing
    const created = {
      acceptedSettledMsat: 0,
      acceptedVerifiedOutcomeCount: 0,
      blockerRefs: [] as string[],
      contributionRefs: [] as string[],
      rejectedVerifiedOutcomeCount: 0,
      weight: 0,
    }
    subjects.set(subjectRef, created)
    return created
  }

  for (const edge of input.edges) {
    const subject = ensureSubject(edge.subjectRef)
    if (!settledVerified(edge)) {
      subject.blockerRefs.push(`blocker.public.reputation.unsettled_or_unverified:${edge.edgeRef}`)
      continue
    }

    subject.contributionRefs.push(edge.edgeRef, ...edge.verificationRefs, ...edge.settlementRefs)
    if (edge.outcome === 'accepted') {
      subject.acceptedSettledMsat += edge.amountMsat
      subject.acceptedVerifiedOutcomeCount += 1
      subject.weight += acceptedWeight(edge.amountMsat)
      continue
    }

    subject.rejectedVerifiedOutcomeCount += 1
    subject.weight -= acceptedWeight(edge.amountMsat) / 2
  }

  const activeOverridesBySubject = new Map<
    string,
    SybilProofReputationManualOverride[]
  >()
  for (const override of input.manualOverrides ?? []) {
    if (!isActiveOverride(override, input.generatedAtIso)) continue
    if (
      override.reasonRef.trim() === '' ||
      override.issuedByRef.trim() === '' ||
      override.overrideRef.trim() === ''
    ) {
      throw new SybilProofReputationUnsafe({
        reason: 'manual reputation overrides require override, issuer, and reason refs',
      })
    }
    ensureSubject(override.subjectRef)
    activeOverridesBySubject.set(override.subjectRef, [
      ...(activeOverridesBySubject.get(override.subjectRef) ?? []),
      override,
    ])
  }

  const maxWeight = Math.max(
    1,
    ...Array.from(subjects.values(), subject => Math.max(0, subject.weight)),
  )

  const projectedSubjects = Array.from(subjects.entries())
    .map(([subjectRef, subject]) => {
      const overrides = activeOverridesBySubject.get(subjectRef) ?? []
      let score = clampScore((Math.max(0, subject.weight) / maxWeight) * 900)
      for (const override of overrides) {
        if (override.scoreDelta !== undefined) {
          score = clampScore(score + override.scoreDelta)
        }
        if (override.scoreFloor !== undefined) {
          score = Math.max(score, clampScore(override.scoreFloor))
        }
      }

      return S.decodeUnknownSync(SybilProofReputationSubjectProjection)({
        acceptedSettledMsat: subject.acceptedSettledMsat,
        acceptedVerifiedOutcomeCount: subject.acceptedVerifiedOutcomeCount,
        authority: SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY,
        blockerRefs: [...new Set(subject.blockerRefs)],
        caveatRefs: overrides.length > 0
          ? ['caveat.public.reputation.closed_beta_manual_override']
          : [],
        contributionRefs: [...new Set(subject.contributionRefs)],
        generatedAtIso: input.generatedAtIso,
        manualOverrideApplied: overrides.length > 0,
        manualOverrideRefs: overrides.map(override => override.overrideRef),
        rejectedVerifiedOutcomeCount: subject.rejectedVerifiedOutcomeCount,
        score,
        scoreBasisRefs: [
          scoreModelRef,
          'basis.public.reputation.verified_outcome_payment_graph',
          ...(overrides.length > 0
            ? ['basis.public.reputation.closed_beta_manual_override']
            : []),
        ],
        subjectRef,
      })
    })
    .sort((left, right) =>
      right.score === left.score
        ? left.subjectRef.localeCompare(right.subjectRef)
        : right.score - left.score,
    )

  return S.decodeUnknownSync(SybilProofReputationProjection)({
    authority: SYBIL_PROOF_REPUTATION_READ_ONLY_AUTHORITY,
    caveatRefs: [
      'caveat.public.reputation.beta_read_only_no_authority',
      ...(input.manualOverrides?.some(override =>
        isActiveOverride(override, input.generatedAtIso),
      )
        ? ['caveat.public.reputation.closed_beta_manual_overrides_enabled']
        : []),
    ],
    generatedAtIso: input.generatedAtIso,
    projectionRef: input.projectionRef,
    scoreModelRef,
    subjects: projectedSubjects,
  })
}
