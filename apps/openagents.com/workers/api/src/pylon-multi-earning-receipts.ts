// Pylon multi-earning per-mode WORK RECEIPT surface — the public-safe receipt
// evidence that sits BEHIND the multi-earning projection's per-mode amount
// counts (EPIC #5523 / DE-4 #5527; promise pylon.v0_3_multi_earning_node.v1, red).
//
// THE GAP THIS ADVANCES: blocker `multi_earning_mode_receipts_missing`. The
// projection (pylon-multi-earning-node.ts) distinguishes modeled / observed /
// pending / paid / settled COUNTS per earning mode, but it took those counts as
// hand-fed integers — there was no defined per-unit work-receipt shape behind
// them, and no way to derive the projection's earning store from real receipts.
// This module supplies that missing piece: a public-safe per-mode work-receipt
// record, a validating builder, an idempotent receipt store, and a PURE fold
// that turns a set of work receipts into the projection's earning store. That
// is the structural part of "per-mode receipts": every count the projection
// reports can now point back to a dereferenceable per-mode receipt.
//
// HONESTY / SCOPE: PURE and INERT. It mints no money, reads no wallet, moves no
// funds, and admits no live settled receipt. It defines the EVIDENCE SHAPE so
// that, once real settled receipts exist across >=2 modes in one install, they
// can be folded into the safe projection. The blocker STAYS listed: no live
// settled per-mode receipts exist yet, and a green flip is receipt-first and
// owner-signed per proof.claim_upgrade_receipts.v1. The promise
// pylon.v0_3_multi_earning_node.v1 STAYS red.
//
// PUBLIC-SAFE BY CONSTRUCTION: every ref is validated by the SAME bounded,
// neutral token discipline as the projection (isPublicSafeToken): no money
// units, wallet, payment, payout, customer, secret, bolt11, preimage, or raw
// timestamp material.

import { Schema as S } from 'effect'

import {
  PYLON_MULTI_EARNING_PROMISE,
  PylonMultiEarningError,
  type PylonMultiEarningStore,
  isPublicSafeToken,
  makeInMemoryPylonMultiEarningStore,
  recordModeEarning,
} from './pylon-multi-earning-node'

export const PYLON_MULTI_EARNING_RECEIPT_SCHEMA =
  'openagents.pylon_multi_earning_work_receipt.v1' as const

// The amount classes a single work receipt may attest. A receipt records that a
// work EVENT happened, so it starts at `observed`; `modeled` is an estimate with
// no underlying work event and therefore has no receipt. Order matches the
// money-state order of the projection's amount classes.
export const RECEIPTABLE_AMOUNT_CLASSES = [
  'observed',
  'pending',
  'paid',
  'settled',
] as const
export type ReceiptableAmountClass = (typeof RECEIPTABLE_AMOUNT_CLASSES)[number]

const isReceiptableAmountClass = (
  value: string,
): value is ReceiptableAmountClass =>
  (RECEIPTABLE_AMOUNT_CLASSES as ReadonlyArray<string>).includes(value)

/**
 * One per-mode WORK RECEIPT for a single Pylon install. INERT: it attests that
 * one unit of work in one earning mode reached one amount class, and carries the
 * public-safe assignment ref it came from plus its own dereferenceable receipt
 * ref. Carries NO money amount, no wallet, no payment, no payout, no
 * customer/provider material. A settled receipt additionally carries a
 * public-safe settlement-receipt ref.
 */
export const PylonModeWorkReceipt = S.Struct({
  schema: S.Literal(PYLON_MULTI_EARNING_RECEIPT_SCHEMA),
  /** Neutral earning-mode label (e.g. "training", "forum_tips", "compute"). */
  mode: S.String,
  /** The amount class this single receipt attests. */
  amountClass: S.Literals(['observed', 'pending', 'paid', 'settled']),
  /** Public-safe ref to the assignment / work unit this receipt came from. */
  assignmentRef: S.String,
  /** Public-safe, dereferenceable ref that uniquely identifies this receipt. */
  receiptRef: S.String,
  /** Public-safe settlement-receipt ref; present iff amountClass is "settled". */
  settlementReceiptRef: S.optional(S.String),
})
export type PylonModeWorkReceipt = typeof PylonModeWorkReceipt.Type

const assertSafe = (label: string, value: string): string => {
  if (!isPublicSafeToken(value)) {
    throw new PylonMultiEarningError({
      reason: `${label} must be a bounded, public-safe token (no money, wallet, payment, payout, customer, secret, bolt11, preimage, or timestamp material)`,
    })
  }
  return value.trim()
}

/**
 * Build one per-mode work receipt from neutral inputs. PURE / validating. A
 * receipt may only carry a settlement-receipt ref when it attests `settled`, and
 * a `settled` receipt MUST carry one — so a settled unit can never enter the
 * projection without a dereferenceable settlement receipt behind it.
 */
export const recordModeWorkReceipt = (input: {
  mode: string
  amountClass: ReceiptableAmountClass
  assignmentRef: string
  receiptRef: string
  settlementReceiptRef?: string
}):
  | { ok: true; receipt: PylonModeWorkReceipt }
  | { ok: false; error: PylonMultiEarningError } => {
  try {
    if (!isReceiptableAmountClass(input.amountClass)) {
      throw new PylonMultiEarningError({
        reason: `amountClass must be one of: ${RECEIPTABLE_AMOUNT_CLASSES.join(', ')}`,
      })
    }
    const mode = assertSafe('mode', input.mode)
    const assignmentRef = assertSafe('assignmentRef', input.assignmentRef)
    const receiptRef = assertSafe('receiptRef', input.receiptRef)
    const isSettled = input.amountClass === 'settled'
    const settlementReceiptRef =
      input.settlementReceiptRef === undefined
        ? undefined
        : assertSafe('settlementReceiptRef', input.settlementReceiptRef)

    if (isSettled && settlementReceiptRef === undefined) {
      throw new PylonMultiEarningError({
        reason:
          'a settled work receipt requires a public-safe settlementReceiptRef',
      })
    }
    if (!isSettled && settlementReceiptRef !== undefined) {
      throw new PylonMultiEarningError({
        reason:
          'a settlementReceiptRef is only valid on a settled work receipt',
      })
    }

    return {
      ok: true,
      receipt: {
        schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
        mode,
        amountClass: input.amountClass,
        assignmentRef,
        receiptRef,
        ...(settlementReceiptRef === undefined ? {} : { settlementReceiptRef }),
      },
    }
  } catch (error) {
    if (error instanceof PylonMultiEarningError) {
      return { ok: false, error }
    }
    throw error
  }
}

/**
 * An idempotent in-memory store of per-mode work receipts. Recording the same
 * receiptRef twice collapses to the first occurrence, so each receipt counts
 * once in the projection. Insertion order of first-seen receipts is preserved.
 */
export type PylonModeWorkReceiptStore = {
  list: () => ReadonlyArray<PylonModeWorkReceipt>
}

export const makeInMemoryPylonModeWorkReceiptStore = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): PylonModeWorkReceiptStore => {
  const byRef = new Map<string, PylonModeWorkReceipt>()
  for (const receipt of receipts) {
    if (!byRef.has(receipt.receiptRef)) {
      byRef.set(receipt.receiptRef, receipt)
    }
  }
  const deduped = [...byRef.values()]
  return { list: () => deduped }
}

/**
 * Per-mode settlement-coverage report for a set of work receipts. INERT. For one
 * mode it states how many settled receipts the mode carries and how many
 * DISTINCT settlement-receipt refs back them. Coverage is `complete` only when
 * those two numbers are equal — i.e. every settled unit in the mode points at
 * its own dereferenceable settlement receipt, never sharing one.
 */
export type ModeSettlementCoverage = {
  mode: string
  settledReceiptCount: number
  distinctSettlementRefCount: number
  settlementCoverageComplete: boolean
}

/**
 * Whole-install settlement-coverage report. INERT and public-safe: it carries
 * only neutral mode labels and integer counts. `crossModeSettlementReuse` is
 * true when one settlement-receipt ref is claimed by settled receipts in more
 * than one mode (a cross-mode over-claim). `allModesSettlementCovered` is the
 * single honest gate the fold trusts: every mode is per-mode complete AND no
 * settlement ref is reused across modes.
 */
export type WorkReceiptSettlementCoverage = {
  schema: typeof PYLON_MULTI_EARNING_RECEIPT_SCHEMA
  perMode: ReadonlyArray<ModeSettlementCoverage>
  totalSettledReceiptCount: number
  totalDistinctSettlementRefCount: number
  crossModeSettlementReuse: boolean
  allModesSettlementCovered: boolean
}

/**
 * Verify that a set of work receipts does not over-claim settlement. PURE /
 * INERT. Receipts are first deduped by receiptRef (so a receipt counts once),
 * then settled receipts are audited: within each mode the number of settled
 * receipts must equal the number of distinct settlement-receipt refs, and no
 * settlement-receipt ref may be shared across modes. This is the integrity
 * check that lets the projection's per-mode `settledCount` be TRUSTED as backed
 * by that many distinct, dereferenceable settlements — without it, two settled
 * receipts could silently share one settlement and inflate the count.
 */
export const verifyWorkReceiptSettlementCoverage = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): WorkReceiptSettlementCoverage => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()
  const settled = deduped.filter(r => r.amountClass === 'settled')

  // Preserve first-seen mode order for a stable report.
  const order: string[] = []
  const refsByMode = new Map<string, string[]>()
  for (const receipt of settled) {
    // A settled receipt always carries a settlementReceiptRef (enforced by
    // recordModeWorkReceipt); guard defensively for hand-built inputs.
    const ref = receipt.settlementReceiptRef
    if (ref === undefined) {
      continue
    }
    const existing = refsByMode.get(receipt.mode)
    if (existing === undefined) {
      order.push(receipt.mode)
      refsByMode.set(receipt.mode, [ref])
    } else {
      existing.push(ref)
    }
  }

  const perMode: ModeSettlementCoverage[] = []
  const refToModes = new Map<string, Set<string>>()
  for (const mode of order) {
    const refs = refsByMode.get(mode) ?? []
    const distinct = new Set(refs)
    perMode.push({
      mode,
      settledReceiptCount: refs.length,
      distinctSettlementRefCount: distinct.size,
      settlementCoverageComplete: refs.length === distinct.size,
    })
    for (const ref of distinct) {
      const modes = refToModes.get(ref)
      if (modes === undefined) {
        refToModes.set(ref, new Set([mode]))
      } else {
        modes.add(mode)
      }
    }
  }

  const totalSettledReceiptCount = settled.length
  const totalDistinctSettlementRefCount = refToModes.size
  const crossModeSettlementReuse = [...refToModes.values()].some(
    modes => modes.size > 1,
  )
  const allModesSettlementCovered =
    perMode.every(m => m.settlementCoverageComplete) && !crossModeSettlementReuse

  return {
    schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
    perMode,
    totalSettledReceiptCount,
    totalDistinctSettlementRefCount,
    crossModeSettlementReuse,
    allModesSettlementCovered,
  }
}

/**
 * One per-mode entry of the settlement MANIFEST. INERT and public-safe. Unlike
 * the coverage report (which carries only integer COUNTS), this carries the
 * actual DISTINCT settlement-receipt refs that back a mode's settled units, in
 * first-seen order — so each settled unit the projection counts is individually
 * dereferenceable, not collapsed to a single representative ref.
 */
export type ModeSettlementManifestEntry = {
  mode: string
  settledReceiptCount: number
  settlementReceiptRefs: ReadonlyArray<string>
}

/**
 * Whole-install settlement MANIFEST. INERT, public-safe, still red. It is the
 * EVIDENCE companion to verifyWorkReceiptSettlementCoverage: the auditor states
 * THAT every settled unit is backed by its own distinct settlement; this
 * manifest enumerates the actual distinct refs so an owner can dereference each
 * one when verifying a (receipt-first, owner-signed) green flip. The projection
 * record keeps only one representative ref per mode; this fills that gap.
 *
 * `coverageComplete` mirrors the auditor's `allModesSettlementCovered`, so a
 * consumer reading the manifest knows whether per-mode `settledReceiptCount`
 * equals the number of distinct refs listed (no over-claim, no cross-mode reuse).
 */
export type PylonSettlementManifest = {
  schema: typeof PYLON_MULTI_EARNING_RECEIPT_SCHEMA
  promiseId: typeof PYLON_MULTI_EARNING_PROMISE
  promiseState: 'red'
  inert: true
  perMode: ReadonlyArray<ModeSettlementManifestEntry>
  totalSettledReceiptCount: number
  totalDistinctSettlementRefCount: number
  coverageComplete: boolean
}

/**
 * Build the public-safe settlement manifest for a set of work receipts. PURE /
 * INERT and always `promiseState: 'red'`. Receipts are first deduped by
 * receiptRef, then settled receipts are grouped by mode and their
 * settlement-receipt refs collected DISTINCT in first-seen order. The totals and
 * `coverageComplete` are taken from verifyWorkReceiptSettlementCoverage, so the
 * manifest and the auditor can never disagree about whether settlement is fully
 * covered. Surfacing this list is the per-mode receipt evidence the projection
 * needs to be fully dereferenceable.
 */
export const projectPylonSettlementManifest = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): PylonSettlementManifest => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()
  const settled = deduped.filter(r => r.amountClass === 'settled')

  // Preserve first-seen mode order for a stable manifest.
  const order: string[] = []
  const refsByMode = new Map<string, string[]>()
  for (const receipt of settled) {
    // A settled receipt always carries a settlementReceiptRef (enforced by
    // recordModeWorkReceipt); guard defensively for hand-built inputs.
    const ref = receipt.settlementReceiptRef
    if (ref === undefined) {
      continue
    }
    const existing = refsByMode.get(receipt.mode)
    if (existing === undefined) {
      order.push(receipt.mode)
      refsByMode.set(receipt.mode, [ref])
    } else {
      existing.push(ref)
    }
  }

  const perMode: ModeSettlementManifestEntry[] = []
  for (const mode of order) {
    const refs = refsByMode.get(mode) ?? []
    const seen = new Set<string>()
    const distinct: string[] = []
    for (const ref of refs) {
      if (!seen.has(ref)) {
        seen.add(ref)
        distinct.push(ref)
      }
    }
    perMode.push({
      mode,
      settledReceiptCount: refs.length,
      settlementReceiptRefs: distinct,
    })
  }

  // Single source of truth for totals + the coverage gate.
  const coverage = verifyWorkReceiptSettlementCoverage(deduped)

  return {
    schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
    promiseId: PYLON_MULTI_EARNING_PROMISE,
    promiseState: 'red',
    inert: true,
    perMode,
    totalSettledReceiptCount: coverage.totalSettledReceiptCount,
    totalDistinctSettlementRefCount: coverage.totalDistinctSettlementRefCount,
    coverageComplete: coverage.allModesSettlementCovered,
  }
}

/**
 * Per-mode work-unit-coverage report for a set of work receipts. INERT. For one
 * mode it states how many (deduped) receipts the mode carries and how many
 * DISTINCT work units (`assignmentRef`s) back them. Coverage is `complete` only
 * when those two numbers are equal — i.e. every receipt in the mode attests its
 * own distinct work unit, never re-counting one unit across two receipts.
 */
export type ModeWorkUnitCoverage = {
  mode: string
  receiptCount: number
  distinctAssignmentRefCount: number
  workUnitCoverageComplete: boolean
}

/**
 * Whole-install work-unit-coverage report. INERT and public-safe: it carries
 * only neutral mode labels and integer counts. `crossModeWorkUnitReuse` is true
 * when one `assignmentRef` is claimed by receipts in more than one mode (a
 * cross-mode over-claim that could inflate the >=2-modes bar with a single work
 * unit). `allWorkUnitsDistinct` is the honest gate the fold trusts: every mode is
 * per-mode complete AND no work unit is reused across modes.
 */
export type WorkReceiptWorkUnitCoverage = {
  schema: typeof PYLON_MULTI_EARNING_RECEIPT_SCHEMA
  perMode: ReadonlyArray<ModeWorkUnitCoverage>
  totalReceiptCount: number
  totalDistinctAssignmentRefCount: number
  crossModeWorkUnitReuse: boolean
  allWorkUnitsDistinct: boolean
}

/**
 * Verify that a set of work receipts does not over-claim WORK UNITS. PURE /
 * INERT. Receipts are first deduped by receiptRef (so a receipt counts once),
 * then grouped by mode: within each mode the number of receipts must equal the
 * number of distinct `assignmentRef`s, and no `assignmentRef` may be shared
 * across modes. This is the integrity check that lets the projection's per-mode
 * `observed/pending/paid/settled` counts be TRUSTED as one-per-work-unit. It is
 * the work-unit analogue of `verifyWorkReceiptSettlementCoverage`: that auditor
 * stops two settled units sharing one settlement receipt; this one stops two
 * receipts sharing one work unit and double-counting it into the amount classes.
 */
export const verifyWorkReceiptWorkUnitCoverage = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): WorkReceiptWorkUnitCoverage => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()

  // Preserve first-seen mode order for a stable report.
  const order: string[] = []
  const refsByMode = new Map<string, string[]>()
  for (const receipt of deduped) {
    const existing = refsByMode.get(receipt.mode)
    if (existing === undefined) {
      order.push(receipt.mode)
      refsByMode.set(receipt.mode, [receipt.assignmentRef])
    } else {
      existing.push(receipt.assignmentRef)
    }
  }

  const perMode: ModeWorkUnitCoverage[] = []
  const refToModes = new Map<string, Set<string>>()
  for (const mode of order) {
    const refs = refsByMode.get(mode) ?? []
    const distinct = new Set(refs)
    perMode.push({
      mode,
      receiptCount: refs.length,
      distinctAssignmentRefCount: distinct.size,
      workUnitCoverageComplete: refs.length === distinct.size,
    })
    for (const ref of distinct) {
      const modes = refToModes.get(ref)
      if (modes === undefined) {
        refToModes.set(ref, new Set([mode]))
      } else {
        modes.add(mode)
      }
    }
  }

  const totalReceiptCount = deduped.length
  const totalDistinctAssignmentRefCount = refToModes.size
  const crossModeWorkUnitReuse = [...refToModes.values()].some(
    modes => modes.size > 1,
  )
  const allWorkUnitsDistinct =
    perMode.every(m => m.workUnitCoverageComplete) && !crossModeWorkUnitReuse

  return {
    schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
    perMode,
    totalReceiptCount,
    totalDistinctAssignmentRefCount,
    crossModeWorkUnitReuse,
    allWorkUnitsDistinct,
  }
}

/**
 * One per-mode entry of the WORK-RECEIPT manifest. INERT and public-safe. Unlike
 * the projection record (which collapses a mode to integer amount-class COUNTS),
 * this enumerates the actual DISTINCT work-receipt refs backing each amount class
 * in first-seen order — so every unit the projection counts in
 * `observed/pending/paid/settled` is individually dereferenceable, not just a
 * number. Carries the same per-mode work-unit coverage verdict as the auditor.
 *
 * NOTE: `settledReceiptRefs` lists the WORK-receipt refs (`receiptRef`) of a
 * mode's settled units; the dereferenceable SETTLEMENT refs live in the separate
 * settlement manifest (`projectPylonSettlementManifest`). The two manifests cover
 * the two distinct axes — work-unit identity vs settlement identity.
 */
export type ModeWorkReceiptManifestEntry = {
  mode: string
  receiptCount: number
  distinctAssignmentRefCount: number
  workUnitCoverageComplete: boolean
  observedReceiptRefs: ReadonlyArray<string>
  pendingReceiptRefs: ReadonlyArray<string>
  paidReceiptRefs: ReadonlyArray<string>
  settledReceiptRefs: ReadonlyArray<string>
}

/**
 * Whole-install WORK-RECEIPT manifest. INERT, public-safe, still red. It is the
 * work-unit-axis companion to projectPylonSettlementManifest: the settlement
 * manifest enumerates the distinct SETTLEMENT refs behind each settled count;
 * this manifest enumerates the distinct WORK-receipt refs behind every
 * amount-class count, so an owner verifying a (receipt-first, owner-signed) green
 * flip can dereference each individual work unit the projection reports — not
 * only the settled ones, and not collapsed to a count.
 *
 * `coverageComplete` mirrors verifyWorkReceiptWorkUnitCoverage's
 * `allWorkUnitsDistinct`, so a consumer reading the manifest knows whether each
 * mode's `receiptCount` equals the number of distinct work units listed (no
 * in-mode or cross-mode work-unit over-claim).
 */
export type PylonWorkReceiptManifest = {
  schema: typeof PYLON_MULTI_EARNING_RECEIPT_SCHEMA
  promiseId: typeof PYLON_MULTI_EARNING_PROMISE
  promiseState: 'red'
  inert: true
  perMode: ReadonlyArray<ModeWorkReceiptManifestEntry>
  totalReceiptCount: number
  totalDistinctAssignmentRefCount: number
  coverageComplete: boolean
}

/**
 * Build the public-safe WORK-RECEIPT manifest for a set of work receipts. PURE /
 * INERT and always `promiseState: 'red'`. Receipts are first deduped by
 * receiptRef, then grouped by mode (first-seen order); within each mode the
 * `receiptRef`s are split by amount class (each in first-seen order). The per-mode
 * counts, coverage verdict, and install totals are taken from
 * verifyWorkReceiptWorkUnitCoverage, so the manifest (evidence) and the auditor
 * (gate) can never disagree about whether the work units are distinct. Surfacing
 * this list is the per-mode receipt evidence the projection needs so each counted
 * unit — observed, pending, paid, or settled — is dereferenceable.
 */
export const projectPylonWorkReceiptManifest = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): PylonWorkReceiptManifest => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()

  // Preserve first-seen mode order for a stable manifest.
  const order: string[] = []
  const grouped = new Map<string, PylonModeWorkReceipt[]>()
  for (const receipt of deduped) {
    const existing = grouped.get(receipt.mode)
    if (existing === undefined) {
      order.push(receipt.mode)
      grouped.set(receipt.mode, [receipt])
    } else {
      existing.push(receipt)
    }
  }

  // Single source of truth for per-mode counts, coverage, and install totals.
  const coverage = verifyWorkReceiptWorkUnitCoverage(deduped)
  const coverageByMode = new Map(coverage.perMode.map(m => [m.mode, m]))

  const refsOf = (
    modeReceipts: ReadonlyArray<PylonModeWorkReceipt>,
    amountClass: ReceiptableAmountClass,
  ): string[] =>
    modeReceipts
      .filter(r => r.amountClass === amountClass)
      .map(r => r.receiptRef)

  const perMode: ModeWorkReceiptManifestEntry[] = []
  for (const mode of order) {
    const modeReceipts = grouped.get(mode) ?? []
    const modeCoverage = coverageByMode.get(mode)
    perMode.push({
      mode,
      receiptCount: modeCoverage?.receiptCount ?? modeReceipts.length,
      distinctAssignmentRefCount: modeCoverage?.distinctAssignmentRefCount ?? 0,
      workUnitCoverageComplete: modeCoverage?.workUnitCoverageComplete ?? true,
      observedReceiptRefs: refsOf(modeReceipts, 'observed'),
      pendingReceiptRefs: refsOf(modeReceipts, 'pending'),
      paidReceiptRefs: refsOf(modeReceipts, 'paid'),
      settledReceiptRefs: refsOf(modeReceipts, 'settled'),
    })
  }

  return {
    schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
    promiseId: PYLON_MULTI_EARNING_PROMISE,
    promiseState: 'red',
    inert: true,
    perMode,
    totalReceiptCount: coverage.totalReceiptCount,
    totalDistinctAssignmentRefCount: coverage.totalDistinctAssignmentRefCount,
    coverageComplete: coverage.allWorkUnitsDistinct,
  }
}

/**
 * Whole-install ref-namespace-disjointness report. INERT and public-safe: it
 * carries only integer overlap counts. A work receipt names up to THREE genuinely
 * distinct evidence artifacts — `assignmentRef` (the work UNIT), `receiptRef` (the
 * WORK proof) and `settlementReceiptRef` (the SETTLEMENT proof). The "earning
 * depth" claim (work was assigned -> work was done -> it settled) rests on these
 * being three SEPARATE, independently dereferenceable artifacts. This report
 * states how many distinct tokens appear in more than one namespace; each such
 * overlap is one artifact masquerading as two distinct pieces of evidence.
 * `allRefNamespacesDisjoint` is true only when no token is shared across the
 * assignment / receipt / settlement namespaces.
 */
export type WorkReceiptRefDisjointness = {
  schema: typeof PYLON_MULTI_EARNING_RECEIPT_SCHEMA
  /** Distinct tokens used as BOTH an assignmentRef and a receiptRef. */
  assignmentReceiptOverlapCount: number
  /** Distinct tokens used as BOTH an assignmentRef and a settlementReceiptRef. */
  assignmentSettlementOverlapCount: number
  /** Distinct tokens used as BOTH a receiptRef and a settlementReceiptRef. */
  receiptSettlementOverlapCount: number
  /** Distinct tokens that appear in more than one namespace (the union). */
  totalOverlapTokenCount: number
  allRefNamespacesDisjoint: boolean
}

/**
 * Verify that the three ref NAMESPACES of a set of work receipts are mutually
 * disjoint. PURE / INERT. Receipts are first deduped by receiptRef, then their
 * `assignmentRef`s, `receiptRef`s, and `settlementReceiptRef`s are collected into
 * three sets; the report counts how many distinct tokens fall into more than one
 * set. This is the integrity check that lets the three refs on each receipt be
 * TRUSTED as three genuinely distinct artifacts — without it, a single token
 * could pose as a work UNIT, its WORK proof, AND its SETTLEMENT proof at once,
 * faking evidence depth that does not exist. It is the cross-namespace analogue of
 * the per-namespace auditors (work-unit / settlement coverage), which only check
 * distinctness WITHIN a single namespace.
 */
export const verifyWorkReceiptRefDisjointness = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
): WorkReceiptRefDisjointness => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()

  const assignmentRefs = new Set<string>()
  const receiptRefs = new Set<string>()
  const settlementRefs = new Set<string>()
  for (const receipt of deduped) {
    assignmentRefs.add(receipt.assignmentRef)
    receiptRefs.add(receipt.receiptRef)
    if (receipt.settlementReceiptRef !== undefined) {
      settlementRefs.add(receipt.settlementReceiptRef)
    }
  }

  const intersection = (a: Set<string>, b: Set<string>): string[] =>
    [...a].filter(token => b.has(token))

  const assignmentReceipt = intersection(assignmentRefs, receiptRefs)
  const assignmentSettlement = intersection(assignmentRefs, settlementRefs)
  const receiptSettlement = intersection(receiptRefs, settlementRefs)

  const overlapping = new Set<string>([
    ...assignmentReceipt,
    ...assignmentSettlement,
    ...receiptSettlement,
  ])

  return {
    schema: PYLON_MULTI_EARNING_RECEIPT_SCHEMA,
    assignmentReceiptOverlapCount: assignmentReceipt.length,
    assignmentSettlementOverlapCount: assignmentSettlement.length,
    receiptSettlementOverlapCount: receiptSettlement.length,
    totalOverlapTokenCount: overlapping.size,
    allRefNamespacesDisjoint: overlapping.size === 0,
  }
}

/**
 * Fold a set of per-mode work receipts into the projection's earning store.
 * PURE. Receipts are first deduped by receiptRef, then grouped by mode; each
 * mode's amount-class counts come from its receipts, and a mode that carries any
 * settled receipt takes that mode's first settled receipt's settlementReceiptRef
 * as the per-mode representative ref (the full per-receipt detail stays in the
 * work-receipt store). The resulting store feeds projectPylonMultiEarningNode
 * unchanged, so the projection's counts are now backed by receipts.
 *
 * Before folding, three integrity checks run. Work-unit coverage: if any mode's
 * amount-class counts would be inflated by two receipts re-counting one work unit
 * (a shared `assignmentRef` within a mode or reused across modes), the fold
 * REJECTS. Settlement coverage: if any settled count would not be backed by that
 * many DISTINCT settlement receipts (within a mode or reused across modes), the
 * fold REJECTS. Ref-namespace disjointness: if any single token poses in more than
 * one of the assignment / receipt / settlement namespaces (faking evidence depth),
 * the fold REJECTS. Any rejection prevents a projection that over-claims.
 *
 * `modeledCount` is always 0 here: `modeled` is an estimate with no work event,
 * so it has no receipt and cannot be folded in.
 */
export const foldWorkReceiptsIntoEarningStore = (
  receipts: ReadonlyArray<PylonModeWorkReceipt>,
):
  | { ok: true; store: PylonMultiEarningStore }
  | { ok: false; error: PylonMultiEarningError } => {
  const deduped = makeInMemoryPylonModeWorkReceiptStore(receipts).list()

  const workUnits = verifyWorkReceiptWorkUnitCoverage(deduped)
  if (!workUnits.allWorkUnitsDistinct) {
    return {
      ok: false,
      error: new PylonMultiEarningError({
        reason: workUnits.crossModeWorkUnitReuse
          ? 'work-unit over-claim: an assignmentRef is reused across earning modes; each receipt must attest its own distinct work unit'
          : 'work-unit over-claim: a mode has more receipts than distinct work units; each receipt must attest its own distinct assignmentRef',
      }),
    }
  }

  const coverage = verifyWorkReceiptSettlementCoverage(deduped)
  if (!coverage.allModesSettlementCovered) {
    return {
      ok: false,
      error: new PylonMultiEarningError({
        reason: coverage.crossModeSettlementReuse
          ? 'settlement over-claim: a settlementReceiptRef is reused across earning modes; each settled unit requires its own distinct settlement receipt'
          : 'settlement over-claim: a mode has more settled receipts than distinct settlement receipts; each settled unit requires its own distinct settlement receipt',
      }),
    }
  }

  const disjointness = verifyWorkReceiptRefDisjointness(deduped)
  if (!disjointness.allRefNamespacesDisjoint) {
    return {
      ok: false,
      error: new PylonMultiEarningError({
        reason:
          'ref over-claim: a token is shared across the assignment / receipt / settlement namespaces; the work unit, its work receipt, and its settlement receipt must be three genuinely distinct artifacts',
      }),
    }
  }

  // Preserve first-seen mode order for a stable projection.
  const order: string[] = []
  const grouped = new Map<string, PylonModeWorkReceipt[]>()
  for (const receipt of deduped) {
    const existing = grouped.get(receipt.mode)
    if (existing === undefined) {
      order.push(receipt.mode)
      grouped.set(receipt.mode, [receipt])
    } else {
      existing.push(receipt)
    }
  }

  const records = []
  for (const mode of order) {
    const modeReceipts = grouped.get(mode) ?? []
    const countOf = (amountClass: ReceiptableAmountClass): number =>
      modeReceipts.filter(r => r.amountClass === amountClass).length
    const firstSettled = modeReceipts.find(r => r.amountClass === 'settled')

    const built = recordModeEarning({
      mode,
      observedCount: countOf('observed'),
      pendingCount: countOf('pending'),
      paidCount: countOf('paid'),
      settledCount: countOf('settled'),
      ...(firstSettled?.settlementReceiptRef === undefined
        ? {}
        : { settlementReceiptRef: firstSettled.settlementReceiptRef }),
    })
    if (!built.ok) {
      return { ok: false, error: built.error }
    }
    records.push(built.record)
  }

  return { ok: true, store: makeInMemoryPylonMultiEarningStore(records) }
}
