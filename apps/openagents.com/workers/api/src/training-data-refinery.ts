import { Schema as S } from 'effect'

import {
  isRecord,
  optionalString,
  parseJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import {
  Cs336A4DataRefineryJobKind,
  Cs336A4HomeworkStages,
  Cs336A4PsionicLaneRef,
  type Cs336A4HomeworkStage,
} from './cs336-a4-data-refinery'
import type {
  TrainingRunRecord,
  TrainingWindowLeaseRecord,
  TrainingWindowRecord,
} from './training-run-window-authority'
import type { TrainingVerificationChallengeRecord } from './training-verification'

export type DataRefineryShard = Readonly<{
  inputDocumentCount: number | null
  outputDigestRef: string
  pylonRef: string | null
  receiptRefs: ReadonlyArray<string>
  settledPayoutSats: number
  shardRef: string
  sourceRefs: ReadonlyArray<string>
  stage: Cs336A4HomeworkStage
  trainingRunRef: string
  verificationRefs: ReadonlyArray<string>
  verified: boolean
}>

export type DataRefineryProjection = Readonly<{
  blockerRefs: ReadonlyArray<string>
  evalDeltaBonusBlockerRefs: ReadonlyArray<string>
  observedVerifiedShardCount: number
  observedVerifiedStages: ReadonlyArray<Cs336A4HomeworkStage>
  psionicLaneRef: string
  requiredVerifiedStageCount: number
  schemaVersion: 'openagents.training.data_refinery.v1'
  scopeBoundaryRefs: ReadonlyArray<string>
  shards: ReadonlyArray<DataRefineryShard>
  sourceRefs: ReadonlyArray<string>
  status:
    | 'blocked_no_shards'
    | 'collecting_shards'
    | 'stages_verified'
}>

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

export const Cs336A4RefineryStageEvidence = S.Struct({
  inputDocumentCount: S.optionalKey(S.Number),
  outputDigestRef: PublicSafeRef,
  pylonRef: S.optionalKey(PublicSafeRef),
  receiptRefs: S.Array(PublicSafeRef),
  shardRef: S.optionalKey(PublicSafeRef),
  sourceRefs: PublicSafeRefs,
  stage: S.Literals([...Cs336A4HomeworkStages]),
  verificationRefs: PublicSafeRefs,
})
export type Cs336A4RefineryStageEvidence =
  typeof Cs336A4RefineryStageEvidence.Type

export const Cs336A4DataRefineryEvidenceRequest = S.Struct({
  psionicLaneRef: S.optionalKey(PublicSafeRef),
  receiptRefs: PublicSafeRefs,
  shards: S.Array(Cs336A4RefineryStageEvidence),
  sourceRefs: PublicSafeRefs,
})
export type Cs336A4DataRefineryEvidenceRequest =
  typeof Cs336A4DataRefineryEvidenceRequest.Type

export class DataRefineryUnsafeProjectionError extends Error {
  readonly _tag = 'DataRefineryUnsafeProjectionError'
}

export class DataRefineryEvidenceValidationError extends Error {
  readonly _tag = 'DataRefineryEvidenceValidationError'
}

/**
 * Public-safety guard for admitted A4 refinery evidence. Pylon refs and
 * output-digest refs are legitimate public provenance, but the guard
 * still rejects wallet, payment, invoice, mnemonic, key, raw-shard, and
 * private path material before it can reach D1.
 */
const unsafeRefineryMaterialPattern =
  /(\"?(mnemonic|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner|shard|warc)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeRefineryMaterialPattern.test(json)) {
    throw new DataRefineryUnsafeProjectionError(
      'CS336 A4 data-refinery projection contains wallet, payment, raw-shard, or private material.',
    )
  }

  return json
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const optionalNumber = (value: unknown): number | undefined => {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value
  }

  if (typeof value !== 'string' || value.trim() === '') {
    return undefined
  }

  const parsed = Number(value)

  return Number.isFinite(parsed) ? parsed : undefined
}

const assertAdmissibleShard = (shard: Cs336A4RefineryStageEvidence): void => {
  if (shard.receiptRefs.length === 0) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 refinery shard evidence requires at least one receipt ref; unreceipted shards are not admissible.',
    )
  }

  if (
    shard.inputDocumentCount !== undefined &&
    (!Number.isFinite(shard.inputDocumentCount) || shard.inputDocumentCount <= 0)
  ) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 refinery shard input document count must be a positive finite number when present.',
    )
  }
}

/**
 * Admits receipted CS336 A4 refinery shards into a training run's public
 * projection. The public-safety guard runs at admission time on the
 * exact evidence that will be projected. Each shard names one
 * deterministic stage, its output-digest commitment, and the receipt
 * and verification refs that back it; eval-delta leaderboard rows are
 * intentionally not admitted here because no fixed-trainer eval loop
 * exists yet (the bonus stays design-only per the payment policy).
 */
export const admitCs336A4DataRefineryEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A4DataRefineryEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  if (input.request.shards.length === 0) {
    throw new DataRefineryEvidenceValidationError(
      'CS336 A4 data-refinery evidence requires at least one shard.',
    )
  }

  for (const shard of input.request.shards) {
    assertAdmissibleShard(shard)
  }

  const evidence = {
    jobKind: Cs336A4DataRefineryJobKind,
    psionicLaneRef: input.request.psionicLaneRef ?? Cs336A4PsionicLaneRef,
    receiptRefs: uniqueRefs([...(input.request.receiptRefs ?? [])]),
    shards: input.request.shards,
    sourceRefs: uniqueRefs([...(input.request.sourceRefs ?? [])]),
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}
  const existing = isRecord(projection.a4DataRefinery)
    ? projection.a4DataRefinery
    : undefined
  const existingLeaderboardRows = Array.isArray(existing?.leaderboardRows)
    ? existing?.leaderboardRows
    : []

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      a4DataRefinery: {
        ...evidence,
        // Preserve any previously admitted eval-delta leaderboard rows
        // without inventing new ones; the deterministic-stage admission
        // never fabricates eval-delta scores.
        leaderboardRows: existingLeaderboardRows,
      },
    }),
    updatedAt: input.nowIso,
  }
}

const refineryEvidenceRecord = (
  run: TrainingRunRecord,
): Record<string, unknown> | undefined => {
  const projection = parseJsonRecord(run.publicProjectionJson)
  const nested = projection?.a4DataRefinery

  return isRecord(nested) ? nested : undefined
}

const isStage = (value: unknown): value is Cs336A4HomeworkStage =>
  typeof value === 'string' &&
  (Cs336A4HomeworkStages as ReadonlyArray<string>).includes(value)

const shardsFromEvidence = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    evidence: Record<string, unknown> | undefined
    run: TrainingRunRecord
  }>,
): ReadonlyArray<DataRefineryShard> => {
  const shards = input.evidence?.shards

  if (!Array.isArray(shards)) {
    return []
  }

  const verifiedChallengeRefs = input.challenges
    .filter(challenge => challenge.state === 'Verified')
    .map(challenge => challenge.challengeRef)

  return shards.flatMap((shard, index): ReadonlyArray<DataRefineryShard> => {
    if (!isRecord(shard) || !isStage(shard.stage)) {
      return []
    }

    const outputDigestRef = optionalString(shard.outputDigestRef)

    if (outputDigestRef === undefined) {
      return []
    }

    const verificationRefs = uniqueRefs([
      ...stringArrayFromUnknown(shard.verificationRefs),
      ...verifiedChallengeRefs,
    ])

    return [
      {
        inputDocumentCount: optionalNumber(shard.inputDocumentCount) ?? null,
        outputDigestRef,
        pylonRef: optionalString(shard.pylonRef) ?? null,
        receiptRefs: uniqueRefs(stringArrayFromUnknown(shard.receiptRefs)),
        settledPayoutSats: 0,
        shardRef:
          optionalString(shard.shardRef) ??
          `training.data_refinery.${input.run.trainingRunRef}.shard.${index + 1}`,
        sourceRefs: uniqueRefs(stringArrayFromUnknown(shard.sourceRefs)),
        stage: shard.stage,
        trainingRunRef: input.run.trainingRunRef,
        verificationRefs,
        verified: verificationRefs.length > 0,
      },
    ]
  })
}

const evalDeltaBonusBlockerRefs = (): ReadonlyArray<string> => [
  'blocker.cs336_a4.fixed_trainer_eval_loop_required_for_quality_bonus',
  'blocker.cs336_a4.operator_funding_required_for_bonus_settlement',
  'blocker.cs336_a4.psionic_classifier_adapters_partial',
]

export const Cs336A4RequiredVerifiedStageCount = 3

/**
 * Builds the public CS336 A4 data-refinery projection for a run: the
 * receipted refinery shards, their verification state, and an honest
 * status. `stages_verified` requires at least three distinct stages
 * with a verified deterministic_recompute challenge (acceptance
 * criterion #1). The eval-delta quality bonus is reported separately as
 * blocked: deterministic stages pay per verified shard, never per
 * volume, and the bonus stays design-only until a fixed-trainer eval
 * loop and operator funding exist.
 */
export const publicDataRefineryProjection = (
  input: Readonly<{
    challenges: ReadonlyArray<TrainingVerificationChallengeRecord>
    leases: ReadonlyArray<TrainingWindowLeaseRecord>
    run: TrainingRunRecord
    windows: ReadonlyArray<TrainingWindowRecord>
  }>,
): DataRefineryProjection => {
  const evidence = refineryEvidenceRecord(input.run)
  const shards = shardsFromEvidence({ ...input, evidence })
  const verifiedShards = shards.filter(shard => shard.verified)
  const observedVerifiedStages = [
    ...new Set(verifiedShards.map(shard => shard.stage)),
  ].sort() as ReadonlyArray<Cs336A4HomeworkStage>
  const stagesVerified =
    observedVerifiedStages.length >= Cs336A4RequiredVerifiedStageCount
  const status =
    shards.length === 0
      ? 'blocked_no_shards'
      : stagesVerified
        ? 'stages_verified'
        : 'collecting_shards'

  return {
    blockerRefs:
      status === 'stages_verified'
        ? []
        : [
            'blocker.cs336_a4.requires_three_verified_stages',
            'blocker.cs336_a4.operator_funding_required_for_paid_shards',
          ],
    evalDeltaBonusBlockerRefs: evalDeltaBonusBlockerRefs(),
    observedVerifiedShardCount: verifiedShards.length,
    observedVerifiedStages,
    psionicLaneRef:
      optionalString(evidence?.psionicLaneRef) ?? Cs336A4PsionicLaneRef,
    requiredVerifiedStageCount: Cs336A4RequiredVerifiedStageCount,
    schemaVersion: 'openagents.training.data_refinery.v1',
    scopeBoundaryRefs: [
      'scope.cs336_a4.deterministic_recompute_verified_shards_only',
      'scope.cs336_a4.synthetic_public_safe_corpus_only',
      'scope.cs336_a4.pay_quality_delta_not_raw_volume_or_private_data',
      'scope.cs336_a4.no_settlement_without_provider_confirmation',
    ],
    shards,
    sourceRefs: uniqueRefs([
      'route:/api/training/refinery/a4',
      `route:/api/training/runs/${input.run.trainingRunRef}`,
      ...input.run.sourceRefs,
      ...input.windows.flatMap(window => window.sourceRefs),
      ...input.leases.map(lease => lease.leaseRef),
    ]),
    status,
  }
}
