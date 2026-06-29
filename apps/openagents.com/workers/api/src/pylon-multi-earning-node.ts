// Pylon multi-earning-node safe public projection — the public-safe surface
// that distinguishes modeled / observed / pending / paid / settled amounts
// per earning mode for a single Pylon install
// (EPIC #5523 / DE-4 #5527; promise pylon.v0_3_multi_earning_node.v1, red).
//
// THE GAP THIS CLOSES: the promise carries four blockers. Three of them
// (`pylon_v1_default_install_not_fully_closed`, `multi_earning_mode_receipts_missing`,
// `multi_earning_settlement_refs_missing`) are install / receipt / settlement
// work and stay OWNER-GATED. The fourth, `safe_public_projection_missing`, is a
// distinct deliverable named in the receipt-acceptance row of #5527: a
// `modeled/observed/pending/paid/settled`-distinguishing public projection.
// Nothing in the repo produced that projection — this module does, and ONLY
// that. It clears one blocker; it does not flip the promise.
//
// SCOPE / HONESTY: PURE and INERT. It records no real earnings, moves no money,
// reads no wallet, writes no settlement, and admits no install as closed. The
// default (empty) store reports ZERO settled modes and stays `promiseState: red`.
// Even when armed, the projection is honest: it surfaces exactly the amount
// classes a caller hands it and never reports a settled mode the store did not
// carry. The promise pylon.v0_3_multi_earning_node.v1 STAYS `red`; a green flip
// requires settled receipts across >=2 modes in one install AND is receipt-first
// and owner-signed per proof.claim_upgrade_receipts.v1.
//
// PUBLIC-SAFE BY CONSTRUCTION: a record carries only neutral, bounded refs (an
// earning-mode label, an optional public-safe receipt ref) and integer amount
// counts per class. No raw amounts in money units, no wallet / payment / payout
// / customer / provider / secret / raw-timestamp / preimage / bolt11 material.

import { Schema as S } from 'effect'

import { distinctEarningModeFamilies } from './pylon-earning-mode-taxonomy'
import {
  type PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'
import { currentIsoTimestamp } from './runtime-primitives'

export const PYLON_MULTI_EARNING_PROJECTION_SCHEMA =
  'openagents.pylon_multi_earning_node.v1' as const

export const PYLON_MULTI_EARNING_PROMISE =
  'pylon.v0_3_multi_earning_node.v1' as const

// The projection surface clears the projection blocker only; the install,
// per-mode-receipt, and settlement blockers stay owner-gated. All three are
// surfaced so the projection is honest about what remains.
export const PYLON_SAFE_PROJECTION_BLOCKER =
  'blocker.product_promises.safe_public_projection_missing' as const

export const PYLON_MULTI_EARNING_REMAINING_BLOCKERS = [
  'blocker.product_promises.pylon_v1_default_install_not_fully_closed',
  'blocker.product_promises.multi_earning_mode_receipts_missing',
  'blocker.product_promises.multi_earning_settlement_refs_missing',
] as const

// The five amount classes the projection must distinguish, in money-state order:
// modeled (estimated, no observation) -> observed (saw work happen, no payment)
// -> pending (payment in flight, not final) -> paid (payment received, not yet
// settled to the owner treasury) -> settled (final, dereferenceable receipt).
// Only `settled` counts toward the >=2-modes-for-green bar.
export const EARNING_AMOUNT_CLASSES = [
  'modeled',
  'observed',
  'pending',
  'paid',
  'settled',
] as const
export type EarningAmountClass = (typeof EARNING_AMOUNT_CLASSES)[number]

export class PylonMultiEarningError extends S.TaggedErrorClass<PylonMultiEarningError>()(
  'PylonMultiEarningError',
  {
    reason: S.String,
  },
) {}

// A bounded, neutral ref token (same discipline as the signature metering
// surface): no money/secret/customer/path/timestamp/bolt11/preimage material.
const SAFE_TOKEN_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,200}$/
const UNSAFE_TOKEN_PATTERN =
  /(@|\/users\/|\/home\/|access[_-]?token|api[_-]?key|auth\.json|bearer|bolt11|cookie|customer|email|gho_|ghp_|invoice|lnbc|lntb|lnbcrt|lno1|lnurl|mnemonic|oauth|payment|payout|preimage|private|provider|raw[_-]|secret|seed[_-]?phrase|sk-[a-z0-9]|token|wallet)/i
const RAW_TIMESTAMP_PATTERN = /\d{4}-\d{2}-\d{2}t\d{2}:\d{2}:\d{2}/i

const isNonEmpty = (value: string): boolean => value.trim().length > 0

/**
 * Whether `value` is a bounded, neutral, public-safe ref token: non-empty,
 * matches the safe shape, and carries no money / wallet / payment / payout /
 * customer / secret / bolt11 / preimage / raw-timestamp material. Exported so
 * adjacent surfaces (e.g. the per-mode work-receipt layer) reuse exactly this
 * discipline instead of re-deriving it.
 */
export const isPublicSafeToken = (value: string): boolean => {
  const trimmed = value.trim()
  return (
    isNonEmpty(trimmed) &&
    SAFE_TOKEN_PATTERN.test(trimmed) &&
    !UNSAFE_TOKEN_PATTERN.test(trimmed) &&
    !RAW_TIMESTAMP_PATTERN.test(trimmed)
  )
}

const assertSafeToken = (label: string, value: string): string => {
  const trimmed = value.trim()
  if (!isNonEmpty(trimmed)) {
    throw new PylonMultiEarningError({
      reason: `${label} must be a non-empty token`,
    })
  }
  if (!isPublicSafeToken(trimmed)) {
    throw new PylonMultiEarningError({
      reason: `${label} must be a bounded, public-safe token (no money, wallet, payment, payout, customer, secret, bolt11, preimage, or timestamp material)`,
    })
  }
  return trimmed
}

const assertNonNegativeInteger = (label: string, value: number): number => {
  if (!Number.isInteger(value) || value < 0) {
    throw new PylonMultiEarningError({
      reason: `${label} must be a non-negative integer`,
    })
  }
  return value
}

/**
 * One per-mode earning record for a single Pylon install. INERT: it records how
 * many evidence units exist in each amount class for one earning mode, plus an
 * optional public-safe settlement-receipt ref. Carries NO money amount, no
 * wallet, no payment, no payout, no customer/provider material.
 */
export const PylonModeEarningRecord = S.Struct({
  schema: S.Literal(PYLON_MULTI_EARNING_PROJECTION_SCHEMA),
  /** Neutral earning-mode label (e.g. "training", "forum_tips", "compute"). */
  mode: S.String,
  /** Count of evidence units in each amount class. Integers, never money. */
  modeledCount: S.Number,
  observedCount: S.Number,
  pendingCount: S.Number,
  paidCount: S.Number,
  settledCount: S.Number,
  /** Optional public-safe settlement-receipt ref; present iff settledCount>0. */
  settlementReceiptRef: S.optional(S.String),
})
export type PylonModeEarningRecord = typeof PylonModeEarningRecord.Type

/**
 * Build one per-mode earning record from neutral inputs. PURE / validating. A
 * record may only carry a settlement-receipt ref when it reports a settled
 * count, and may only report a settled count when it carries a receipt ref —
 * so the projection can never claim a settled mode with no dereferenceable
 * receipt behind it.
 */
export const recordModeEarning = (input: {
  mode: string
  modeledCount?: number
  observedCount?: number
  pendingCount?: number
  paidCount?: number
  settledCount?: number
  settlementReceiptRef?: string
}):
  | { ok: true; record: PylonModeEarningRecord }
  | { ok: false; error: PylonMultiEarningError } => {
  try {
    const mode = assertSafeToken('mode', input.mode)
    const modeledCount = assertNonNegativeInteger(
      'modeledCount',
      input.modeledCount ?? 0,
    )
    const observedCount = assertNonNegativeInteger(
      'observedCount',
      input.observedCount ?? 0,
    )
    const pendingCount = assertNonNegativeInteger(
      'pendingCount',
      input.pendingCount ?? 0,
    )
    const paidCount = assertNonNegativeInteger('paidCount', input.paidCount ?? 0)
    const settledCount = assertNonNegativeInteger(
      'settledCount',
      input.settledCount ?? 0,
    )

    const settlementReceiptRef =
      input.settlementReceiptRef === undefined
        ? undefined
        : assertSafeToken('settlementReceiptRef', input.settlementReceiptRef)

    // The settled<->receipt invariant: a settled count requires a receipt ref,
    // and a receipt ref requires a settled count. This is what prevents the
    // projection from ever over-claiming settlement.
    if (settledCount > 0 && settlementReceiptRef === undefined) {
      throw new PylonMultiEarningError({
        reason:
          'a settled count requires a public-safe settlementReceiptRef (no settled mode without a dereferenceable receipt)',
      })
    }
    if (settledCount === 0 && settlementReceiptRef !== undefined) {
      throw new PylonMultiEarningError({
        reason:
          'a settlementReceiptRef is only valid alongside a settled count > 0',
      })
    }

    return {
      ok: true,
      record: {
        schema: PYLON_MULTI_EARNING_PROJECTION_SCHEMA,
        mode,
        modeledCount,
        observedCount,
        pendingCount,
        paidCount,
        settledCount,
        ...(settlementReceiptRef === undefined
          ? {}
          : { settlementReceiptRef }),
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
 * An idempotent in-memory store of per-mode earning records. Recording the same
 * mode twice collapses to the first occurrence, so the projection reports one
 * record per mode. Injected so the surface stays pure and testable; the live
 * Worker passes an empty store while INERT.
 */
export type PylonMultiEarningStore = {
  list: () => ReadonlyArray<PylonModeEarningRecord>
}

export const emptyPylonMultiEarningStore: PylonMultiEarningStore = {
  list: () => [],
}

/**
 * Build an in-memory store from a set of per-mode records, collapsing duplicate
 * modes to the first occurrence (one record per mode).
 */
export const makeInMemoryPylonMultiEarningStore = (
  records: ReadonlyArray<PylonModeEarningRecord>,
): PylonMultiEarningStore => {
  const byMode = new Map<string, PylonModeEarningRecord>()
  for (const record of records) {
    if (!byMode.has(record.mode)) {
      byMode.set(record.mode, record)
    }
  }
  const deduped = [...byMode.values()]
  return { list: () => deduped }
}

/** The number of distinct earning-mode LABELS that carry a settled unit. */
export const settledModeCount = (store: PylonMultiEarningStore): number =>
  store.list().filter(record => record.settledCount > 0).length

/**
 * The distinct earning-mode FAMILIES that carry at least one settled unit, in
 * first-seen order. A family collapses version/variant spellings of one earning
 * mode (e.g. "training" and "training_v2") to a single entry, so two labels of
 * the SAME mode cannot inflate the multi-earning count. This is the over-claim
 * guard behind the ">=2 settled modes for green" bar.
 */
export const settledModeFamilies = (
  store: PylonMultiEarningStore,
): ReadonlyArray<string> =>
  distinctEarningModeFamilies(
    store
      .list()
      .filter(record => record.settledCount > 0)
      .map(record => record.mode),
  )

/** The number of distinct settled earning-mode families (label-split-immune). */
export const settledModeFamilyCount = (store: PylonMultiEarningStore): number =>
  settledModeFamilies(store).length

/**
 * Staleness contract for the projection: built fresh from the injected store on
 * every request, so it is `live_at_read` (maxStaleness 0).
 */
export const PylonMultiEarningStaleness: PublicProjectionStalenessContract =
  liveAtReadStaleness(['pylon_multi_earning_record_changed'])

/**
 * The public-safe multi-earning-node projection. Distinguishes the five amount
 * classes per mode, rolls up a node-level summary, and stays honest:
 * `inert: true`, `promiseState: 'red'`, the projection blocker reported as
 * cleared and the three install/receipt/settlement blockers reported as still
 * open and owner-gated. NO money, NO settlement, NO live-earning claim, and the
 * >=2-modes bar is reported but never asserted as met by this surface.
 */
export const projectPylonMultiEarningNode = (
  store: PylonMultiEarningStore,
): {
  schema: typeof PYLON_MULTI_EARNING_PROJECTION_SCHEMA
  promiseId: typeof PYLON_MULTI_EARNING_PROMISE
  promiseState: 'red'
  inert: true
  generatedAt: string
  maxStalenessSeconds: number
  staleness: PublicProjectionStalenessContract
  amountClasses: ReadonlyArray<EarningAmountClass>
  modes: ReadonlyArray<{
    mode: string
    modeledCount: number
    observedCount: number
    pendingCount: number
    paidCount: number
    settledCount: number
    settlementReceiptRef?: string
  }>
  settledModeCount: number
  // Distinct settled earning-mode FAMILIES (version/variant spellings of one
  // mode collapse to one entry). The >=2 bar is measured against THIS, so the
  // multi-earning claim cannot be faked by splitting one mode into two labels.
  settledModeFamilies: ReadonlyArray<string>
  settledModeFamilyCount: number
  // The >=2-modes-in-one-install bar for green. Reported, not asserted: this
  // surface clears the projection blocker, not the receipt/settlement ones.
  settledModesRequiredForGreen: number
  settledModesBarMet: boolean
  clearsBlocker: typeof PYLON_SAFE_PROJECTION_BLOCKER
  remainingOwnerGatedBlockers: typeof PYLON_MULTI_EARNING_REMAINING_BLOCKERS
} => {
  const records = store.list()
  const settled = settledModeCount(store)
  const families = settledModeFamilies(store)
  const settledModesRequiredForGreen = 2
  return {
    schema: PYLON_MULTI_EARNING_PROJECTION_SCHEMA,
    promiseId: PYLON_MULTI_EARNING_PROMISE,
    // Honest: the projection clears one blocker; the promise stays red until
    // settled receipts across >=2 modes exist AND the owner signs the flip.
    promiseState: 'red',
    inert: true,
    generatedAt: currentIsoTimestamp(),
    maxStalenessSeconds: PylonMultiEarningStaleness.maxStalenessSeconds,
    staleness: PylonMultiEarningStaleness,
    amountClasses: EARNING_AMOUNT_CLASSES,
    modes: records.map(record => ({
      mode: record.mode,
      modeledCount: record.modeledCount,
      observedCount: record.observedCount,
      pendingCount: record.pendingCount,
      paidCount: record.paidCount,
      settledCount: record.settledCount,
      ...(record.settlementReceiptRef === undefined
        ? {}
        : { settlementReceiptRef: record.settlementReceiptRef }),
    })),
    settledModeCount: settled,
    settledModeFamilies: families,
    settledModeFamilyCount: families.length,
    settledModesRequiredForGreen,
    // Bar is measured against distinct FAMILIES, not labels: two spellings of
    // one earning mode can never satisfy the multi-earning requirement.
    settledModesBarMet: families.length >= settledModesRequiredForGreen,
    clearsBlocker: PYLON_SAFE_PROJECTION_BLOCKER,
    remainingOwnerGatedBlockers: PYLON_MULTI_EARNING_REMAINING_BLOCKERS,
  }
}
