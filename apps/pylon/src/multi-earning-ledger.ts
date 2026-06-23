// Multi-earning-node local ledger — INERT, flag-gated, contributor-node surface.
//
// Promise: pylon.v0_3_multi_earning_node.v1
// Blocker:  blocker.product_promises.multi_earning_mode_receipts_missing
//
// The multi-earning-node promise ("one Pylon install earns Bitcoin across more
// than one mode") cannot go green until there are SETTLED receipts for >=2
// earning modes captured FROM ONE INSTALL. Pylon already produces per-mode
// earning records, but each lives in its OWN type-specific store with its own
// shape:
//
//   - NIP-90 provider earnings (`ProviderEarningRecord` in provider-nip90.ts)
//   - assignment closeouts (`AssignmentCloseout.receiptRefs` in assignment.ts)
//   - training worker receipts (`TrainingWorkerReceipt` in assignment.ts)
//   - forum tips (tips.ts) / Tassadar executor / data / labor / referral
//
// There is no unified, dereferenceable interface that reads those per-mode
// records, distinguishes amount classes, and HONESTLY counts how many DISTINCT
// modes carry a settled receipt from one install. That missing interface IS the
// named blocker. This module supplies it WITHOUT changing any live behavior:
//
//   - It is INERT by default. The cross-mode ledger only ingests entries an
//     operator/contributor explicitly hands it (the live earning paths do not
//     auto-feed it yet), and the CLI projection is empty unless the install
//     opts in via `PYLON_MULTI_EARNING_LEDGER_ENABLED=1`.
//   - It is PURE and side-effect-free: it classifies, aggregates, self-verifies,
//     and canonically serializes, but reads no wallet, spawns no process, moves
//     no funds, and writes nothing itself.
//   - It is public-safe by construction: a closed key allowlist rejects any
//     entry that would smuggle a raw address, balance, invoice, mnemonic,
//     credential, or local path into a "receipt". Amounts are carried only as
//     sats integers (an aggregate the public projection already exposes), never
//     raw payment material.
//   - It is HONEST: it counts a mode toward the >=2 green bar ONLY when that
//     mode carries a `settled` receipt. Modeled / observed / pending / paid
//     entries are tracked and surfaced but never counted as settled.
//
// When the live earning paths later wire real settled receipts (one operator
// spend / settlement at a time), they feed `MultiEarningLedgerEntry` records
// into `summarizeMultiEarning` to emit the dereferenceable receipt that clears
// the blocker. Until then the flag stays off and nothing changes.

// The earning modes a single Pylon install can carry. This is the mode taxonomy
// the multi-earning promise stacks; each maps to an existing per-mode earning
// source on the contributor node.
export type MultiEarningMode =
  | "compute" // GEPA / Tassadar executor training + benchmark assignments
  | "labor" // NIP-90 compliant-usage labor jobs
  | "tips" // Forum / Site content tips
  | "data" // consented, redacted, valued trace sales
  | "referral" // creator / referral settlement
  | "inference" // NIP-90 text-inference jobs

export const MULTI_EARNING_MODES: readonly MultiEarningMode[] = [
  "compute",
  "labor",
  "tips",
  "data",
  "referral",
  "inference",
]

// The amount classes a per-mode earning record can be in. Only `settled` counts
// toward the green bar; the rest are honest in-flight or projected states.
//
//   - modeled   — a projected/estimated amount; no real event occurred.
//   - observed  — a real earning event was seen but not yet credited.
//   - pending   — credited and awaiting settlement.
//   - paid       — paid out but not yet confirmed settled on-chain/rail.
//   - settled    — fully settled; a real, dereferenceable settlement receipt.
export type MultiEarningAmountClass =
  | "modeled"
  | "observed"
  | "pending"
  | "paid"
  | "settled"

export const MULTI_EARNING_AMOUNT_CLASSES: readonly MultiEarningAmountClass[] = [
  "modeled",
  "observed",
  "pending",
  "paid",
  "settled",
]

export const MULTI_EARNING_LEDGER_ENV =
  "PYLON_MULTI_EARNING_LEDGER_ENABLED" as const

// A single per-mode earning record fed into the cross-mode ledger. Public-safe
// by construction: it carries only an aggregate sats amount and dereferenceable
// refs, NEVER raw payment material.
export type MultiEarningLedgerEntry = {
  schema: "openagents.pylon.multi_earning_entry.v0.1"
  // Which earning mode this record belongs to.
  mode: MultiEarningMode
  // The amount class of this record. Only `settled` counts toward green.
  amountClass: MultiEarningAmountClass
  // Aggregate sats for this record. >= 0 integer. Never raw invoice material.
  amountSats: number
  // A dereferenceable, public-safe receipt ref (e.g. a settlement receipt ref,
  // a provider earning receiptRef, an assignment closeout receiptRef). Must be a
  // single non-whitespace token; never a raw address/invoice/path.
  receiptRef: string
  // Public-safe ref for the source record this entry was derived from (e.g. a
  // resultEventId, assignmentRef, training run ref). Single token.
  sourceRef: string
  // ISO-8601 timestamp of the observation. Canonical (round-trips).
  observedAt: string
  // Always true; the entry is redacted by construction.
  contentRedacted: true
}

export type MultiEarningLedgerOptions = {
  // Explicit override. When undefined, falls back to the env flag.
  enabled?: boolean
  env?: NodeJS.ProcessEnv
}

function isLedgerEnabled(
  options: MultiEarningLedgerOptions,
  env: NodeJS.ProcessEnv,
): boolean {
  if (options.enabled !== undefined) return options.enabled
  // INERT by default: only an explicit opt-in turns the projection on.
  const flag = env[MULTI_EARNING_LEDGER_ENV]
  return flag === "1" || flag === "true"
}

// ---------------------------------------------------------------------------
// Entry validation: public-safe + well-formed.
// ---------------------------------------------------------------------------

// The exact, closed set of keys a public-safe entry may carry. Any extra key is
// rejected: an unknown field is exactly how a raw target/balance/credential
// could be smuggled into a "receipt" that is then surfaced.
const ALLOWED_ENTRY_KEYS: ReadonlySet<string> = new Set<string>([
  "schema",
  "mode",
  "amountClass",
  "amountSats",
  "receiptRef",
  "sourceRef",
  "observedAt",
  "contentRedacted",
])

const MODE_SET: ReadonlySet<string> = new Set<string>(MULTI_EARNING_MODES)
const AMOUNT_CLASS_SET: ReadonlySet<string> = new Set<string>(
  MULTI_EARNING_AMOUNT_CLASSES,
)

// A public-safe ref must be a non-empty, single-token string (no whitespace).
// This rejects free-text and the obvious shapes a raw address/invoice/path
// would take if smuggled in as a "ref".
function isSafeRef(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && !/\s/.test(value)
}

function isIsoTimestamp(value: unknown): value is string {
  if (typeof value !== "string") return false
  const parsed = Date.parse(value)
  if (Number.isNaN(parsed)) return false
  // Round-trip guard: only accept canonical ISO-8601, so loose/ambiguous date
  // strings cannot pass as a "receipt" timestamp.
  return new Date(parsed).toISOString() === value
}

function isNonNegativeIntegerSats(value: unknown): value is number {
  return typeof value === "number" && Number.isInteger(value) && value >= 0
}

export type MultiEarningEntryVerification = {
  valid: boolean
  reasons: string[]
}

/**
 * Audit a candidate per-mode earning entry parsed from untrusted input.
 *
 * Pure and side-effect-free. Rejects any entry that is malformed OR that would
 * leak private material (a key outside the closed allowlist, a ref carrying
 * whitespace, a non-integer/negative amount, a non-canonical timestamp).
 */
export function verifyMultiEarningEntry(
  candidate: unknown,
): MultiEarningEntryVerification {
  if (
    typeof candidate !== "object" ||
    candidate === null ||
    Array.isArray(candidate)
  ) {
    return { valid: false, reasons: ["not-an-object"] }
  }

  const record = candidate as Record<string, unknown>
  const reasons: string[] = []

  // Public-safety: reject any key outside the closed allowlist before reading
  // values, so a leaked field (balance, raw address, invoice) fails the audit.
  for (const key of Object.keys(record)) {
    if (!ALLOWED_ENTRY_KEYS.has(key)) {
      reasons.push(`unexpected-key:${key}`)
    }
  }

  if (record.schema !== "openagents.pylon.multi_earning_entry.v0.1") {
    reasons.push("bad-schema")
  }
  if (typeof record.mode !== "string" || !MODE_SET.has(record.mode)) {
    reasons.push("bad-mode")
  }
  if (
    typeof record.amountClass !== "string" ||
    !AMOUNT_CLASS_SET.has(record.amountClass)
  ) {
    reasons.push("bad-amount-class")
  }
  if (!isNonNegativeIntegerSats(record.amountSats)) {
    reasons.push("bad-amount-sats")
  }
  if (!isSafeRef(record.receiptRef)) {
    reasons.push("bad-receipt-ref")
  }
  if (!isSafeRef(record.sourceRef)) {
    reasons.push("bad-source-ref")
  }
  if (!isIsoTimestamp(record.observedAt)) {
    reasons.push("bad-observed-at")
  }
  if (record.contentRedacted !== true) {
    reasons.push("not-redacted")
  }

  return { valid: reasons.length === 0, reasons }
}

// ---------------------------------------------------------------------------
// Cross-mode aggregation.
// ---------------------------------------------------------------------------

// Per-mode rollup: how much is in each amount class, plus the distinct settled
// receipt refs for that mode (the evidence a mode "counts" toward green).
export type MultiEarningModeRollup = {
  mode: MultiEarningMode
  // Sum of sats per amount class.
  amountSatsByClass: Record<MultiEarningAmountClass, number>
  // Total record count per amount class.
  recordCountByClass: Record<MultiEarningAmountClass, number>
  // Distinct settled receipt refs for this mode (sorted, deduped).
  settledReceiptRefs: string[]
  // True when this mode carries >=1 distinct settled receipt.
  hasSettledReceipt: boolean
}

export type MultiEarningSummary = {
  schema: "openagents.pylon.multi_earning_summary.v0.1"
  promiseId: "pylon.v0_3_multi_earning_node.v1"
  // Mirrors the registry: the promise is RED until owner-flipped.
  promiseState: "red"
  // The single blocker this ledger addresses.
  clearsBlocker: "blocker.product_promises.multi_earning_mode_receipts_missing"
  // INERT projection (no entries) unless the install opted in.
  enabled: boolean
  inert: boolean
  // Per-mode rollups, only for modes that carry >=1 valid entry, mode-sorted.
  modes: MultiEarningModeRollup[]
  // Count of DISTINCT modes carrying >=1 settled receipt. This is the value
  // compared against the >=2 green bar.
  settledModeCount: number
  // The distinct mode names that carry a settled receipt (sorted).
  settledModes: MultiEarningMode[]
  // The >=2 bar the promise requires.
  requiredSettledModes: 2
  // True ONLY when settledModeCount >= 2. This does NOT flip the promise; it is
  // the honest machine-readable signal that the receipt bar is met.
  meetsMultiEarningBar: boolean
  // The remaining blockers that stay owner-gated regardless of this ledger.
  remainingOwnerGatedBlockers: string[]
  // Count of entries rejected by the public-safety/well-formedness audit.
  rejectedEntryCount: number
  observedAt: string
  contentRedacted: true
}

const REMAINING_OWNER_GATED_BLOCKERS: readonly string[] = [
  "blocker.product_promises.pylon_v1_default_install_not_fully_closed",
  "blocker.product_promises.multi_earning_settlement_refs_missing",
  "blocker.product_promises.safe_public_projection_missing",
]

function emptyClassRecord(): Record<MultiEarningAmountClass, number> {
  return {
    modeled: 0,
    observed: 0,
    pending: 0,
    paid: 0,
    settled: 0,
  }
}

/**
 * Summarize a set of per-mode earning entries into a cross-mode multi-earning
 * projection from one install.
 *
 * Pure and side-effect-free. Default-inert: with the flag off (and no explicit
 * `enabled: true`) the projection reports `inert: true`, ingests no entries, and
 * reports zero settled modes — exactly today's behavior. When armed, it audits
 * every entry with `verifyMultiEarningEntry`, drops the rejects (counting them),
 * rolls valid entries per mode + amount class, and HONESTLY counts how many
 * distinct modes carry a settled receipt. It never flips the promise; it only
 * reports whether the >=2 settled-mode bar is met.
 */
export function summarizeMultiEarning(
  entries: ReadonlyArray<MultiEarningLedgerEntry>,
  observedAt: string,
  options: MultiEarningLedgerOptions = {},
): MultiEarningSummary {
  const env = options.env ?? process.env
  const enabled = isLedgerEnabled(options, env)

  const base: MultiEarningSummary = {
    schema: "openagents.pylon.multi_earning_summary.v0.1",
    promiseId: "pylon.v0_3_multi_earning_node.v1",
    promiseState: "red",
    clearsBlocker:
      "blocker.product_promises.multi_earning_mode_receipts_missing",
    enabled,
    inert: !enabled,
    modes: [],
    settledModeCount: 0,
    settledModes: [],
    requiredSettledModes: 2,
    meetsMultiEarningBar: false,
    remainingOwnerGatedBlockers: [...REMAINING_OWNER_GATED_BLOCKERS],
    rejectedEntryCount: 0,
    observedAt,
    contentRedacted: true,
  }

  // INERT by default: ingest nothing, report an honest empty projection. This is
  // the production default, so the live earning paths are untouched.
  if (!enabled) {
    return base
  }

  const accumulators = new Map<
    MultiEarningMode,
    {
      amountSatsByClass: Record<MultiEarningAmountClass, number>
      recordCountByClass: Record<MultiEarningAmountClass, number>
      settledReceiptRefs: Set<string>
    }
  >()

  let rejectedEntryCount = 0

  for (const entry of entries) {
    const verification = verifyMultiEarningEntry(entry)
    if (!verification.valid) {
      rejectedEntryCount += 1
      continue
    }

    const mode = entry.mode
    let acc = accumulators.get(mode)
    if (!acc) {
      acc = {
        amountSatsByClass: emptyClassRecord(),
        recordCountByClass: emptyClassRecord(),
        settledReceiptRefs: new Set<string>(),
      }
      accumulators.set(mode, acc)
    }

    acc.amountSatsByClass[entry.amountClass] += entry.amountSats
    acc.recordCountByClass[entry.amountClass] += 1
    // Only a `settled` record contributes a settled receipt ref.
    if (entry.amountClass === "settled") {
      acc.settledReceiptRefs.add(entry.receiptRef)
    }
  }

  const modeRollups: MultiEarningModeRollup[] = []
  const settledModes: MultiEarningMode[] = []

  for (const mode of MULTI_EARNING_MODES) {
    const acc = accumulators.get(mode)
    if (!acc) continue
    const settledReceiptRefs = [...acc.settledReceiptRefs].sort()
    const hasSettledReceipt = settledReceiptRefs.length > 0
    if (hasSettledReceipt) settledModes.push(mode)
    modeRollups.push({
      mode,
      amountSatsByClass: acc.amountSatsByClass,
      recordCountByClass: acc.recordCountByClass,
      settledReceiptRefs,
      hasSettledReceipt,
    })
  }

  const settledModeCount = settledModes.length

  return {
    ...base,
    modes: modeRollups,
    settledModeCount,
    settledModes,
    meetsMultiEarningBar: settledModeCount >= 2,
    rejectedEntryCount,
  }
}

// ---------------------------------------------------------------------------
// Fail-closed receipt capture: summarize -> SELF-VERIFY the bar -> serialize.
//
// The summary above is the projection; this is the dereferenceable RECEIPT a
// capture persists ONLY when the >=2 settled-mode bar is actually met. Like the
// spark-helper-autostart capture, it is fail-closed: it returns `captured: true`
// only when the summary honestly meets the bar AND survives a JSON round-trip
// re-audit, so any emitted artifact is gate-valid by construction. It does NOT
// flip the promise; clearing the blocker still needs a REAL summary built from
// real settled receipts AND owner sign-off.
// ---------------------------------------------------------------------------

export type MultiEarningReceipt = {
  schema: "openagents.pylon.multi_earning_receipt.v0.1"
  ref: string
  promiseId: "pylon.v0_3_multi_earning_node.v1"
  settledModeCount: number
  settledModes: MultiEarningMode[]
  // The distinct settled receipt refs that back this multi-earning attestation,
  // sorted and deduped across modes.
  settledReceiptRefs: string[]
  meetsMultiEarningBar: true
  observedAt: string
  contentRedacted: true
}

export function buildMultiEarningReceiptRef(
  settledModeCount: number,
): string {
  return `receipt.pylon.multi_earning_node.modes_${settledModeCount}.v0.1`
}

const RECEIPT_KEY_ORDER: readonly (keyof MultiEarningReceipt)[] = [
  "schema",
  "ref",
  "promiseId",
  "settledModeCount",
  "settledModes",
  "settledReceiptRefs",
  "meetsMultiEarningBar",
  "observedAt",
  "contentRedacted",
]

/**
 * Serialize a multi-earning receipt to canonical, deterministic JSON over the
 * closed key order (trailing newline). Independent of the object's own key
 * insertion order, so the persisted artifact is stable and re-auditable.
 */
export function serializeMultiEarningReceipt(receipt: MultiEarningReceipt): string {
  const source = receipt as Record<string, unknown>
  const ordered: Record<string, unknown> = {}
  for (const key of RECEIPT_KEY_ORDER) {
    ordered[key] = source[key]
  }
  return `${JSON.stringify(ordered, null, 2)}\n`
}

const ALLOWED_RECEIPT_KEYS: ReadonlySet<string> = new Set<string>(
  RECEIPT_KEY_ORDER as readonly string[],
)

export type MultiEarningReceiptVerification = {
  valid: boolean
  // True only when valid AND it attests >=2 settled modes. This is the bar a
  // captured receipt must meet before it could be cited against the blocker.
  clearsBlocker: boolean
  reasons: string[]
}

/**
 * Audit a candidate multi-earning receipt parsed from untrusted input.
 *
 * Pure and side-effect-free. Rejects malformed or leak-prone artifacts and only
 * reports `clearsBlocker` when the receipt honestly attests >=2 settled modes
 * with a settled receipt ref per settled mode.
 */
export function verifyMultiEarningReceipt(
  candidate: unknown,
): MultiEarningReceiptVerification {
  if (
    typeof candidate !== "object" ||
    candidate === null ||
    Array.isArray(candidate)
  ) {
    return { valid: false, clearsBlocker: false, reasons: ["not-an-object"] }
  }

  const record = candidate as Record<string, unknown>
  const reasons: string[] = []

  for (const key of Object.keys(record)) {
    if (!ALLOWED_RECEIPT_KEYS.has(key)) {
      reasons.push(`unexpected-key:${key}`)
    }
  }

  if (record.schema !== "openagents.pylon.multi_earning_receipt.v0.1") {
    reasons.push("bad-schema")
  }
  if (record.promiseId !== "pylon.v0_3_multi_earning_node.v1") {
    reasons.push("bad-promise-id")
  }
  if (record.meetsMultiEarningBar !== true) {
    reasons.push("does-not-meet-bar")
  }
  if (record.contentRedacted !== true) {
    reasons.push("not-redacted")
  }
  if (!isIsoTimestamp(record.observedAt)) {
    reasons.push("bad-observed-at")
  }

  const settledModes = record.settledModes
  const settledModesOk =
    Array.isArray(settledModes) &&
    settledModes.length >= 2 &&
    settledModes.every((m) => typeof m === "string" && MODE_SET.has(m)) &&
    new Set(settledModes).size === settledModes.length
  if (!settledModesOk) {
    reasons.push("bad-settled-modes")
  }

  if (
    typeof record.settledModeCount !== "number" ||
    !Number.isInteger(record.settledModeCount) ||
    record.settledModeCount < 2 ||
    (Array.isArray(settledModes) &&
      record.settledModeCount !== settledModes.length)
  ) {
    reasons.push("bad-settled-mode-count")
  }

  const settledReceiptRefs = record.settledReceiptRefs
  const settledReceiptRefsOk =
    Array.isArray(settledReceiptRefs) &&
    settledReceiptRefs.length >= 2 &&
    settledReceiptRefs.every((r) => isSafeRef(r)) &&
    new Set(settledReceiptRefs).size === settledReceiptRefs.length
  if (!settledReceiptRefsOk) {
    reasons.push("bad-settled-receipt-refs")
  }

  const refMatch =
    typeof record.ref === "string" &&
    /^receipt\.pylon\.multi_earning_node\.modes_(\d+)\.v0\.1$/.test(record.ref)
  if (!refMatch) {
    reasons.push("bad-ref")
  } else if (
    typeof record.settledModeCount === "number" &&
    record.ref !== buildMultiEarningReceiptRef(record.settledModeCount)
  ) {
    reasons.push("ref-count-mismatch")
  }

  const valid = reasons.length === 0
  return { valid, clearsBlocker: valid, reasons }
}

export type MultiEarningCaptureResult =
  | {
      captured: false
      // Why no artifact was emitted (e.g. `inert`, `below-bar:<n>`,
      // `self-verify-failed:<reason>`, `round-trip-failed:<reason>`).
      reasons: string[]
      summary: MultiEarningSummary
    }
  | {
      captured: true
      summary: MultiEarningSummary
      receipt: MultiEarningReceipt
      verification: MultiEarningReceiptVerification
      // Canonical JSON the capture path should persist verbatim.
      serialized: string
    }

/**
 * Capture a multi-earning receipt from a set of per-mode entries, fail-closed.
 *
 * Pure and side-effect-free: it summarizes, builds, self-verifies, and produces
 * the canonical serialization, but writes nothing. The capture path calls this
 * and persists `result.serialized` only when `result.captured` — so an artifact
 * that does not honestly meet the >=2 settled-mode bar (or would leak a field)
 * is never emitted. This does NOT clear the blocker; clearing it still needs a
 * REAL set of settled receipts from one install AND owner sign-off.
 */
export function captureMultiEarningReceipt(
  entries: ReadonlyArray<MultiEarningLedgerEntry>,
  observedAt: string,
  options: MultiEarningLedgerOptions = {},
): MultiEarningCaptureResult {
  const summary = summarizeMultiEarning(entries, observedAt, options)

  if (summary.inert) {
    return { captured: false, reasons: ["inert"], summary }
  }
  if (!summary.meetsMultiEarningBar) {
    return {
      captured: false,
      reasons: [`below-bar:${summary.settledModeCount}`],
      summary,
    }
  }

  const settledReceiptRefs = [
    ...new Set(
      summary.modes
        .filter((m) => m.hasSettledReceipt)
        .flatMap((m) => m.settledReceiptRefs),
    ),
  ].sort()

  const receipt: MultiEarningReceipt = {
    schema: "openagents.pylon.multi_earning_receipt.v0.1",
    ref: buildMultiEarningReceiptRef(summary.settledModeCount),
    promiseId: "pylon.v0_3_multi_earning_node.v1",
    settledModeCount: summary.settledModeCount,
    settledModes: [...summary.settledModes],
    settledReceiptRefs,
    meetsMultiEarningBar: true,
    observedAt,
    contentRedacted: true,
  }

  // Self-audit: never emit an artifact that does not pass its own gate.
  const verification = verifyMultiEarningReceipt(receipt)
  if (!verification.clearsBlocker) {
    return {
      captured: false,
      reasons: verification.reasons.map((r) => `self-verify-failed:${r}`),
      summary,
    }
  }

  // Round-trip guard: the persisted form is what an auditor re-reads.
  const serialized = serializeMultiEarningReceipt(receipt)
  const roundTrip = verifyMultiEarningReceipt(JSON.parse(serialized))
  if (!roundTrip.clearsBlocker) {
    return {
      captured: false,
      reasons: roundTrip.reasons.map((r) => `round-trip-failed:${r}`),
      summary,
    }
  }

  return { captured: true, summary, receipt, verification, serialized }
}
