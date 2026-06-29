import type { Cs336A4CrawlShardBatchCloseoutReceipt } from './cs336-a4-crawl-shard-batch-closeout'
import type { Cs336A4CrawlShardDispatchManifest } from './cs336-a4-crawl-shard-dispatch-manifest'
import { Cs336A4DataRefineryJobKind } from './cs336-a4-data-refinery'
import type { Cs336A4EvalDeltaSettlementCloseout } from './cs336-a4-eval-delta-settlement-closeout'

/**
 * Paid CS336 A4 refinery shard dispatch receipt.
 *
 * The lower-level crawl-shard modules prove that a batch of assignments is
 * authentic/complete and that the returned provenance receipts uniquely close
 * it out. The eval-delta closeout proves a fixed-reference-model quality
 * payment can be computed against a recompute-verified, decontaminated shard.
 *
 * This module binds those artifacts into the public-safe receipt an operator
 * can use as the paid-dispatch evidence record: every dispatched assignment is
 * priced at the base verified-shard rate, the batch closeout must match the
 * dispatch manifest exactly, and at least one assignment must carry a payable
 * eval-delta settlement receipt. It computes sats only; it never emits wallet,
 * invoice, preimage, raw crawl payload, or private material.
 */

export const Cs336A4PaidRefineryShardDispatchSchemaVersion =
  'openagents.training.data_refinery.paid_refinery_shard_dispatch.v1' as const

export type Cs336A4PaidRefineryShardDispatchReceipt = Readonly<{
  assignmentCount: number
  basePayoutSats: number
  baseRateSatsPerVerifiedShard: number
  batchCloseoutRef: string
  contentDigestRef: string
  dispatchManifestRef: string
  evalDeltaSettlementReceiptRefs: ReadonlyArray<string>
  evalDeltaSettledBonusSats: number
  jobKind: typeof Cs336A4DataRefineryJobKind
  paidAssignmentRefs: ReadonlyArray<string>
  planRef: string
  provenanceReceiptRefs: ReadonlyArray<string>
  receiptRef: string
  schemaVersion: typeof Cs336A4PaidRefineryShardDispatchSchemaVersion
  snapshotRef: string
  totalComputedPayoutSats: number
  verificationRefs: ReadonlyArray<string>
}>

export class Cs336A4PaidRefineryShardDispatchError extends Error {
  readonly _tag = 'Cs336A4PaidRefineryShardDispatchError'
}

export class Cs336A4PaidRefineryShardDispatchUnsafeMaterialError extends Error {
  readonly _tag = 'Cs336A4PaidRefineryShardDispatchUnsafeMaterialError'
}

const unsafeMaterialPattern =
  /(\/Users\/|\/home\/|api[_-]?key|bearer|bolt11|bolt12|https?:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|payment[_-]?(hash|preimage)|preimage|private|raw[_-]?(crawl|dataset|invoice|payment|payload|prompt|runner|shard|warc)|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet|warc)/i

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertPublicSafeRefs = (refs: ReadonlyArray<string>): void => {
  for (const ref of refs) {
    if (unsafeMaterialPattern.test(ref)) {
      throw new Cs336A4PaidRefineryShardDispatchUnsafeMaterialError(
        'CS336 A4 paid refinery dispatch receipt contains wallet, payment, raw payload, URL, or private material.',
      )
    }
  }
}

const sameRefSet = (
  left: ReadonlyArray<string>,
  right: ReadonlyArray<string>,
): boolean => {
  const normalizedLeft = uniqueRefs(left)
  const normalizedRight = uniqueRefs(right)

  return (
    normalizedLeft.length === normalizedRight.length &&
    normalizedLeft.every((ref, index) => ref === normalizedRight[index])
  )
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

const canonicalReceiptBody = (
  input: Omit<Cs336A4PaidRefineryShardDispatchReceipt, 'contentDigestRef' | 'receiptRef'>,
): string =>
  JSON.stringify({
    assignmentCount: input.assignmentCount,
    basePayoutSats: input.basePayoutSats,
    baseRateSatsPerVerifiedShard: input.baseRateSatsPerVerifiedShard,
    batchCloseoutRef: input.batchCloseoutRef,
    dispatchManifestRef: input.dispatchManifestRef,
    evalDeltaSettlementReceiptRefs: input.evalDeltaSettlementReceiptRefs,
    evalDeltaSettledBonusSats: input.evalDeltaSettledBonusSats,
    jobKind: input.jobKind,
    paidAssignmentRefs: input.paidAssignmentRefs,
    planRef: input.planRef,
    provenanceReceiptRefs: input.provenanceReceiptRefs,
    schemaVersion: input.schemaVersion,
    snapshotRef: input.snapshotRef,
    totalComputedPayoutSats: input.totalComputedPayoutSats,
    verificationRefs: input.verificationRefs,
  })

/**
 * Builds a deterministic paid-dispatch receipt for a fully closed out A4
 * refinery shard batch. Fails closed unless:
 *
 *  - the closeout receipt binds to the same manifest/plan/snapshot;
 *  - closeout closures cover exactly the manifest's assignment refs;
 *  - the base rate is a positive integer sats amount;
 *  - at least one fixed-reference-model eval-delta closeout is payable; and
 *  - every payable eval-delta receipt names an assignment in the dispatched set.
 */
export const buildCs336A4PaidRefineryShardDispatchReceipt = async (
  input: Readonly<{
    baseRateSatsPerVerifiedShard: number
    closeout: Cs336A4CrawlShardBatchCloseoutReceipt
    evalDeltaCloseouts: ReadonlyArray<Cs336A4EvalDeltaSettlementCloseout>
    manifest: Cs336A4CrawlShardDispatchManifest
    verificationRefs: ReadonlyArray<string>
  }>,
): Promise<Cs336A4PaidRefineryShardDispatchReceipt> => {
  const {
    baseRateSatsPerVerifiedShard,
    closeout,
    evalDeltaCloseouts,
    manifest,
    verificationRefs,
  } = input

  if (
    !Number.isInteger(baseRateSatsPerVerifiedShard) ||
    baseRateSatsPerVerifiedShard <= 0
  ) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires a positive integer baseRateSatsPerVerifiedShard.',
    )
  }

  if (manifest.assignmentRefs.length === 0) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires a non-empty dispatch manifest.',
    )
  }

  if (
    closeout.manifestRef !== manifest.manifestRef ||
    closeout.planRef !== manifest.planRef ||
    closeout.snapshotRef !== manifest.snapshotRef
  ) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires the batch closeout to match the dispatch manifest.',
    )
  }

  const closedAssignmentRefs = closeout.closures.map(
    closure => closure.assignmentRef,
  )
  if (!sameRefSet(closedAssignmentRefs, manifest.assignmentRefs)) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires closeout closures to cover exactly the dispatched assignment refs.',
    )
  }

  const paidAssignmentRefs = uniqueRefs(manifest.assignmentRefs)
  const dispatched = new Set(paidAssignmentRefs)
  const payableEvalCloseouts = evalDeltaCloseouts.filter(
    closeout => closeout.settlementReceipt.payable,
  )

  if (payableEvalCloseouts.length === 0) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires at least one payable eval-delta settlement closeout.',
    )
  }

  for (const evalCloseout of payableEvalCloseouts) {
    if (!dispatched.has(evalCloseout.settlementReceipt.assignmentRef)) {
      throw new Cs336A4PaidRefineryShardDispatchError(
        'CS336 A4 paid refinery dispatch received an eval-delta settlement for an assignment outside the dispatched batch.',
      )
    }
  }

  const normalizedVerificationRefs = uniqueRefs(verificationRefs)
  if (normalizedVerificationRefs.length === 0) {
    throw new Cs336A4PaidRefineryShardDispatchError(
      'CS336 A4 paid refinery dispatch requires deterministic-recompute verification refs.',
    )
  }

  const provenanceReceiptRefs = uniqueRefs(
    closeout.closures.map(closure => closure.provenanceReceiptRef),
  )
  const evalDeltaSettlementReceiptRefs = uniqueRefs(
    payableEvalCloseouts.map(
      closeout => closeout.settlementReceipt.receiptRef,
    ),
  )
  const evalDeltaSettledBonusSats = payableEvalCloseouts.reduce(
    (total, closeout) => total + closeout.settlementReceipt.settledBonusSats,
    0,
  )
  const basePayoutSats =
    paidAssignmentRefs.length * baseRateSatsPerVerifiedShard

  const bodyWithoutDigest = {
    assignmentCount: paidAssignmentRefs.length,
    basePayoutSats,
    baseRateSatsPerVerifiedShard,
    batchCloseoutRef: closeout.closeoutRef,
    dispatchManifestRef: manifest.manifestRef,
    evalDeltaSettlementReceiptRefs,
    evalDeltaSettledBonusSats,
    jobKind: Cs336A4DataRefineryJobKind,
    paidAssignmentRefs,
    planRef: manifest.planRef,
    provenanceReceiptRefs,
    schemaVersion: Cs336A4PaidRefineryShardDispatchSchemaVersion,
    snapshotRef: manifest.snapshotRef,
    totalComputedPayoutSats: basePayoutSats + evalDeltaSettledBonusSats,
    verificationRefs: normalizedVerificationRefs,
  } satisfies Omit<
    Cs336A4PaidRefineryShardDispatchReceipt,
    'contentDigestRef' | 'receiptRef'
  >

  assertPublicSafeRefs([
    bodyWithoutDigest.batchCloseoutRef,
    bodyWithoutDigest.dispatchManifestRef,
    ...bodyWithoutDigest.evalDeltaSettlementReceiptRefs,
    ...bodyWithoutDigest.paidAssignmentRefs,
    bodyWithoutDigest.planRef,
    ...bodyWithoutDigest.provenanceReceiptRefs,
    bodyWithoutDigest.snapshotRef,
    ...bodyWithoutDigest.verificationRefs,
  ])

  const contentDigestRef = await sha256Hex(
    canonicalReceiptBody(bodyWithoutDigest),
  )

  return {
    ...bodyWithoutDigest,
    contentDigestRef,
    receiptRef: `receipt.cs336_a4.paid_refinery_dispatch.${manifest.snapshotRef}.${contentDigestRef.slice(0, 16)}`,
  }
}
