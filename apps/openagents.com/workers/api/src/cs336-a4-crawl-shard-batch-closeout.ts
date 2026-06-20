import type { Cs336A4CrawlShardAssignment } from './cs336-a4-crawl-shard-assignment'
import type { Cs336A4CrawlShardDispatchManifest } from './cs336-a4-crawl-shard-dispatch-manifest'
import {
  assertCs336A4CrawlShardProvenanceBinding,
  Cs336A4CrawlShardProvenanceBindingError,
  type Cs336A4CrawlShardProvenanceBindingMismatch,
} from './cs336-a4-crawl-shard-provenance-binding'
import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4ProvenanceReceipt } from './cs336-a4-provenance'

/**
 * Crawl-shard BATCH CLOSEOUT receipt for CS336 A4 refinery acquisition
 * (`blocker.product_promises.corpus_provenance_receipts_missing`).
 *
 * Two deterministic artifacts already exist on either end of a paid
 * crawl-shard batch but nothing composes them, so an operator has no single,
 * auditable record that a dispatched batch was COMPLETELY closed out with
 * bound provenance receipts:
 *
 *  1. `buildCs336A4CrawlShardDispatchManifest`
 *     (cs336-a4-crawl-shard-dispatch-manifest.ts) emits the authoritative,
 *     content-addressed list of `assignmentRefs` an operator dispatched paid
 *     work against — but it says nothing about what came BACK.
 *  2. `assertCs336A4CrawlShardProvenanceBinding`
 *     (cs336-a4-crawl-shard-provenance-binding.ts) proves ONE returned
 *     provenance receipt closes out ONE assignment — but it says nothing
 *     about whether the SET of returned receipts covers the whole dispatched
 *     batch. Every individual receipt can bind correctly while the batch is
 *     still left with an assignment that was never closed out (corpus never
 *     delivered) or carries a receipt for an assignment that was never
 *     dispatched (paying for work outside the batch), or two receipts that
 *     both claim the same assignment (double-counted).
 *
 * This module is the fail-closed composition that closes all of those at
 * once. `buildCs336A4CrawlShardBatchCloseoutReceipt` is the single entry
 * point an admission/closeout path should call after a dispatched batch
 * returns: it asserts the provided authentic assignments are exactly the
 * manifest's dispatched set, asserts EVERY returned receipt binds to its
 * assignment (re-using the per-assignment binding gate verbatim), asserts the
 * receipts uniquely tile the manifest (no unclosed assignment, no receipt for
 * an undispatched assignment, no duplicate), and only then emits a
 * deterministic, content-addressed `Cs336A4CrawlShardBatchCloseoutReceipt` —
 * the auditable record that "this dispatched batch is fully and uniquely
 * closed out with bound corpus provenance".
 *
 * It is a pure composition over already-built artifacts: it re-uses the
 * existing fail-closed binding gate (re-raising its typed reason), and never
 * fetches, fabricates, mutates, or settles anything. It does NOT re-validate
 * any receipt's internal transform chain (that is `buildCs336A4ProvenanceReceipt`'s
 * job), does NOT re-derive assignment refs (that is the dispatch manifest's
 * job), and does NOT settle payment (that is the eval-delta settlement's job);
 * it answers exactly one question: is this dispatched batch fully closed out?
 */

export const Cs336A4CrawlShardBatchCloseoutSchemaVersion =
  'openagents.training.data_refinery.crawl_shard_batch_closeout.v1' as const

/**
 * One closure in a batch closeout: the dispatched assignment ref and the
 * content-addressed provenance receipt that closes it out. Ordered by
 * `assignmentRef` in the receipt so the closeout ref is input-order
 * independent.
 */
export type Cs336A4CrawlShardCloseoutEntry = Readonly<{
  assignmentRef: string
  inputShardRef: string
  provenanceReceiptRef: string
}>

export type Cs336A4CrawlShardBatchCloseoutReceipt = Readonly<{
  /** Number of dispatched assignments closed out (equals manifest.assignmentCount). */
  assignmentCount: number
  /** Content-addressed batch closeout ref derived from contentDigestRef. */
  closeoutRef: string
  /** Ordered (by assignmentRef) closures binding each assignment to its receipt. */
  closures: ReadonlyArray<Cs336A4CrawlShardCloseoutEntry>
  /** SHA-256 over the canonical closeout body (hex). */
  contentDigestRef: string
  jobKind: typeof Cs336A4DataRefineryJobKind
  /** Content-addressed dispatch manifest ref the batch was closed out against. */
  manifestRef: string
  /** Content-addressed plan ref the batch was dispatched from. */
  planRef: string
  schemaVersion: typeof Cs336A4CrawlShardBatchCloseoutSchemaVersion
  snapshotRef: string
}>

export const Cs336A4CrawlShardBatchCloseoutFailures = [
  'empty_manifest',
  'assignment_set_mismatch',
  'duplicate_assignment',
  'provenance_binding_failed',
  'receipt_for_undispatched_assignment',
  'duplicate_receipt',
  'unclosed_assignment',
] as const
export type Cs336A4CrawlShardBatchCloseoutFailure =
  (typeof Cs336A4CrawlShardBatchCloseoutFailures)[number]

/**
 * Thrown when the returned receipt set does not fully and uniquely close out
 * the dispatched batch. `reason` is the typed batch-level failure; when the
 * failure is a per-receipt binding rejection, `bindingReason` carries the
 * underlying provenance-binding mismatch so an audit trail can record the
 * exact cause.
 */
export class Cs336A4CrawlShardBatchCloseoutError extends Error {
  readonly _tag = 'Cs336A4CrawlShardBatchCloseoutError'
  readonly reason: Cs336A4CrawlShardBatchCloseoutFailure
  readonly bindingReason?: Cs336A4CrawlShardProvenanceBindingMismatch

  constructor(
    reason: Cs336A4CrawlShardBatchCloseoutFailure,
    detail: string,
    bindingReason?: Cs336A4CrawlShardProvenanceBindingMismatch,
  ) {
    super(detail)
    this.reason = reason
    if (bindingReason !== undefined) {
      this.bindingReason = bindingReason
    }
  }
}

export class Cs336A4CrawlShardBatchCloseoutUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4CrawlShardBatchCloseoutUnsafeMaterialError'
}

const unsafeCloseoutMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|https?:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet|warc)/i

const assertJsonPublicSafe = (json: string): void => {
  if (unsafeCloseoutMaterialPattern.test(json)) {
    throw new Cs336A4CrawlShardBatchCloseoutUnsafeMaterialError(
      'CS336 A4 crawl shard batch closeout contains crawl payload, URL, wallet, payment, or private material.',
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

/**
 * Builds the canonical closeout body with fields in a fixed order so the
 * content digest is stable regardless of caller key/receipt ordering. The
 * closures are sorted by `assignmentRef` so the same batch (in any input
 * order) yields the same closeout ref.
 */
const canonicalCloseoutBody = (
  input: Readonly<{
    closures: ReadonlyArray<Cs336A4CrawlShardCloseoutEntry>
    manifestRef: string
    planRef: string
    snapshotRef: string
  }>,
): string =>
  JSON.stringify({
    assignmentCount: input.closures.length,
    closures: input.closures,
    jobKind: Cs336A4DataRefineryJobKind,
    manifestRef: input.manifestRef,
    planRef: input.planRef,
    schemaVersion: Cs336A4CrawlShardBatchCloseoutSchemaVersion,
    snapshotRef: input.snapshotRef,
  })

/**
 * Composes the dispatch manifest (the authoritative dispatched set) with the
 * per-assignment provenance binding gate into one deterministic, fail-closed
 * batch closeout receipt.
 *
 * Order of operations:
 *  1. The manifest must be non-empty (`empty_manifest`).
 *  2. The provided authentic assignments must be EXACTLY the manifest's
 *     dispatched set — same refs, no extras, no missing, no duplicates
 *     (`assignment_set_mismatch`, `duplicate_assignment`). The assignment
 *     objects are required because the binding gate compares against their
 *     `inputShardRef` + source provenance.
 *  3. EVERY returned receipt must bind to its assignment via
 *     `assertCs336A4CrawlShardProvenanceBinding`; a receipt for an assignment
 *     outside the batch is rejected (`receipt_for_undispatched_assignment`),
 *     a binding rejection re-raises the underlying reason
 *     (`provenance_binding_failed`), and two receipts for the same assignment
 *     are rejected (`duplicate_receipt`).
 *  4. Every dispatched assignment must be closed out by exactly one receipt;
 *     any uncovered assignment hard-fails (`unclosed_assignment`).
 *  5. Emit a content-addressed closeout receipt binding the manifest to the
 *     ordered (assignmentRef -> provenance receipt) closures. The
 *     `closeoutRef` is content-addressed via SHA-256 over a canonical body, so
 *     the same manifest + receipt set (in any order) always yields the same
 *     ref.
 *
 * Emits refs only; the public-safety guard fails closed before any unsafe
 * material is committed.
 */
export const buildCs336A4CrawlShardBatchCloseoutReceipt = async (
  input: Readonly<{
    assignments: ReadonlyArray<Cs336A4CrawlShardAssignment>
    manifest: Cs336A4CrawlShardDispatchManifest
    receipts: ReadonlyArray<Cs336A4ProvenanceReceipt>
  }>,
): Promise<Cs336A4CrawlShardBatchCloseoutReceipt> => {
  const { assignments, manifest, receipts } = input

  if (manifest.assignmentRefs.length === 0) {
    throw new Cs336A4CrawlShardBatchCloseoutError(
      'empty_manifest',
      'CS336 A4 crawl-shard batch closeout requires a non-empty dispatch manifest.',
    )
  }

  // Stage 1: the provided authentic assignments must be exactly the manifest's
  // dispatched set, so the closeout cannot bind receipts against assignment
  // objects the operator never dispatched.
  const dispatched = new Set(manifest.assignmentRefs)
  const assignmentByRef = new Map<string, Cs336A4CrawlShardAssignment>()
  for (const assignment of assignments) {
    if (assignmentByRef.has(assignment.assignmentRef)) {
      throw new Cs336A4CrawlShardBatchCloseoutError(
        'duplicate_assignment',
        `CS336 A4 crawl-shard batch closeout received a duplicate assignment (${assignment.assignmentRef}).`,
      )
    }
    if (!dispatched.has(assignment.assignmentRef)) {
      throw new Cs336A4CrawlShardBatchCloseoutError(
        'assignment_set_mismatch',
        `CS336 A4 crawl-shard batch closeout received an assignment not in the manifest (${assignment.assignmentRef}).`,
      )
    }
    assignmentByRef.set(assignment.assignmentRef, assignment)
  }
  if (assignmentByRef.size !== dispatched.size) {
    throw new Cs336A4CrawlShardBatchCloseoutError(
      'assignment_set_mismatch',
      `CS336 A4 crawl-shard batch closeout assignment set (${assignmentByRef.size}) does not match the manifest (${dispatched.size}).`,
    )
  }

  // Stage 2: every returned receipt must bind to its assignment, and each
  // assignment may be closed out only once.
  const closures: Cs336A4CrawlShardCloseoutEntry[] = []
  const closed = new Set<string>()
  for (const receipt of receipts) {
    const assignment = assignmentByRef.get(receipt.assignmentRef)
    if (!assignment) {
      throw new Cs336A4CrawlShardBatchCloseoutError(
        'receipt_for_undispatched_assignment',
        `CS336 A4 crawl-shard batch closeout received a provenance receipt for an assignment not in the batch (${receipt.assignmentRef}).`,
      )
    }
    if (closed.has(receipt.assignmentRef)) {
      throw new Cs336A4CrawlShardBatchCloseoutError(
        'duplicate_receipt',
        `CS336 A4 crawl-shard batch closeout received two provenance receipts for the same assignment (${receipt.assignmentRef}).`,
      )
    }

    let provenanceReceiptRef: string
    try {
      provenanceReceiptRef = assertCs336A4CrawlShardProvenanceBinding({
        assignment,
        receipt,
      })
    } catch (error) {
      if (error instanceof Cs336A4CrawlShardProvenanceBindingError) {
        throw new Cs336A4CrawlShardBatchCloseoutError(
          'provenance_binding_failed',
          `CS336 A4 crawl-shard batch closeout rejected an unbound provenance receipt: ${error.message}`,
          error.reason,
        )
      }
      throw error
    }

    closed.add(receipt.assignmentRef)
    closures.push({
      assignmentRef: receipt.assignmentRef,
      inputShardRef: assignment.inputShardRef,
      provenanceReceiptRef,
    })
  }

  // Stage 3: every dispatched assignment must be closed out.
  for (const assignmentRef of manifest.assignmentRefs) {
    if (!closed.has(assignmentRef)) {
      throw new Cs336A4CrawlShardBatchCloseoutError(
        'unclosed_assignment',
        `CS336 A4 crawl-shard batch closeout left a dispatched assignment with no provenance receipt (${assignmentRef}).`,
      )
    }
  }

  // Order closures canonically by assignmentRef so the closeout ref does not
  // depend on caller receipt ordering.
  const ordered = [...closures].sort((left, right) =>
    left.assignmentRef < right.assignmentRef
      ? -1
      : left.assignmentRef > right.assignmentRef
        ? 1
        : 0,
  )

  const body = canonicalCloseoutBody({
    closures: ordered,
    manifestRef: manifest.manifestRef,
    planRef: manifest.planRef,
    snapshotRef: manifest.snapshotRef,
  })

  assertJsonPublicSafe(body)

  const contentDigestRef = await sha256Hex(body)
  const closeoutRef = `closeout.cs336_a4.crawl_shard_batch.${manifest.snapshotRef}.${contentDigestRef.slice(0, 16)}`

  return {
    assignmentCount: ordered.length,
    closeoutRef,
    closures: ordered,
    contentDigestRef,
    jobKind: Cs336A4DataRefineryJobKind,
    manifestRef: manifest.manifestRef,
    planRef: manifest.planRef,
    schemaVersion: Cs336A4CrawlShardBatchCloseoutSchemaVersion,
    snapshotRef: manifest.snapshotRef,
  }
}
