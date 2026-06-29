import {
  Cs336A4DataRefineryJobKind,
  Cs336A4HomeworkStages,
  type Cs336A4HomeworkStage,
} from './cs336-a4-data-refinery'

/**
 * Corpus provenance receipts for CS336 A4 refinery shards.
 *
 * The deterministic refinery core (PII masking, Gopher rules, exact and
 * MinHash dedup) already commits a SHA-256 digest over each stage
 * output, but a refinery shard that only carries a single
 * `outputDigestRef` cannot prove WHERE its corpus came from or that the
 * sequence of transforms applied to it is internally consistent. The
 * `training.data_refinery_corpus.v1` promise requires that "every shard
 * carrying source-provenance and transform digests" — this module
 * builds exactly that artifact: a deterministic, public-safe receipt
 * that binds a source's provenance (origin / snapshot / license /
 * acquisition mode) to a CHAIN-LINKED set of per-stage transform
 * digests, where each stage's input digest must equal the prior stage's
 * output digest and each stage's recomputed digest must equal its
 * committed output digest. The receipt is content-addressed: its
 * `receiptRef` is derived from a SHA-256 over the canonical receipt
 * body, so two parties who hold the same provenance + transform chain
 * derive the same receipt ref.
 *
 * This module emits provenance metadata only — refs, digests, counts,
 * and license identifiers. It never carries raw crawl payload,
 * contributor content, wallet, or payment material; the public-safety
 * guard fails closed before any such material can be committed.
 */

export const Cs336A4ProvenanceSchemaVersion =
  'openagents.training.data_refinery.provenance.v1' as const

/**
 * How a source corpus was acquired. The current Psion corpus is a
 * frozen bounded synthetic mixture (`bounded_synthetic_corpus`); the
 * crawl-scale modes are listed so the receipt shape is forward-stable,
 * but acquiring crawl-scale corpora remains a separate planned blocker.
 */
export const Cs336A4AcquisitionModes = [
  'bounded_synthetic_corpus',
  'licensed_public_dataset',
  'public_crawl_snapshot',
] as const
export type Cs336A4AcquisitionMode = (typeof Cs336A4AcquisitionModes)[number]

export type Cs336A4SourceProvenance = Readonly<{
  acquisitionMode: Cs336A4AcquisitionMode
  /** License / usage-terms identifier under which the source is admitted. */
  licenseRef: string
  /** Immutable snapshot identifier (e.g. crawl snapshot id or seed digest). */
  snapshotRef: string
  /** Stable origin identifier for the source corpus. */
  sourceRef: string
}>

export type Cs336A4TransformStep = Readonly<{
  /** Refinery code version that produced this transform, for replay. */
  codeVersionRef: string
  /** Digest of the input this stage consumed. */
  inputDigestRef: string
  /** Digest of the output this stage committed. */
  outputDigestRef: string
  /** Digest a verifier recomputed; must equal outputDigestRef. */
  recomputedDigestRef: string
  stage: Cs336A4HomeworkStage
}>

export type Cs336A4ProvenanceReceipt = Readonly<{
  assignmentRef: string
  /** SHA-256 over the canonical receipt body (hex). */
  contentDigestRef: string
  finalOutputDigestRef: string
  inputShardRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  provenance: Cs336A4SourceProvenance
  /** Content-addressed receipt ref derived from contentDigestRef. */
  receiptRef: string
  /** True when every transform step's recompute matched its output. */
  recomputeVerified: boolean
  schemaVersion: typeof Cs336A4ProvenanceSchemaVersion
  /** Digest of the raw source input that enters the chain. */
  sourceInputDigestRef: string
  transformChain: ReadonlyArray<Cs336A4TransformStep>
}>

export class Cs336A4ProvenanceValidationError extends Error {
  readonly _tag = 'Cs336A4ProvenanceValidationError'
}

export class Cs336A4ProvenanceUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4ProvenanceUnsafeMaterialError'
}

const unsafeProvenanceMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeProvenanceMaterialPattern.test(json)) {
    throw new Cs336A4ProvenanceUnsafeMaterialError(
      'CS336 A4 provenance receipt contains crawl payload, wallet, payment, or private material.',
    )
  }
}

const assertPublicSafe = (value: unknown): void => {
  assertJsonPublicSafe(JSON.stringify(value))
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

const isStage = (value: string): value is Cs336A4HomeworkStage =>
  (Cs336A4HomeworkStages as ReadonlyArray<string>).includes(value)

const requireNonEmptyRef = (label: string, value: string): string => {
  const trimmed = value.trim()

  if (trimmed === '') {
    throw new Cs336A4ProvenanceValidationError(
      `CS336 A4 provenance receipt requires a non-empty ${label}.`,
    )
  }

  return trimmed
}

/**
 * Builds the canonical receipt body with fields in a fixed order so the
 * content digest is stable regardless of caller key ordering. The
 * transform chain order is preserved because it is a sequence, not a
 * set.
 */
const canonicalReceiptBody = (
  input: Readonly<{
    assignmentRef: string
    finalOutputDigestRef: string
    inputShardRef: string
    provenance: Cs336A4SourceProvenance
    recomputeVerified: boolean
    sourceInputDigestRef: string
    transformChain: ReadonlyArray<Cs336A4TransformStep>
  }>,
): string =>
  JSON.stringify({
    assignmentRef: input.assignmentRef,
    finalOutputDigestRef: input.finalOutputDigestRef,
    inputShardRef: input.inputShardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    provenance: {
      acquisitionMode: input.provenance.acquisitionMode,
      licenseRef: input.provenance.licenseRef,
      snapshotRef: input.provenance.snapshotRef,
      sourceRef: input.provenance.sourceRef,
    },
    recomputeVerified: input.recomputeVerified,
    schemaVersion: Cs336A4ProvenanceSchemaVersion,
    sourceInputDigestRef: input.sourceInputDigestRef,
    transformChain: input.transformChain.map(step => ({
      codeVersionRef: step.codeVersionRef,
      inputDigestRef: step.inputDigestRef,
      outputDigestRef: step.outputDigestRef,
      recomputedDigestRef: step.recomputedDigestRef,
      stage: step.stage,
    })),
  })

/**
 * Builds a deterministic, public-safe corpus provenance receipt for one
 * refinery shard. Fails closed when:
 *  - the transform chain is empty or names an unknown stage,
 *  - the chain is not linked (a stage's input digest must equal the
 *    prior stage's output digest, and the first stage's input must equal
 *    the declared source input digest),
 *  - the declared final output digest does not equal the last stage's
 *    output digest,
 *  - any step's recomputed digest does not equal its committed output
 *    digest (deterministic recompute is the verification class), or
 *  - the receipt would carry crawl payload, wallet, or private material.
 *
 * The returned `receiptRef` is content-addressed: it is derived from a
 * SHA-256 over the canonical receipt body, so the same provenance and
 * transform chain always yield the same ref.
 */
export const buildCs336A4ProvenanceReceipt = async (
  input: Readonly<{
    assignmentRef: string
    finalOutputDigestRef: string
    inputShardRef: string
    provenance: Cs336A4SourceProvenance
    sourceInputDigestRef: string
    transformChain: ReadonlyArray<Cs336A4TransformStep>
  }>,
): Promise<Cs336A4ProvenanceReceipt> => {
  const assignmentRef = requireNonEmptyRef('assignmentRef', input.assignmentRef)
  const inputShardRef = requireNonEmptyRef('inputShardRef', input.inputShardRef)
  const sourceInputDigestRef = requireNonEmptyRef(
    'sourceInputDigestRef',
    input.sourceInputDigestRef,
  )
  const finalOutputDigestRef = requireNonEmptyRef(
    'finalOutputDigestRef',
    input.finalOutputDigestRef,
  )

  const provenance: Cs336A4SourceProvenance = {
    acquisitionMode: input.provenance.acquisitionMode,
    licenseRef: requireNonEmptyRef('licenseRef', input.provenance.licenseRef),
    snapshotRef: requireNonEmptyRef('snapshotRef', input.provenance.snapshotRef),
    sourceRef: requireNonEmptyRef('sourceRef', input.provenance.sourceRef),
  }

  if (
    !(Cs336A4AcquisitionModes as ReadonlyArray<string>).includes(
      provenance.acquisitionMode,
    )
  ) {
    throw new Cs336A4ProvenanceValidationError(
      `CS336 A4 provenance receipt names an unknown acquisition mode: ${provenance.acquisitionMode}.`,
    )
  }

  if (input.transformChain.length === 0) {
    throw new Cs336A4ProvenanceValidationError(
      'CS336 A4 provenance receipt requires a non-empty transform chain; a shard with no transform digests has no provenance.',
    )
  }

  let recomputeVerified = true
  let previousOutputDigestRef = sourceInputDigestRef

  const transformChain = input.transformChain.map(
    (step, index): Cs336A4TransformStep => {
      if (!isStage(step.stage)) {
        throw new Cs336A4ProvenanceValidationError(
          `CS336 A4 provenance receipt transform step ${index} names an unknown stage: ${String(step.stage)}.`,
        )
      }

      const stepInputDigestRef = requireNonEmptyRef(
        `transformChain[${index}].inputDigestRef`,
        step.inputDigestRef,
      )
      const stepOutputDigestRef = requireNonEmptyRef(
        `transformChain[${index}].outputDigestRef`,
        step.outputDigestRef,
      )
      const stepRecomputedDigestRef = requireNonEmptyRef(
        `transformChain[${index}].recomputedDigestRef`,
        step.recomputedDigestRef,
      )

      if (stepInputDigestRef !== previousOutputDigestRef) {
        throw new Cs336A4ProvenanceValidationError(
          `CS336 A4 provenance transform chain is not linked at step ${index}: input digest does not equal the prior output digest.`,
        )
      }

      if (stepRecomputedDigestRef !== stepOutputDigestRef) {
        recomputeVerified = false
      }

      previousOutputDigestRef = stepOutputDigestRef

      return {
        codeVersionRef: requireNonEmptyRef(
          `transformChain[${index}].codeVersionRef`,
          step.codeVersionRef,
        ),
        inputDigestRef: stepInputDigestRef,
        outputDigestRef: stepOutputDigestRef,
        recomputedDigestRef: stepRecomputedDigestRef,
        stage: step.stage,
      }
    },
  )

  if (previousOutputDigestRef !== finalOutputDigestRef) {
    throw new Cs336A4ProvenanceValidationError(
      'CS336 A4 provenance receipt final output digest does not equal the last transform step output digest.',
    )
  }

  if (!recomputeVerified) {
    throw new Cs336A4ProvenanceValidationError(
      'CS336 A4 provenance receipt has a transform step whose recomputed digest does not match its committed output; deterministic recompute did not verify.',
    )
  }

  const body = canonicalReceiptBody({
    assignmentRef,
    finalOutputDigestRef,
    inputShardRef,
    provenance,
    recomputeVerified,
    sourceInputDigestRef,
    transformChain,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const receiptRef = `receipt.cs336_a4.provenance.${assignmentRef}.${contentDigestRef.slice(0, 16)}`

  const receipt: Cs336A4ProvenanceReceipt = {
    assignmentRef,
    contentDigestRef,
    finalOutputDigestRef,
    inputShardRef,
    jobKind: Cs336A4DataRefineryJobKind,
    provenance,
    receiptRef,
    recomputeVerified,
    schemaVersion: Cs336A4ProvenanceSchemaVersion,
    sourceInputDigestRef,
    transformChain,
  }

  assertPublicSafe(receipt)

  return receipt
}
