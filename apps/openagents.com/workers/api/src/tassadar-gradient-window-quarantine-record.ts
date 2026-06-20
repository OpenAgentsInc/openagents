import { Schema as S } from 'effect'

import { currentIsoTimestamp } from './runtime-primitives'
import {
  TassadarGradientWindowIntakeSchemaVersion,
  admitTassadarGradientWindowToQuarantine,
} from './tassadar-gradient-window-intake'
import { TassadarGradientWindowCandidate } from './tassadar-gradient-window-regime'

/**
 * Quarantine record for training.public_gradient_windows.v1.
 *
 * The intake admission predicate (tassadar-gradient-window-intake.ts) decides
 * whether a freshly submitted candidate MAY enter quarantine. It returns a
 * decision with a `quarantineRecordRef`, but it does not produce the durable
 * artifact a quarantine store would actually persist. That persisted record is
 * what this module supplies: the canonical, public-safe row representing one
 * admitted window living in quarantine and the verification work it still owes
 * before it could ever promote.
 *
 * `buildTassadarGradientWindowQuarantineRecord` is a pure, deterministic
 * function. It first re-runs the admission predicate; if the submission is not
 * admitted (malformed, unsafe, compiled-core targeting, frozen-core mutating,
 * or missing required evidence) it REFUSES to build a record and throws
 * `TassadarGradientWindowQuarantineRecordUnsafe` carrying the rejection reasons.
 * A quarantine record can therefore never be manufactured for a submission that
 * was not admitted. The record grants quarantine residency only: it confers no
 * promotion, settlement, canonical-checkpoint mutation, compiled-core-gradient
 * mutation, or direct-submission authority.
 *
 * This advances blocker.product_promises.public_gradient_live_window_runtime_missing
 * by building the runtime's quarantine persistence format — the edge after
 * admission. It does NOT clear that blocker: no live store persists these
 * records, no route serves them, and no public window has been accepted,
 * promoted, paid, or settled.
 */

export const TassadarGradientWindowQuarantineRecordSchemaVersion =
  'openagents.training.public_gradient_window.quarantine_record.v1'
export type TassadarGradientWindowQuarantineRecordSchemaVersion =
  typeof TassadarGradientWindowQuarantineRecordSchemaVersion

/**
 * Verification stages a quarantined window must still clear, in order, before
 * the regime gate could allow promotion. Surfaced so a runtime knows exactly
 * what work an admitted record awaits.
 */
export const TassadarGradientWindowPendingVerificationStages = [
  'recomputed',
  'replicated',
  'canary_passed',
  'promoted',
] as const

export const TassadarGradientWindowQuarantineRecord = S.Struct({
  admittedAt: S.String,
  authority: S.Struct({
    canonicalCheckpointMutationAllowed: S.Literal(false),
    compiledCoreGradientMutationAllowed: S.Literal(false),
    directSubmissionMutationAllowed: S.Literal(false),
    promotionAllowed: S.Literal(false),
    quarantineResidencyGranted: S.Literal(true),
    settlementMutationAllowed: S.Literal(false),
  }),
  authorityBoundary: S.String,
  candidateDigests: S.Struct({
    baseCheckpointDigest: S.String,
    datasetShardDigest: S.String,
    frozenCoreDigestAfter: S.String,
    frozenCoreDigestBefore: S.String,
    learnedInterfaceDigest: S.String,
    optimizerStateDigest: S.String,
    quarantineCheckpointDigest: S.String,
    trainingConfigDigest: S.String,
    updateDigest: S.String,
  }),
  compiledCoreUnchanged: S.Literal(true),
  evidenceRefs: S.Struct({
    constructionReceiptRefs: S.Array(S.String),
    curatedDataRefs: S.Array(S.String),
    psionicH1EvidenceRefs: S.Array(S.String),
    verificationReceiptRefs: S.Array(S.String),
  }),
  frozenParameterScopes: S.Array(S.String),
  identity: S.Struct({
    compiledCoreRef: S.String,
    contributorRef: S.String,
    modelFamilyRef: S.String,
    randomSeedRef: S.String,
  }),
  intakeSchemaVersion: S.Literal(TassadarGradientWindowIntakeSchemaVersion),
  pendingVerificationStages: S.Array(S.String),
  publicSafe: S.Literal(true),
  recordRef: S.String,
  schemaVersion: S.Literal(TassadarGradientWindowQuarantineRecordSchemaVersion),
  sourceRefs: S.Array(S.String),
  stage: S.Literal('quarantined'),
  trainableParameterScopes: S.Array(S.String),
  windowRef: S.String,
})
export type TassadarGradientWindowQuarantineRecord =
  typeof TassadarGradientWindowQuarantineRecord.Type

export class TassadarGradientWindowQuarantineRecordUnsafe extends S.TaggedErrorClass<TassadarGradientWindowQuarantineRecordUnsafe>()(
  'TassadarGradientWindowQuarantineRecordUnsafe',
  {
    rejectionReasonRefs: S.Array(S.String),
    reason: S.String,
  },
) {}

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9_.:/-]/g, '_').slice(0, 120)

const unsafeRecordPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|bearer|cookie|email[_-]?(address|body|raw)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|lnbc|lntb|lno1|mnemonic|payment[_-]?(hash|preimage|secret)|preimage|seed[_-]?phrase|sk-[a-z0-9]|wallet[_-]?(home|mnemonic|path|private|seed))/i

/**
 * Derive the canonical, public-safe quarantine record ref from a window ref so
 * the same admitted window always maps to the same record id. This matches the
 * `quarantineRecordRef` the intake admission predicate emits.
 */
export const tassadarGradientWindowQuarantineRecordRef = (
  windowRef: string,
): string =>
  `quarantine.public.tassadar_gradient_window.${safeSuffix(windowRef)}`

const decodeCandidate = S.decodeUnknownSync(TassadarGradientWindowCandidate)

/**
 * Build the durable, public-safe quarantine record for an admitted submission.
 *
 * Refuses (throws TassadarGradientWindowQuarantineRecordUnsafe) when the
 * submission does not pass the intake admission predicate, so a record can
 * never be fabricated for a window that was not admitted to quarantine.
 */
export const buildTassadarGradientWindowQuarantineRecord = (
  submission: unknown,
  options: { admittedAt?: string | undefined } = {},
): TassadarGradientWindowQuarantineRecord => {
  const decision = admitTassadarGradientWindowToQuarantine(submission)
  if (!decision.admitted || decision.quarantineRecordRef === null) {
    throw new TassadarGradientWindowQuarantineRecordUnsafe({
      rejectionReasonRefs: decision.rejectionReasonRefs,
      reason:
        'A quarantine record may only be built for a submission admitted to quarantine by the intake admission predicate.',
    })
  }

  const candidate = decodeCandidate(submission)

  const record = TassadarGradientWindowQuarantineRecord.make({
    admittedAt: options.admittedAt ?? currentIsoTimestamp(),
    authority: {
      canonicalCheckpointMutationAllowed: false,
      compiledCoreGradientMutationAllowed: false,
      directSubmissionMutationAllowed: false,
      promotionAllowed: false,
      quarantineResidencyGranted: true,
      settlementMutationAllowed: false,
    },
    authorityBoundary:
      'A quarantine record holds one admitted public gradient window in quarantine while it awaits recompute, replication, and canary verification. It grants quarantine residency only; it grants no promotion, settlement, canonical-checkpoint mutation, compiled-core-gradient mutation, or direct-submission authority. Admission is not acceptance: the window can still be blocked by the regime gate.',
    candidateDigests: {
      baseCheckpointDigest: candidate.baseCheckpointDigest,
      datasetShardDigest: candidate.datasetShardDigest,
      frozenCoreDigestAfter: candidate.frozenCoreDigestAfter,
      frozenCoreDigestBefore: candidate.frozenCoreDigestBefore,
      learnedInterfaceDigest: candidate.learnedInterfaceDigest,
      optimizerStateDigest: candidate.optimizerStateDigest,
      quarantineCheckpointDigest: candidate.quarantineCheckpointDigest,
      trainingConfigDigest: candidate.trainingConfigDigest,
      updateDigest: candidate.updateDigest,
    },
    compiledCoreUnchanged: true,
    evidenceRefs: {
      constructionReceiptRefs: [...candidate.constructionReceiptRefs].sort(),
      curatedDataRefs: [...candidate.curatedDataRefs].sort(),
      psionicH1EvidenceRefs: [...candidate.psionicH1EvidenceRefs].sort(),
      verificationReceiptRefs: [...candidate.verificationReceiptRefs].sort(),
    },
    frozenParameterScopes: [...candidate.frozenParameterScopes].sort(),
    identity: {
      compiledCoreRef: candidate.compiledCoreRef,
      contributorRef: candidate.contributorRef,
      modelFamilyRef: candidate.modelFamilyRef,
      randomSeedRef: candidate.randomSeedRef,
    },
    intakeSchemaVersion: TassadarGradientWindowIntakeSchemaVersion,
    pendingVerificationStages: [...TassadarGradientWindowPendingVerificationStages],
    publicSafe: true,
    recordRef: decision.quarantineRecordRef,
    schemaVersion: TassadarGradientWindowQuarantineRecordSchemaVersion,
    sourceRefs: [...candidate.sourceRefs].sort(),
    stage: 'quarantined',
    trainableParameterScopes: [...candidate.trainableParameterScopes].sort(),
    windowRef: candidate.windowRef,
  })

  if (unsafeRecordPattern.test(JSON.stringify(record))) {
    throw new TassadarGradientWindowQuarantineRecordUnsafe({
      rejectionReasonRefs: [
        'blocker.public.tassadar_gradient_window.quarantine_record.unsafe_material',
      ],
      reason:
        'A quarantine record must not carry private, credential, payment, or raw material.',
    })
  }

  return record
}
