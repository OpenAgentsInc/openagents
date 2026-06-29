import { Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import type { TrainingRunRecord } from './training-run-window-authority'

/**
 * Admission seam for CS336 A1 real-gradient evidence (issue #4678).
 *
 * `publicRealGradientStatus` in `training-run-window-authority.ts` reads
 * `publicProjectionJson.realGradient` but nothing could write that
 * evidence before this module: it is the previously missing admission
 * path for the A1 loss leaderboard. Admission enforces the issue's
 * acceptance bars on the exact JSON that will be projected:
 * receipted shard contributions only, at least two distinct contributor
 * devices, a strictly ordered finite loss curve, loss under the declared
 * budget, Freivalds commitment refs, gradient closeout refs, merge/eval
 * refs, and the public-safety guard.
 */

export const Cs336A1RealGradientJobKind = 'cs336_a1_homework'
export const Cs336A1RealGradientPsionicLaneRef =
  'psion_cs336_a1_real_gradient_reference_v1'
export const Cs336A1RealGradientWorkloadRef =
  'workload.cs336_a1.real_gradient_tiny_a1_lm.v1'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

export const Cs336A1RealGradientLossPointEvidence = S.Struct({
  sourceRefs: PublicSafeRefs,
  step: S.Number,
  validationLoss: S.Number,
})
export type Cs336A1RealGradientLossPointEvidence =
  typeof Cs336A1RealGradientLossPointEvidence.Type

export const Cs336A1RealGradientShardEvidence = S.Struct({
  dataUnitCount: S.Number,
  deviceClassRef: S.optionalKey(PublicSafeRef),
  gradientCommitmentRef: PublicSafeRef,
  pylonRef: PublicSafeRef,
  receiptRefs: S.Array(PublicSafeRef),
  shardIndex: S.Number,
  shardLoss: S.Number,
  sourceRefs: PublicSafeRefs,
  stepIndex: S.Number,
  verificationRefs: PublicSafeRefs,
})
export type Cs336A1RealGradientShardEvidence =
  typeof Cs336A1RealGradientShardEvidence.Type

export const Cs336A1RealGradientEvidenceRequest = S.Struct({
  budgetLabel: NonEmptyTrimmedString.check(S.isMaxLength(300)),
  budgetRef: PublicSafeRef,
  evalRef: PublicSafeRef,
  freivaldsCommitmentRefs: S.Array(PublicSafeRef),
  gradientCloseoutRefs: S.Array(PublicSafeRef),
  lossCurve: S.Array(Cs336A1RealGradientLossPointEvidence),
  lossSourceRefs: PublicSafeRefs,
  maxValidationLoss: S.Number,
  mergeRef: PublicSafeRef,
  psionicLaneRef: S.optionalKey(PublicSafeRef),
  receiptRefs: PublicSafeRefs,
  shardContributions: S.Array(Cs336A1RealGradientShardEvidence),
  sourceRefs: PublicSafeRefs,
})
export type Cs336A1RealGradientEvidenceRequest =
  typeof Cs336A1RealGradientEvidenceRequest.Type

export class RealGradientUnsafeProjectionError extends Error {
  readonly _tag = 'RealGradientUnsafeProjectionError'
}

export class RealGradientEvidenceValidationError extends Error {
  readonly _tag = 'RealGradientEvidenceValidationError'
}

/**
 * Public-safety guard for admitted A1 real-gradient evidence. Pylon
 * refs are legitimate public provenance on shard contributions; wallet,
 * payment, invoice, mnemonic, key, and private path material is
 * rejected before it can reach D1.
 */
const unsafeRealGradientMaterialPattern =
  /(\"?(mnemonic|preimage|invoice|bolt11|bolt12|lno1|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(dataset|invoice|payment|payload|prompt|runner)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeRealGradientMaterialPattern.test(json)) {
    throw new RealGradientUnsafeProjectionError(
      'CS336 A1 real-gradient projection contains wallet, payment, or private material.',
    )
  }

  return json
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertAdmissibleShard = (
  shard: Cs336A1RealGradientShardEvidence,
): void => {
  if (
    !Number.isFinite(shard.dataUnitCount) ||
    shard.dataUnitCount <= 0 ||
    !Number.isInteger(shard.shardIndex) ||
    shard.shardIndex < 0 ||
    !Number.isInteger(shard.stepIndex) ||
    shard.stepIndex < 0
  ) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient shard evidence requires non-negative integer step/shard indexes and a positive data-unit count.',
    )
  }

  if (!Number.isFinite(shard.shardLoss)) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient shard evidence requires a finite shard loss.',
    )
  }

  if (shard.receiptRefs.length === 0) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient shard evidence requires at least one receipt ref; unreceipted shard gradients are not admissible.',
    )
  }
}

const assertAdmissibleLossCurve = (
  request: Cs336A1RealGradientEvidenceRequest,
): void => {
  if (request.lossCurve.length < 2) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires a loss curve with at least two points.',
    )
  }

  for (let index = 0; index < request.lossCurve.length; index += 1) {
    const point = request.lossCurve[index]!

    if (
      !Number.isFinite(point.validationLoss) ||
      !Number.isInteger(point.step) ||
      point.step < 0
    ) {
      throw new RealGradientEvidenceValidationError(
        'CS336 A1 real-gradient loss points require non-negative integer steps and finite losses.',
      )
    }

    if (index > 0 && point.step <= request.lossCurve[index - 1]!.step) {
      throw new RealGradientEvidenceValidationError(
        'CS336 A1 real-gradient loss curve steps must be strictly increasing.',
      )
    }
  }

  if (
    !Number.isFinite(request.maxValidationLoss) ||
    request.maxValidationLoss <= 0
  ) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires a positive finite maxValidationLoss budget.',
    )
  }

  const finalLoss =
    request.lossCurve[request.lossCurve.length - 1]!.validationLoss

  if (finalLoss > request.maxValidationLoss) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires the final validation loss at or below the declared budget.',
    )
  }
}

/**
 * Admits receipted CS336 A1 real-gradient evidence into a training
 * run's public projection under the exact key that
 * `publicRealGradientStatus` reads.
 */
export const admitCs336A1RealGradientEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A1RealGradientEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  const { request } = input

  assertAdmissibleLossCurve(request)

  if (request.shardContributions.length === 0) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires at least one shard contribution.',
    )
  }

  for (const shard of request.shardContributions) {
    assertAdmissibleShard(shard)
  }

  const distinctPylons = new Set(
    request.shardContributions.map(shard => shard.pylonRef),
  )

  if (distinctPylons.size < 2) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires shard contributions from at least two distinct contributor devices.',
    )
  }

  if (request.freivaldsCommitmentRefs.length === 0) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires at least one Freivalds commitment ref.',
    )
  }

  if (request.gradientCloseoutRefs.length === 0) {
    throw new RealGradientEvidenceValidationError(
      'CS336 A1 real-gradient evidence requires at least one gradient closeout ref.',
    )
  }

  const evidence = {
    budgetLabel: request.budgetLabel,
    budgetRef: request.budgetRef,
    evalRef: request.evalRef,
    freivaldsCommitmentRefs: uniqueRefs([...request.freivaldsCommitmentRefs]),
    gradientCloseoutRefs: uniqueRefs([...request.gradientCloseoutRefs]),
    jobKind: Cs336A1RealGradientJobKind,
    lossCurve: request.lossCurve.map(point => ({
      sourceRefs: uniqueRefs([...(point.sourceRefs ?? [])]),
      step: point.step,
      validationLoss: point.validationLoss,
    })),
    lossSourceRefs: uniqueRefs([...(request.lossSourceRefs ?? [])]),
    maxValidationLoss: request.maxValidationLoss,
    mergeRef: request.mergeRef,
    psionicLaneRef:
      request.psionicLaneRef ?? Cs336A1RealGradientPsionicLaneRef,
    receiptRefs: uniqueRefs([...(request.receiptRefs ?? [])]),
    shardContributions: request.shardContributions.map(shard => ({
      dataUnitCount: shard.dataUnitCount,
      ...(shard.deviceClassRef === undefined
        ? {}
        : { deviceClassRef: shard.deviceClassRef }),
      gradientCommitmentRef: shard.gradientCommitmentRef,
      pylonRef: shard.pylonRef,
      receiptRefs: uniqueRefs([...shard.receiptRefs]),
      shardIndex: shard.shardIndex,
      shardLoss: shard.shardLoss,
      sourceRefs: uniqueRefs([...(shard.sourceRefs ?? [])]),
      stepIndex: shard.stepIndex,
      verificationRefs: uniqueRefs([...(shard.verificationRefs ?? [])]),
    })),
    sourceRefs: uniqueRefs([
      ...(request.sourceRefs ?? []),
      Cs336A1RealGradientWorkloadRef,
    ]),
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      realGradient: evidence,
    }),
    updatedAt: input.nowIso,
  }
}
