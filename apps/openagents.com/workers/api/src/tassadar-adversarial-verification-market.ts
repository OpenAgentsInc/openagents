import { Effect, Schema as S } from 'effect'

import {
  TassadarCompiledModuleConstructionRewardSats,
  autoSettleVerifiedCompiledModuleConstruction,
  type TassadarAutoSettlementDeps,
  type TassadarCompiledModuleConstructionSettlementOutcome,
} from './tassadar-auto-settlement'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export const TassadarAdversarialDivergenceRewardSats =
  TassadarCompiledModuleConstructionRewardSats

export const TassadarDivergenceKind = S.Literals([
  'trace_digest_mismatch',
  'output_mismatch',
  'near_miss_refusal_missing',
])
export type TassadarDivergenceKind = typeof TassadarDivergenceKind.Type

export const TassadarAdversarialDivergenceClaim = S.Struct({
  claimRef: S.String,
  claimantActorRef: S.String,
  claimantDeviceRef: S.String,
  divergenceKind: TassadarDivergenceKind,
  expectedBehaviorDigest: S.String,
  implementationRefs: S.Array(S.String),
  inputDigest: S.String,
  inputRef: S.String,
  moduleDigest: S.String,
  moduleKind: S.String,
  moduleRef: S.String,
  observedBehaviorDigest: S.String,
  psionicEvidenceRefs: S.Array(S.String),
  psionicNearMissRefusalRef: S.optional(S.String),
  sourceRefs: S.Array(S.String),
  specRef: S.String,
  workRequestId: S.String,
})
export type TassadarAdversarialDivergenceClaim =
  typeof TassadarAdversarialDivergenceClaim.Type

export const TassadarAdversarialDivergenceReproduction = S.Struct({
  blockerRefs: S.Array(S.String),
  expectedBehaviorDigest: S.String,
  inputDigest: S.String,
  observedBehaviorDigest: S.String,
  psionicEvidenceRefs: S.Array(S.String),
  reproduced: S.Boolean,
  reproductionRef: S.String,
  validatorActorRef: S.String,
  validatorDeviceRef: S.String,
  validatorReceiptRefs: S.Array(S.String),
})
export type TassadarAdversarialDivergenceReproduction =
  typeof TassadarAdversarialDivergenceReproduction.Type

export const TassadarAdversarialVerificationStatus = S.Literals([
  'confirmed_defect',
  'rejected_false_claim',
])
export type TassadarAdversarialVerificationStatus =
  typeof TassadarAdversarialVerificationStatus.Type

export const TassadarAdversarialVerificationVerdict = S.Struct({
  blockerRefs: S.Array(S.String),
  claimRef: S.String,
  confirmationRef: S.String,
  defectContributionRef: S.String,
  divergenceKind: TassadarDivergenceKind,
  inputDigest: S.String,
  moduleDigest: S.String,
  moduleKind: S.String,
  psionicEvidenceRefs: S.Array(S.String),
  reproducible: S.Boolean,
  settlementEligible: S.Boolean,
  status: TassadarAdversarialVerificationStatus,
  validatorDeviceRef: S.String,
  validatorReceiptRefs: S.Array(S.String),
  verificationClass: S.Literal('e3_adversarial_divergence'),
  verificationCommandRef: S.Literal(
    'command.public.tassadar.e3_adversarial_divergence',
  ),
  verificationReceiptRefs: S.Array(S.String),
})
export type TassadarAdversarialVerificationVerdict =
  typeof TassadarAdversarialVerificationVerdict.Type

export type TassadarAdversarialVerificationReleaseGate = Readonly<{
  blockerRefs: ReadonlyArray<string>
  gateRef: string
  releaseAllowed: boolean
  settlementEligible: boolean
  status: 'accepted' | 'rejected'
  validatorReceiptRefs: ReadonlyArray<string>
  verificationReceiptRefs: ReadonlyArray<string>
}>

export type TassadarAdversarialDefectSettlementOutcome = Readonly<{
  confirmationRef: string
  defectContributionRef: string
  kind: 'settlement_attempted' | 'not_confirmed'
  realBitcoinMoved: boolean
  settlement: TassadarCompiledModuleConstructionSettlementOutcome | null
  settlementReceiptRef: string | null
  skipped:
    | 'not_confirmed'
    | TassadarCompiledModuleConstructionSettlementOutcome['skipped']
}>

export class TassadarAdversarialVerificationUnsafe extends S.TaggedErrorClass<TassadarAdversarialVerificationUnsafe>()(
  'TassadarAdversarialVerificationUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const digestPattern = /^(sha256:)?[a-f0-9]{32,128}$/i
const moduleDigestPattern = /^[a-f0-9]{32,128}$/i
const unsafeAdversarialPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|prompt|record|value)|dataset\.(private|raw)|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?(hash|id|invoice|preimage|proof|raw|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private([._-]|$)|provider[_-]?(account|credential|grant|payload|secret|token)|raw([._-]|$)|repo[_-]?private|secret|seed[_-]?phrase|sk-[a-z0-9]|source[._-]?(archive|private|raw)|token|trace[._-]?(raw|full|private|payload)|wallet)/i

const decodeClaim = S.decodeUnknownSync(TassadarAdversarialDivergenceClaim)
const decodeReproduction = S.decodeUnknownSync(
  TassadarAdversarialDivergenceReproduction,
)
const decodeVerdict = S.decodeUnknownSync(TassadarAdversarialVerificationVerdict)

const uniqueRefs = (
  refs: ReadonlyArray<string | undefined>,
): ReadonlyArray<string> =>
  [...new Set(refs.filter((ref): ref is string => ref !== undefined))]
    .map(ref => ref.trim())
    .filter(ref => ref.length > 0)
    .sort()

const assertSafeRefs = (
  refs: ReadonlyArray<string | undefined>,
  field: string,
): ReadonlyArray<string> => {
  const normalized = uniqueRefs(refs)
  const unsafe = normalized.find(ref =>
    !safeRefPattern.test(ref) || unsafeAdversarialPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new TassadarAdversarialVerificationUnsafe({
      reason: `${field} must be public-safe refs.`,
    })
  }

  return normalized
}

const assertDigest = (digest: string, field: string): void => {
  if (!digestPattern.test(digest)) {
    throw new TassadarAdversarialVerificationUnsafe({
      reason: `${field} must be a sha256-style digest ref.`,
    })
  }
}

const assertModuleDigest = (digest: string): void => {
  if (!moduleDigestPattern.test(digest)) {
    throw new TassadarAdversarialVerificationUnsafe({
      reason: 'Tassadar adversarial moduleDigest must be raw hex.',
    })
  }
}

const stableRefSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]+/g, '_').slice(0, 120)

const claimRefs = (
  claim: TassadarAdversarialDivergenceClaim,
): ReadonlyArray<string> =>
  assertSafeRefs(
    [
      claim.claimRef,
      claim.claimantActorRef,
      claim.claimantDeviceRef,
      claim.inputRef,
      claim.moduleKind,
      claim.moduleRef,
      claim.psionicNearMissRefusalRef,
      claim.specRef,
      claim.workRequestId,
      ...claim.implementationRefs,
      ...claim.psionicEvidenceRefs,
      ...claim.sourceRefs,
    ],
    'Tassadar adversarial divergence claim',
  )

const reproductionRefs = (
  reproduction: TassadarAdversarialDivergenceReproduction,
): ReadonlyArray<string> =>
  assertSafeRefs(
    [
      reproduction.reproductionRef,
      reproduction.validatorActorRef,
      reproduction.validatorDeviceRef,
      ...reproduction.blockerRefs,
      ...reproduction.psionicEvidenceRefs,
      ...reproduction.validatorReceiptRefs,
    ],
    'Tassadar adversarial divergence reproduction',
  )

export const verifyTassadarAdversarialDivergenceClaim = (
  input: Readonly<{
    claim: TassadarAdversarialDivergenceClaim
    reproduction: TassadarAdversarialDivergenceReproduction
  }>,
): TassadarAdversarialVerificationVerdict => {
  const claim = decodeClaim(input.claim)
  const reproduction = decodeReproduction(input.reproduction)
  claimRefs(claim)
  reproductionRefs(reproduction)
  assertModuleDigest(claim.moduleDigest)
  const digestChecks: ReadonlyArray<readonly [string, string]> = [
    ['claim inputDigest', claim.inputDigest],
    ['claim expectedBehaviorDigest', claim.expectedBehaviorDigest],
    ['claim observedBehaviorDigest', claim.observedBehaviorDigest],
    ['reproduction inputDigest', reproduction.inputDigest],
    ['reproduction expectedBehaviorDigest', reproduction.expectedBehaviorDigest],
    ['reproduction observedBehaviorDigest', reproduction.observedBehaviorDigest],
  ]
  digestChecks.forEach(([field, digest]) => assertDigest(digest, field))

  const behaviorDiverges =
    claim.expectedBehaviorDigest !== claim.observedBehaviorDigest
  const sameDevice =
    claim.claimantDeviceRef.trim() === reproduction.validatorDeviceRef.trim()
  const needsNearMissRef =
    claim.divergenceKind === 'near_miss_refusal_missing' &&
    claim.psionicNearMissRefusalRef === undefined
  const blockers = uniqueRefs([
    ...reproduction.blockerRefs,
    ...(sameDevice
      ? ['blocker.public.tassadar_adversarial.same_device_validator']
      : []),
    ...(claim.implementationRefs.length < 2
      ? ['blocker.public.tassadar_adversarial.implementation_pair_missing']
      : []),
    ...(!behaviorDiverges
      ? ['blocker.public.tassadar_adversarial.no_behavior_divergence']
      : []),
    ...(!reproduction.reproduced
      ? ['blocker.public.tassadar_adversarial.validator_did_not_reproduce']
      : []),
    ...(claim.inputDigest !== reproduction.inputDigest
      ? ['blocker.public.tassadar_adversarial.input_digest_mismatch']
      : []),
    ...(claim.expectedBehaviorDigest !== reproduction.expectedBehaviorDigest
      ? ['blocker.public.tassadar_adversarial.expected_digest_mismatch']
      : []),
    ...(claim.observedBehaviorDigest !== reproduction.observedBehaviorDigest
      ? ['blocker.public.tassadar_adversarial.observed_digest_mismatch']
      : []),
    ...(needsNearMissRef
      ? ['blocker.public.tassadar_adversarial.near_miss_refusal_ref_missing']
      : []),
  ])
  const accepted = blockers.length === 0
  const suffix = stableRefSuffix(`${claim.claimRef}.${claim.inputDigest}`)
  const psionicEvidenceRefs = uniqueRefs([
    claim.psionicNearMissRefusalRef,
    ...claim.psionicEvidenceRefs,
    ...reproduction.psionicEvidenceRefs,
  ])

  return {
    blockerRefs: blockers,
    claimRef: claim.claimRef,
    confirmationRef: accepted
      ? `receipt.public.tassadar_adversarial_divergence.confirmed.${suffix}`
      : `receipt.public.tassadar_adversarial_divergence.rejected.${suffix}`,
    defectContributionRef:
      `defect.public.tassadar_adversarial_divergence.${suffix}`,
    divergenceKind: claim.divergenceKind,
    inputDigest: claim.inputDigest,
    moduleDigest: claim.moduleDigest,
    moduleKind: claim.moduleKind,
    psionicEvidenceRefs,
    reproducible: accepted,
    settlementEligible: accepted,
    status: accepted ? 'confirmed_defect' : 'rejected_false_claim',
    validatorDeviceRef: reproduction.validatorDeviceRef,
    validatorReceiptRefs: uniqueRefs(reproduction.validatorReceiptRefs),
    verificationClass: 'e3_adversarial_divergence',
    verificationCommandRef: 'command.public.tassadar.e3_adversarial_divergence',
    verificationReceiptRefs: accepted
      ? uniqueRefs([
          claim.claimRef,
          reproduction.reproductionRef,
          ...reproduction.validatorReceiptRefs,
          ...psionicEvidenceRefs,
        ])
      : [],
  }
}

export const projectTassadarAdversarialVerificationReleaseGate = (
  verdictInput: TassadarAdversarialVerificationVerdict | undefined,
): TassadarAdversarialVerificationReleaseGate => {
  if (verdictInput === undefined) {
    return {
      blockerRefs: [
        'blocker.public.tassadar_adversarial.verification_missing',
      ],
      gateRef: 'gate.public.tassadar_adversarial.missing',
      releaseAllowed: false,
      settlementEligible: false,
      status: 'rejected',
      validatorReceiptRefs: [],
      verificationReceiptRefs: [],
    }
  }

  const verdict = decodeVerdict(verdictInput)
  assertSafeRefs(
    [
      verdict.claimRef,
      verdict.confirmationRef,
      verdict.defectContributionRef,
      verdict.moduleKind,
      verdict.validatorDeviceRef,
      ...verdict.blockerRefs,
      ...verdict.psionicEvidenceRefs,
      ...verdict.validatorReceiptRefs,
      ...verdict.verificationReceiptRefs,
    ],
    'Tassadar adversarial verification verdict',
  )
  assertModuleDigest(verdict.moduleDigest)
  assertDigest(verdict.inputDigest, 'verdict inputDigest')
  const accepted =
    verdict.status === 'confirmed_defect' &&
    verdict.reproducible &&
    verdict.settlementEligible &&
    verdict.blockerRefs.length === 0

  return {
    blockerRefs: uniqueRefs([
      ...verdict.blockerRefs,
      ...(!accepted && verdict.blockerRefs.length === 0
        ? ['blocker.public.tassadar_adversarial.defect_not_confirmed']
        : []),
    ]),
    gateRef:
      `gate.public.tassadar_adversarial.${stableRefSuffix(verdict.claimRef)}`,
    releaseAllowed: accepted,
    settlementEligible: accepted,
    status: accepted ? 'accepted' : 'rejected',
    validatorReceiptRefs: verdict.validatorReceiptRefs,
    verificationReceiptRefs: accepted ? verdict.verificationReceiptRefs : [],
  }
}

export const settleConfirmedTassadarDivergenceDefect = <Bindings>(
  deps: TassadarAutoSettlementDeps<Bindings>,
  input: Readonly<{
    amountSats?: number | undefined
    challenge: TrainingVerificationChallengeRecord
    lease: TrainingWindowLeaseRecord
    verdict: TassadarAdversarialVerificationVerdict
  }>,
): Effect.Effect<TassadarAdversarialDefectSettlementOutcome> =>
  Effect.gen(function* () {
    const verdict = decodeVerdict(input.verdict)
    const gate = projectTassadarAdversarialVerificationReleaseGate(verdict)

    if (!gate.settlementEligible) {
      return {
        confirmationRef: verdict.confirmationRef,
        defectContributionRef: verdict.defectContributionRef,
        kind: 'not_confirmed',
        realBitcoinMoved: false,
        settlement: null,
        settlementReceiptRef: null,
        skipped: 'not_confirmed',
      }
    }

    const settlement = yield* autoSettleVerifiedCompiledModuleConstruction(
      deps,
      {
        amountSats: input.amountSats ?? TassadarAdversarialDivergenceRewardSats,
        challenge: input.challenge,
        constructionContributionRef: verdict.defectContributionRef,
        lease: input.lease,
        moduleDigest: verdict.moduleDigest,
        moduleKind: verdict.moduleKind,
      },
    )

    return {
      confirmationRef: verdict.confirmationRef,
      defectContributionRef: verdict.defectContributionRef,
      kind: 'settlement_attempted',
      realBitcoinMoved: settlement.realBitcoinMoved,
      settlement,
      settlementReceiptRef: settlement.settlementReceiptRef,
      skipped: settlement.skipped,
    }
  })

export type TassadarAdversarialSettlementDeps<Bindings> =
  TassadarAutoSettlementDeps<Bindings> &
    Readonly<{
      run: TrainingRunRecord
    }>
