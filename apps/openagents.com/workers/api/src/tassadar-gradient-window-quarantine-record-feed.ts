import { verifyTassadarGradientWindowQuarantineRecord } from './tassadar-gradient-window-quarantine-record-verify'

/**
 * Quarantine-record feed builder for training.public_gradient_windows.v1.
 *
 * The quarantine record builder (tassadar-gradient-window-quarantine-record.ts)
 * BUILDS one durable record, the read-side verifier
 * (tassadar-gradient-window-quarantine-record-verify.ts) validates ONE untrusted
 * read-back record, and the promotion lineage guard
 * (tassadar-gradient-window-promotion-lineage.ts) checks one record against the
 * receipt that descends from it. But a public quarantine route does not serve a
 * single record — it serves a COLLECTION. Nothing turned an untrusted list of
 * read-back quarantine records into the one public-safe, verified, de-duplicated,
 * ordered feed such a route or store-scan would publish.
 *
 * This is the symmetric counterpart to the promoted-window receipt feed builder
 * (tassadar-gradient-window-promotion-receipt-feed.ts): the receipt side of the
 * runtime had a collection-level aggregator, the quarantine side did not.
 *
 * `buildTassadarGradientWindowQuarantineRecordFeed` closes that gap. It is a
 * pure, TOTAL function over an array of untrusted records: it runs each through
 * the read-side verifier, admits only records that pass every invariant, drops
 * duplicates (same canonical record ref) keeping the first, counts and explains
 * every rejection, and returns the accepted entries deterministically ordered by
 * record ref. It never throws, so it is safe at the edge of a real quarantine
 * feed.
 *
 * Every admitted entry is still residency-only: a quarantine record asserts only
 * that the window holds quarantine residency and still owes its full recompute ->
 * replicate -> canary -> promote verification debt, so `promotionEligible` is
 * never surfaced and the feed confers no promotion, settlement,
 * canonical-checkpoint, compiled-core-gradient, or direct-submission authority.
 *
 * This advances blocker.product_promises.public_gradient_live_window_runtime_missing
 * by building the collection-level aggregation layer a public quarantine feed
 * needs. It does NOT clear that blocker: no live store persists these records, no
 * route serves this feed, and no public window has been accepted, promoted, paid,
 * or settled — so a real feed is empty.
 */

export const TassadarGradientWindowQuarantineRecordFeedSchemaVersion =
  'openagents.training.public_gradient_window.quarantine_record_feed.v1'
export type TassadarGradientWindowQuarantineRecordFeedSchemaVersion =
  typeof TassadarGradientWindowQuarantineRecordFeedSchemaVersion

const feedBlocker = (suffix: string): string =>
  `blocker.public.tassadar_gradient_window.quarantine_record_feed.${suffix}`

export type TassadarGradientWindowQuarantineRecordFeedEntry = Readonly<{
  pendingVerificationStages: ReadonlyArray<string>
  recordRef: string
  windowRef: string
}>

export type TassadarGradientWindowQuarantineRecordFeed = Readonly<{
  acceptedEntries: ReadonlyArray<TassadarGradientWindowQuarantineRecordFeedEntry>
  acceptedRecordCount: number
  publicSafe: true
  rejectedRecordCount: number
  rejectionReasonRefs: ReadonlyArray<string>
  schemaVersion: TassadarGradientWindowQuarantineRecordFeedSchemaVersion
}>

/**
 * Aggregate an untrusted list of read-back quarantine records into a public-safe
 * feed.
 *
 * Pure and total: never throws. Each record is verified by the read-side
 * verifier; invalid records and duplicate record refs are dropped and counted,
 * carrying their reasons, and the accepted entries are returned ordered by
 * record ref.
 */
export const buildTassadarGradientWindowQuarantineRecordFeed = (
  records: ReadonlyArray<unknown>,
): TassadarGradientWindowQuarantineRecordFeed => {
  const acceptedByRef = new Map<
    string,
    TassadarGradientWindowQuarantineRecordFeedEntry
  >()
  const rejectionReasons: Array<string> = []
  let rejectedRecordCount = 0

  for (const record of records) {
    const verification = verifyTassadarGradientWindowQuarantineRecord(record)

    if (
      !verification.valid ||
      verification.recordRef === null ||
      verification.windowRef === null
    ) {
      rejectedRecordCount += 1
      rejectionReasons.push(...verification.invalidReasonRefs)
      continue
    }

    if (acceptedByRef.has(verification.recordRef)) {
      rejectedRecordCount += 1
      rejectionReasons.push(feedBlocker('duplicate_record_ref'))
      continue
    }

    acceptedByRef.set(verification.recordRef, {
      pendingVerificationStages: verification.pendingVerificationStages,
      recordRef: verification.recordRef,
      windowRef: verification.windowRef,
    })
  }

  const acceptedEntries = [...acceptedByRef.values()].sort((a, b) =>
    a.recordRef < b.recordRef ? -1 : a.recordRef > b.recordRef ? 1 : 0,
  )

  return {
    acceptedEntries,
    acceptedRecordCount: acceptedEntries.length,
    publicSafe: true,
    rejectedRecordCount,
    rejectionReasonRefs: [...new Set(rejectionReasons)].sort(),
    schemaVersion: TassadarGradientWindowQuarantineRecordFeedSchemaVersion,
  }
}
