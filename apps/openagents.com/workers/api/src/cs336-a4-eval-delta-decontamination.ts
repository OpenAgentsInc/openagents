import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4EvalDeltaMeasurement } from './cs336-a4-eval-delta-payment'

/**
 * Eval-delta decontamination receipt for CS336 A4 refinery shards
 * (`blocker.product_promises.eval_delta_payment_missing`).
 *
 * The eval-delta bonus pays a contributor for a positive downstream eval
 * delta (`settleCs336A4EvalDeltaPayment`) measured by a held-constant
 * trainer on a `heldOutEvalSetRef` the contributor does not control. The
 * settlement closeout (`closeCs336A4EvalDeltaSettlement`) already binds the
 * measurement to the shard's corpus SOURCE
 * (`verifyCs336A4EvalDeltaMeasurementBinding`) and refuses to price a bonus
 * for an unverified stage. But it has no evidence that the shard's corpus was
 * DECONTAMINATED against that held-out eval set. That is the classic way to
 * game an eval-delta: leak examples from the held-out eval set into the
 * "filtered" corpus and the filtered score rises for free — a positive delta
 * that reflects memorisation, not data quality. Without a decontamination
 * receipt, every assignment/source/recompute check still passes and the gamed
 * bonus is paid.
 *
 * This module is the missing anti-gaming evidence. It builds a deterministic,
 * content-addressed, public-safe receipt attesting that ONE shard's corpus was
 * checked for overlap against ONE held-out eval set under a declared method,
 * and that every detected contaminated span was removed and the post-removal
 * digest recompute-verified. `clean` is true only when the corpus is fully
 * decontaminated and verified. The binding gate
 * (`assertCs336A4EvalDeltaDecontamination`) refuses to clear a bonus unless a
 * CLEAN receipt covers exactly the measurement's source AND held-out eval set.
 *
 * It carries metadata only — refs, digests, n-gram size, and span counts. It
 * never carries raw corpus text, eval-set contents, wallet, or payment
 * material; the public-safety guard fails closed before any such material can
 * be committed.
 */

export const Cs336A4EvalDeltaDecontaminationSchemaVersion =
  'openagents.training.data_refinery.eval_delta_decontamination.v1' as const

/**
 * Why a shard's corpus is NOT clean against the held-out eval set. A `clean`
 * receipt carries none of these.
 */
export const Cs336A4EvalDeltaDecontaminationReasons = [
  'contaminated_spans_not_fully_removed',
  'post_decontamination_recompute_unverified',
] as const
export type Cs336A4EvalDeltaDecontaminationReason =
  (typeof Cs336A4EvalDeltaDecontaminationReasons)[number]

export const Cs336A4EvalDeltaDecontaminationBindingMismatches = [
  'source_ref_mismatch',
  'held_out_eval_set_ref_mismatch',
  'receipt_not_clean',
] as const
export type Cs336A4EvalDeltaDecontaminationBindingMismatch =
  (typeof Cs336A4EvalDeltaDecontaminationBindingMismatches)[number]

export type Cs336A4EvalDeltaDecontaminationReceipt = Readonly<{
  /** SHA-256 over the canonical receipt body (hex). */
  contentDigestRef: string
  /** Number of contaminated spans detected in the corpus. */
  contaminatedSpansDetected: number
  /** Number of contaminated spans removed from the corpus. */
  contaminatedSpansRemoved: number
  /** Held-out eval set the corpus was decontaminated against. */
  heldOutEvalSetRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  /** Decontamination method (e.g. n-gram overlap) used. */
  methodRef: string
  /** Contiguous token n-gram size used to detect overlap; positive integer. */
  ngramSize: number
  /** Corpus digest before contaminated spans were removed. */
  preDecontaminationDigestRef: string
  /** Corpus digest after contaminated spans were removed. */
  postDecontaminationDigestRef: string
  /** True only when all detected spans removed AND post digest recompute-verified. */
  clean: boolean
  /** Digest a verifier recomputed for the post-removal corpus; must match post. */
  recomputedPostDigestRef: string
  /** Content-addressed receipt ref derived from contentDigestRef. */
  receiptRef: string
  schemaVersion: typeof Cs336A4EvalDeltaDecontaminationSchemaVersion
  /** Corpus source this receipt decontaminates. */
  sourceRef: string
}>

export class Cs336A4EvalDeltaDecontaminationValidationError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaDecontaminationValidationError'
}

export class Cs336A4EvalDeltaDecontaminationUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaDecontaminationUnsafeMaterialError'
}

export class Cs336A4EvalDeltaDecontaminationError extends Error {
  readonly _tag = 'Cs336A4EvalDeltaDecontaminationError'
  readonly reason: Cs336A4EvalDeltaDecontaminationBindingMismatch

  constructor(
    reason: Cs336A4EvalDeltaDecontaminationBindingMismatch,
    detail: string,
  ) {
    super(detail)
    this.reason = reason
  }
}

const unsafeDecontaminationMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(corpus|crawl|dataset|eval|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeDecontaminationMaterialPattern.test(json)) {
    throw new Cs336A4EvalDeltaDecontaminationUnsafeMaterialError(
      'CS336 A4 eval-delta decontamination receipt contains raw corpus, eval-set, wallet, payment, or private material.',
    )
  }
}

const sha256Hex = async (value: string): Promise<string> => {
  const digest = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value),
  )

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

const requireNonEmptyRef = (label: string, value: string): string => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      `CS336 A4 eval-delta decontamination receipt requires a non-empty ${label}.`,
    )
  }

  return trimmed
}

const requireNonNegativeInteger = (label: string, value: number): number => {
  if (!Number.isInteger(value) || value < 0) {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      `CS336 A4 eval-delta decontamination receipt requires ${label} to be a non-negative integer.`,
    )
  }

  return value
}

const requirePositiveInteger = (label: string, value: number): number => {
  if (!Number.isInteger(value) || value <= 0) {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      `CS336 A4 eval-delta decontamination receipt requires ${label} to be a positive integer.`,
    )
  }

  return value
}

/**
 * Canonical receipt body with fields in a fixed order so the content digest is
 * stable regardless of caller key ordering.
 */
const canonicalReceiptBody = (
  input: Readonly<{
    contaminatedSpansDetected: number
    contaminatedSpansRemoved: number
    heldOutEvalSetRef: string
    methodRef: string
    ngramSize: number
    preDecontaminationDigestRef: string
    postDecontaminationDigestRef: string
    clean: boolean
    recomputedPostDigestRef: string
    sourceRef: string
  }>,
): string =>
  JSON.stringify({
    clean: input.clean,
    contaminatedSpansDetected: input.contaminatedSpansDetected,
    contaminatedSpansRemoved: input.contaminatedSpansRemoved,
    heldOutEvalSetRef: input.heldOutEvalSetRef,
    jobKind: Cs336A4DataRefineryJobKind,
    methodRef: input.methodRef,
    ngramSize: input.ngramSize,
    postDecontaminationDigestRef: input.postDecontaminationDigestRef,
    preDecontaminationDigestRef: input.preDecontaminationDigestRef,
    recomputedPostDigestRef: input.recomputedPostDigestRef,
    schemaVersion: Cs336A4EvalDeltaDecontaminationSchemaVersion,
    sourceRef: input.sourceRef,
  })

/**
 * Builds a deterministic, public-safe eval-delta decontamination receipt for
 * one refinery shard. Fails closed when:
 *  - any required ref is empty or any count is not a valid integer,
 *  - more spans were removed than were detected (`contaminatedSpansRemoved >
 *    contaminatedSpansDetected`),
 *  - no spans were detected yet the corpus digest changed (a removal with
 *    nothing to remove), or detected spans yet the corpus digest is unchanged
 *    (a claimed detection that removed nothing),
 *  - the receipt would carry raw corpus, eval-set, wallet, or private material.
 *
 * The receipt is `clean` only when every detected span was removed AND the
 * post-removal digest recompute-verified. A non-clean receipt is still a valid,
 * auditable artifact — it just cannot clear a bonus.
 *
 * The returned `receiptRef` is content-addressed: it is derived from a SHA-256
 * over the canonical receipt body, so the same inputs always yield the same ref.
 */
export const buildCs336A4EvalDeltaDecontaminationReceipt = async (
  input: Readonly<{
    contaminatedSpansDetected: number
    contaminatedSpansRemoved: number
    heldOutEvalSetRef: string
    methodRef: string
    ngramSize: number
    preDecontaminationDigestRef: string
    postDecontaminationDigestRef: string
    recomputedPostDigestRef: string
    sourceRef: string
  }>,
): Promise<Cs336A4EvalDeltaDecontaminationReceipt> => {
  const sourceRef = requireNonEmptyRef('sourceRef', input.sourceRef)
  const heldOutEvalSetRef = requireNonEmptyRef(
    'heldOutEvalSetRef',
    input.heldOutEvalSetRef,
  )
  const methodRef = requireNonEmptyRef('methodRef', input.methodRef)
  const preDecontaminationDigestRef = requireNonEmptyRef(
    'preDecontaminationDigestRef',
    input.preDecontaminationDigestRef,
  )
  const postDecontaminationDigestRef = requireNonEmptyRef(
    'postDecontaminationDigestRef',
    input.postDecontaminationDigestRef,
  )
  const recomputedPostDigestRef = requireNonEmptyRef(
    'recomputedPostDigestRef',
    input.recomputedPostDigestRef,
  )
  const ngramSize = requirePositiveInteger('ngramSize', input.ngramSize)
  const contaminatedSpansDetected = requireNonNegativeInteger(
    'contaminatedSpansDetected',
    input.contaminatedSpansDetected,
  )
  const contaminatedSpansRemoved = requireNonNegativeInteger(
    'contaminatedSpansRemoved',
    input.contaminatedSpansRemoved,
  )

  if (contaminatedSpansRemoved > contaminatedSpansDetected) {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      'CS336 A4 eval-delta decontamination receipt removed more spans than it detected.',
    )
  }

  const digestChanged =
    preDecontaminationDigestRef !== postDecontaminationDigestRef

  // A removal that changed the corpus must have detected something to remove.
  if (digestChanged && contaminatedSpansDetected === 0) {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      'CS336 A4 eval-delta decontamination receipt changed the corpus digest but detected no contaminated spans.',
    )
  }

  // Detecting spans but leaving the corpus byte-identical means nothing was
  // actually removed; that cannot be a clean, honest removal.
  if (!digestChanged && contaminatedSpansDetected > 0) {
    throw new Cs336A4EvalDeltaDecontaminationValidationError(
      'CS336 A4 eval-delta decontamination receipt detected contaminated spans but left the corpus digest unchanged.',
    )
  }

  const recomputeVerified =
    recomputedPostDigestRef === postDecontaminationDigestRef
  const fullyRemoved = contaminatedSpansRemoved === contaminatedSpansDetected
  const clean = recomputeVerified && fullyRemoved

  const body = canonicalReceiptBody({
    clean,
    contaminatedSpansDetected,
    contaminatedSpansRemoved,
    heldOutEvalSetRef,
    methodRef,
    ngramSize,
    postDecontaminationDigestRef,
    preDecontaminationDigestRef,
    recomputedPostDigestRef,
    sourceRef,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const receiptRef = `receipt.cs336_a4.eval_delta_decontamination.${contentDigestRef.slice(0, 16)}`

  const receipt: Cs336A4EvalDeltaDecontaminationReceipt = {
    clean,
    contaminatedSpansDetected,
    contaminatedSpansRemoved,
    contentDigestRef,
    heldOutEvalSetRef,
    jobKind: Cs336A4DataRefineryJobKind,
    methodRef,
    ngramSize,
    postDecontaminationDigestRef,
    preDecontaminationDigestRef,
    receiptRef,
    recomputedPostDigestRef,
    schemaVersion: Cs336A4EvalDeltaDecontaminationSchemaVersion,
    sourceRef,
  }

  assertJsonPublicSafe(JSON.stringify(receipt))

  return receipt
}

export type Cs336A4EvalDeltaDecontaminationResult =
  | Readonly<{
      covered: true
      receiptRef: string
    }>
  | Readonly<{
      covered: false
      detail: string
      reason: Cs336A4EvalDeltaDecontaminationBindingMismatch
    }>

/**
 * Decides whether a decontamination receipt covers exactly the eval-delta
 * `measurement` that would price a bonus: same corpus source, same held-out
 * eval set, and the receipt is `clean`. A pure comparison over two already-built
 * artifacts — it fetches, fabricates, and mutates nothing. Returns a
 * discriminated result so a settlement/admission path can record the precise
 * mismatch reason. Empty refs on either side fail closed with a validation
 * error rather than silently comparing as equal.
 */
export const verifyCs336A4EvalDeltaDecontamination = (
  input: Readonly<{
    measurement: Cs336A4EvalDeltaMeasurement
    decontaminationReceipt: Cs336A4EvalDeltaDecontaminationReceipt
  }>,
): Cs336A4EvalDeltaDecontaminationResult => {
  const { decontaminationReceipt, measurement } = input

  const measurementSourceRef = requireNonEmptyRef(
    'measurement.sourceRef',
    measurement.sourceRef,
  )
  const receiptSourceRef = requireNonEmptyRef(
    'decontaminationReceipt.sourceRef',
    decontaminationReceipt.sourceRef,
  )
  const measurementEvalSetRef = requireNonEmptyRef(
    'measurement.heldOutEvalSetRef',
    measurement.heldOutEvalSetRef,
  )
  const receiptEvalSetRef = requireNonEmptyRef(
    'decontaminationReceipt.heldOutEvalSetRef',
    decontaminationReceipt.heldOutEvalSetRef,
  )

  if (measurementSourceRef !== receiptSourceRef) {
    return {
      covered: false,
      detail: `decontamination receipt source (${receiptSourceRef}) does not match the eval-delta measurement source (${measurementSourceRef}); the corpus paid for is not the corpus decontaminated.`,
      reason: 'source_ref_mismatch',
    }
  }

  if (measurementEvalSetRef !== receiptEvalSetRef) {
    return {
      covered: false,
      detail: `decontamination receipt eval set (${receiptEvalSetRef}) does not match the eval-delta held-out eval set (${measurementEvalSetRef}); the bonus was measured against an eval set the corpus was not decontaminated against.`,
      reason: 'held_out_eval_set_ref_mismatch',
    }
  }

  if (!decontaminationReceipt.clean) {
    return {
      covered: false,
      detail:
        'decontamination receipt is not clean: not every detected contaminated span was removed, or the post-removal digest did not recompute-verify.',
      reason: 'receipt_not_clean',
    }
  }

  return { covered: true, receiptRef: decontaminationReceipt.receiptRef }
}

/**
 * Fail-closed wrapper around `verifyCs336A4EvalDeltaDecontamination`: throws
 * `Cs336A4EvalDeltaDecontaminationError` (carrying the mismatch reason) when no
 * clean receipt covers the measurement's source AND held-out eval set, and
 * returns the receipt's content-addressed `receiptRef` when one does. Use this
 * on a settlement/closeout path where a gamed eval delta (eval leakage in the
 * corpus) must hard-fail before a bonus is priced or recorded.
 */
export const assertCs336A4EvalDeltaDecontamination = (
  input: Readonly<{
    measurement: Cs336A4EvalDeltaMeasurement
    decontaminationReceipt: Cs336A4EvalDeltaDecontaminationReceipt
  }>,
): string => {
  const result = verifyCs336A4EvalDeltaDecontamination(input)

  if (!result.covered) {
    throw new Cs336A4EvalDeltaDecontaminationError(
      result.reason,
      `CS336 A4 eval-delta bonus is not decontaminated: ${result.detail}`,
    )
  }

  return result.receiptRef
}
