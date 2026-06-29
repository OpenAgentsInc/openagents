import { Schema as S } from 'effect'

import {
  Cs336A5JobKinds,
  Cs336A5PsionicLaneRef,
  Cs336A5UpdateBoundaryRef,
} from './cs336-a5-alignment-homework'
import { isRecord, parseJsonRecord } from './json-boundary'
import type { TrainingRunRecord } from './training-run-window-authority'

const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const PublicSafeRef = NonEmptyTrimmedString.check(
  S.isMinLength(3),
  S.isMaxLength(260),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9_.:/-]*$/),
)
const PublicSafeRefs = S.optionalKey(S.Array(PublicSafeRef))

export const Cs336A5EvalSuiteEvidence = S.Struct({
  evalSuiteRef: S.optionalKey(PublicSafeRef),
  metric: S.Literals(['accuracy', 'pass_rate', 'reward_mean']),
  receiptRefs: S.Array(PublicSafeRef),
  sampleCount: S.Number,
  score: S.Number,
  sourceRefs: PublicSafeRefs,
  splitRef: PublicSafeRef,
  taskSetRef: S.Literals(['gsm8k', 'mmlu', 'math']),
  verificationRefs: PublicSafeRefs,
  verifiedSampleCount: S.Number,
})
export type Cs336A5EvalSuiteEvidence = typeof Cs336A5EvalSuiteEvidence.Type

export const Cs336A5WorkShardEvidence = S.Struct({
  jobKind: S.Literals([...Cs336A5JobKinds]),
  outputDigestRef: PublicSafeRef,
  pylonRef: S.optionalKey(PublicSafeRef),
  receiptRefs: S.Array(PublicSafeRef),
  rolloutCount: S.optionalKey(S.Number),
  shardRef: S.optionalKey(PublicSafeRef),
  sourceRefs: PublicSafeRefs,
  splitRef: S.optionalKey(PublicSafeRef),
  verificationRefs: PublicSafeRefs,
})
export type Cs336A5WorkShardEvidence = typeof Cs336A5WorkShardEvidence.Type

export const Cs336A5AlignmentEvidenceRequest = S.Struct({
  evalSuites: S.Array(Cs336A5EvalSuiteEvidence),
  psionicLaneRef: S.optionalKey(PublicSafeRef),
  receiptRefs: PublicSafeRefs,
  shards: S.optionalKey(S.Array(Cs336A5WorkShardEvidence)),
  sourceRefs: PublicSafeRefs,
})
export type Cs336A5AlignmentEvidenceRequest =
  typeof Cs336A5AlignmentEvidenceRequest.Type

export class AlignmentEvalUnsafeProjectionError extends Error {
  readonly _tag = 'AlignmentEvalUnsafeProjectionError'
}

export class AlignmentEvalEvidenceValidationError extends Error {
  readonly _tag = 'AlignmentEvalEvidenceValidationError'
}

/**
 * Public-safety guard for admitted A5 alignment evidence. Pylon refs
 * and output-digest refs are legitimate public provenance, but the
 * guard rejects raw prompts, answers, completions, wallet, payment,
 * invoice, mnemonic, key, and private-path material before it can
 * reach D1 — the same boundary the deployed A5 eval projection
 * enforces at read time.
 */
const unsafeAlignmentMaterialPattern =
  /(\"?(prompt|answer|completion|mnemonic|preimage|invoice|bolt11|bolt12|lno1|raw[A-Za-z0-9_-]*|secret[A-Za-z0-9_-]*|private[A-Za-z0-9_-]*|wallet[A-Za-z0-9_-]*)\"?\s*:|\/Users\/|\/home\/|api[_-]?key|bearer|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|raw[_-]?(completion|dataset|invoice|payment|payload|prompt|rollout)|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|path|seed|mnemonic|private))/i

const publicSafeJson = (value: unknown): string => {
  const json = JSON.stringify(value)

  if (unsafeAlignmentMaterialPattern.test(json)) {
    throw new AlignmentEvalUnsafeProjectionError(
      'CS336 A5 alignment evidence contains raw eval, wallet, payment, or private material.',
    )
  }

  return json
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertAdmissibleEvalSuite = (suite: Cs336A5EvalSuiteEvidence): void => {
  if (suite.receiptRefs.length === 0) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 eval suite evidence requires at least one receipt ref; unreceipted suites are not admissible.',
    )
  }

  if (!Number.isInteger(suite.sampleCount) || suite.sampleCount <= 0) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 eval suite sample count must be a positive integer.',
    )
  }

  if (
    !Number.isInteger(suite.verifiedSampleCount) ||
    suite.verifiedSampleCount < 0 ||
    suite.verifiedSampleCount > suite.sampleCount
  ) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 eval suite verified sample count must be an integer between zero and the sample count.',
    )
  }

  if (!Number.isFinite(suite.score)) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 eval suite score must be finite.',
    )
  }

  if (
    (suite.metric === 'accuracy' || suite.metric === 'pass_rate') &&
    (suite.score < 0 || suite.score > 1)
  ) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 accuracy and pass-rate scores must be within [0, 1].',
    )
  }
}

const assertAdmissibleShard = (shard: Cs336A5WorkShardEvidence): void => {
  if (shard.receiptRefs.length === 0) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 work shard evidence requires at least one receipt ref; unreceipted shards are not admissible.',
    )
  }

  if (
    shard.rolloutCount !== undefined &&
    (!Number.isInteger(shard.rolloutCount) || shard.rolloutCount <= 0)
  ) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 work shard rollout count must be a positive integer when present.',
    )
  }
}

/**
 * Admits receipted CS336 A5 alignment evidence into a training run's
 * public projection. Eval suites are the rows the deployed
 * `GET /api/training/evals/a5` projection serves; work shards record
 * the rollout/grading assignments (job kind, output-digest commitment,
 * receipt and verification refs) that produced them. The public-safety
 * guard runs at admission time on the exact evidence that will be
 * projected. Nothing here claims a policy-gradient update: that step
 * stays behind the #4669 training boundary.
 */
export const admitCs336A5AlignmentEvidence = (
  input: Readonly<{
    nowIso: string
    request: Cs336A5AlignmentEvidenceRequest
    run: TrainingRunRecord
  }>,
): TrainingRunRecord => {
  if (input.request.evalSuites.length === 0) {
    throw new AlignmentEvalEvidenceValidationError(
      'CS336 A5 alignment evidence requires at least one eval suite.',
    )
  }

  for (const suite of input.request.evalSuites) {
    assertAdmissibleEvalSuite(suite)
  }

  for (const shard of input.request.shards ?? []) {
    assertAdmissibleShard(shard)
  }

  const evidence = {
    evalSuites: input.request.evalSuites,
    jobKinds: Cs336A5JobKinds,
    psionicLaneRef: input.request.psionicLaneRef ?? Cs336A5PsionicLaneRef,
    receiptRefs: uniqueRefs([...(input.request.receiptRefs ?? [])]),
    shards: input.request.shards ?? [],
    sourceRefs: uniqueRefs([...(input.request.sourceRefs ?? [])]),
    updateBoundaryRef: Cs336A5UpdateBoundaryRef,
  }

  publicSafeJson(evidence)

  const projection = parseJsonRecord(input.run.publicProjectionJson) ?? {}
  const existing = isRecord(projection.a5Alignment)
    ? projection.a5Alignment
    : undefined

  return {
    ...input.run,
    publicProjectionJson: JSON.stringify({
      ...projection,
      a5Alignment: {
        ...existing,
        ...evidence,
      },
    }),
    updatedAt: input.nowIso,
  }
}
