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
 * Fold a set of per-mode work receipts into the projection's earning store.
 * PURE. Receipts are first deduped by receiptRef, then grouped by mode; each
 * mode's amount-class counts come from its receipts, and a mode that carries any
 * settled receipt takes that mode's first settled receipt's settlementReceiptRef
 * as the per-mode representative ref (the full per-receipt detail stays in the
 * work-receipt store). The resulting store feeds projectPylonMultiEarningNode
 * unchanged, so the projection's counts are now backed by receipts.
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
