// Spark-helper autostart readiness — INERT, flag-gated capability.
//
// Promise: pylon.consumer_compute_earns_bitcoin_self_serve.v1
// Blocker:  blocker.product_promises.spark_helper_autostart_receipt_missing
//
// The video-core promise ("anybody plugs in consumer compute and gets paid
// Bitcoin") is blocked, in part, because there is no receipt proving that a
// NORMAL contributor's Spark backup helper reaches payout-readiness
// automatically on the self-serve path — without an operator hand-starting it.
//
// This module supplies that capability WITHOUT changing any live behavior:
//
//   - It is INERT by default. The autostart decision only fires when the
//     operator/contributor explicitly opts in via `PYLON_SPARK_AUTOSTART=1`
//     (or `{ enabled: true }`). With the flag off it returns a `disabled`
//     projection and recommends nothing — exactly today's behavior.
//   - It does NOT start a helper, spawn a process, move funds, or touch the
//     wallet. It is a pure classifier over an already-computed
//     `SparkBackupReceiveProjection`, plus a public-safe receipt builder.
//   - It never emits a raw Spark address, lightning address, balance, credential
//     material, or stderr. The receipt is redacted by construction.
//
// When the live self-serve path later wires actual helper autostart, it can call
// `classifySparkHelperAutostart` to gate that action and `buildSparkHelperAutostartReceipt`
// to emit the dereferenceable receipt that clears the blocker. Until then the
// flag stays off and nothing changes.

import type {
  SparkBackupReceiveProjection,
  SparkBackupReceiveState,
} from "./wallet.js"

export const SPARK_AUTOSTART_ENV = "PYLON_SPARK_AUTOSTART" as const

/**
 * The autostart-readiness states.
 *
 * - `disabled`            — flag off; no autostart decision made (default/inert).
 * - `credential-missing`  — opted in, but no local Spark backup credential.
 * - `helper-not-ready`    — opted in, credential present, but the helper has not
 *                           reached an address-ready state.
 * - `autostart-ready`     — opted in, credential present, helper reached a
 *                           payout-ready receive target WITHOUT operator action.
 */
export type SparkHelperAutostartState =
  | "disabled"
  | "credential-missing"
  | "helper-not-ready"
  | "autostart-ready"

export type SparkHelperAutostartProjection = {
  schema: "openagents.pylon.spark_helper_autostart.v0.1"
  enabled: boolean
  state: SparkHelperAutostartState
  // True only in `autostart-ready`: a normal contributor's helper is payout-ready
  // with no operator hand-start required for THIS observation.
  payoutReady: boolean
  // Mirrors the underlying spark-backup receive state this decision was derived
  // from (for traceability). Never carries raw targets.
  derivedFromReceiveState: SparkBackupReceiveState
  blockerRefs: string[]
  nextActionRefs: string[]
  // Public-safe redacted ref for the readiness observation, present only when
  // `autostart-ready`. Never a raw address/balance/credential.
  readinessReceiptRef: string | null
  contentRedacted: true
}

export type SparkHelperAutostartOptions = {
  // Explicit override. When undefined, falls back to the env flag.
  enabled?: boolean
  env?: NodeJS.ProcessEnv
}

const READY_RECEIVE_STATES: ReadonlySet<SparkBackupReceiveState> = new Set<
  SparkBackupReceiveState
>(["address-ready", "cached-address-ready"])

function isAutostartEnabled(
  options: SparkHelperAutostartOptions,
  env: NodeJS.ProcessEnv,
): boolean {
  if (options.enabled !== undefined) return options.enabled
  // INERT by default: only an explicit opt-in turns this on.
  const flag = env[SPARK_AUTOSTART_ENV]
  return flag === "1" || flag === "true"
}

/**
 * Classify whether the Spark backup helper would be considered autostart-ready
 * for a normal contributor, given an already-computed receive projection.
 *
 * Pure and side-effect-free. Default-inert: returns `disabled` unless explicitly
 * opted in.
 */
export function classifySparkHelperAutostart(
  receive: SparkBackupReceiveProjection,
  options: SparkHelperAutostartOptions = {},
): SparkHelperAutostartProjection {
  const env = options.env ?? process.env
  const enabled = isAutostartEnabled(options, env)

  const base: SparkHelperAutostartProjection = {
    schema: "openagents.pylon.spark_helper_autostart.v0.1",
    enabled,
    state: "disabled",
    payoutReady: false,
    derivedFromReceiveState: receive.state,
    blockerRefs: [],
    nextActionRefs: [],
    readinessReceiptRef: null,
    contentRedacted: true,
  }

  if (!enabled) {
    return {
      ...base,
      state: "disabled",
      nextActionRefs: ["action.pylon.spark_autostart.opt_in"],
    }
  }

  if (!receive.credentialReady) {
    return {
      ...base,
      state: "credential-missing",
      blockerRefs: ["blocker.wallet.spark_backup.credential_missing"],
      nextActionRefs: ["action.wallet.spark_backup.configure_local_credential"],
    }
  }

  if (!receive.helperReady && !READY_RECEIVE_STATES.has(receive.state)) {
    return {
      ...base,
      state: "helper-not-ready",
      blockerRefs: ["blocker.product_promises.spark_helper_autostart_receipt_missing"],
      nextActionRefs: ["action.wallet.spark_backup.install_or_start_helper"],
    }
  }

  // The helper reached a payout-ready receive target. For autostart purposes we
  // accept both a fresh `address-ready` and an offline `cached-address-ready`
  // (a cached target is still a payout destination the contributor can receive
  // to without operator action).
  return {
    ...base,
    state: "autostart-ready",
    payoutReady: true,
    readinessReceiptRef: buildSparkHelperAutostartReceiptRef(receive.state),
  }
}

/**
 * Build the public-safe, redacted receipt ref for an autostart-ready
 * observation. Carries only the receive state class, never a raw target.
 */
export function buildSparkHelperAutostartReceiptRef(
  receiveState: SparkBackupReceiveState,
): string {
  return `receipt.pylon.spark_helper_autostart.${receiveState}.v0.1`
}

export type SparkHelperAutostartReceipt = {
  schema: "openagents.pylon.spark_helper_autostart_receipt.v0.1"
  ref: string
  payoutReady: boolean
  derivedFromReceiveState: SparkBackupReceiveState
  // No operator hand-start was required for this observation.
  operatorHandStartRequired: boolean
  observedAt: string
  contentRedacted: true
}

/**
 * Build a dereferenceable, public-safe receipt for an autostart-ready
 * observation. Returns null unless the projection is `autostart-ready`, so a
 * receipt can only exist when readiness was actually observed.
 */
export function buildSparkHelperAutostartReceipt(
  projection: SparkHelperAutostartProjection,
  observedAt: string,
): SparkHelperAutostartReceipt | null {
  if (projection.state !== "autostart-ready" || projection.readinessReceiptRef === null) {
    return null
  }
  return {
    schema: "openagents.pylon.spark_helper_autostart_receipt.v0.1",
    ref: projection.readinessReceiptRef,
    payoutReady: projection.payoutReady,
    derivedFromReceiveState: projection.derivedFromReceiveState,
    operatorHandStartRequired: false,
    observedAt,
    contentRedacted: true,
  }
}

/**
 * Result of auditing a candidate autostart receipt.
 *
 * - `valid`        — the candidate is a well-formed, public-safe autostart
 *                    receipt (correct schema/ref/types, no leak-prone fields).
 * - `clearsBlocker`— true ONLY when valid AND it actually attests payout
 *                    readiness with no operator hand-start. This is the bar a
 *                    real captured receipt must meet before it could ever be
 *                    cited to clear `spark_helper_autostart_receipt_missing`.
 * - `reasons`      — human-readable failure reasons; empty when valid.
 */
export type SparkHelperAutostartReceiptVerification = {
  valid: boolean
  clearsBlocker: boolean
  reasons: string[]
}

// The exact, closed set of keys a public-safe receipt may carry. Any extra key
// is rejected: an unknown field is exactly how a raw target/balance/credential
// could be smuggled into a "receipt" that is then published.
const ALLOWED_RECEIPT_KEYS: ReadonlySet<string> = new Set<string>([
  "schema",
  "ref",
  "payoutReady",
  "derivedFromReceiveState",
  "operatorHandStartRequired",
  "observedAt",
  "contentRedacted",
])

const RECEIPT_REF_PATTERN =
  /^receipt\.pylon\.spark_helper_autostart\.([a-z-]+)\.v0\.1$/

function isReadyReceiveState(value: unknown): value is SparkBackupReceiveState {
  return (
    typeof value === "string" &&
    READY_RECEIVE_STATES.has(value as SparkBackupReceiveState)
  )
}

function isIsoTimestamp(value: unknown): value is string {
  if (typeof value !== "string") return false
  const parsed = Date.parse(value)
  if (Number.isNaN(parsed)) return false
  // Round-trip guard: only accept canonical ISO-8601 (what the builder emits),
  // so loose/ambiguous date strings cannot pass as a "receipt" timestamp.
  return new Date(parsed).toISOString() === value
}

/**
 * Audit a candidate autostart receipt parsed from untrusted input (e.g. a JSON
 * artifact a contributor captured on the self-serve path).
 *
 * Pure and side-effect-free. This does NOT capture or produce a receipt; it is
 * the deterministic gate a reviewer/auditor runs to confirm a captured receipt
 * is well-formed, public-safe, and actually attests no-hand-start payout
 * readiness — the prerequisite for ever citing it against the blocker.
 */
export function verifySparkHelperAutostartReceipt(
  candidate: unknown,
): SparkHelperAutostartReceiptVerification {
  const reasons: string[] = []

  if (typeof candidate !== "object" || candidate === null || Array.isArray(candidate)) {
    return { valid: false, clearsBlocker: false, reasons: ["not-an-object"] }
  }

  const record = candidate as Record<string, unknown>

  // Public-safety: reject any key outside the closed allowlist before reading
  // values, so a leaked field (balance, raw address, credential) fails the audit.
  for (const key of Object.keys(record)) {
    if (!ALLOWED_RECEIPT_KEYS.has(key)) {
      reasons.push(`unexpected-key:${key}`)
    }
  }

  if (record.schema !== "openagents.pylon.spark_helper_autostart_receipt.v0.1") {
    reasons.push("bad-schema")
  }

  const refMatch =
    typeof record.ref === "string" ? RECEIPT_REF_PATTERN.exec(record.ref) : null
  if (!refMatch) {
    reasons.push("bad-ref")
  } else if (!READY_RECEIVE_STATES.has(refMatch[1] as SparkBackupReceiveState)) {
    reasons.push("ref-state-not-payout-ready")
  }

  if (!isReadyReceiveState(record.derivedFromReceiveState)) {
    reasons.push("derived-state-not-payout-ready")
  }

  // The ref's encoded state must match the declared derivedFromReceiveState, so
  // a receipt cannot claim one (ready) state in its ref and another in its body.
  if (refMatch && record.derivedFromReceiveState !== refMatch[1]) {
    reasons.push("ref-state-mismatch")
  }

  if (record.payoutReady !== true) reasons.push("not-payout-ready")
  if (record.operatorHandStartRequired !== false) {
    reasons.push("operator-hand-start-required")
  }
  if (record.contentRedacted !== true) reasons.push("not-redacted")
  if (!isIsoTimestamp(record.observedAt)) reasons.push("bad-observed-at")

  const valid = reasons.length === 0
  return {
    valid,
    // Validity already implies payoutReady && !operatorHandStartRequired.
    clearsBlocker: valid,
    reasons,
  }
}

// ---------------------------------------------------------------------------
// Set-level audit: bind autostart receipts to DISTINCT normal contributors.
//
// `verifySparkHelperAutostartReceipt` audits one anonymous receipt's shape, but
// the blocker is specifically about the NORMAL contributor self-serve path —
// "≥1 normal contributor reaches payout-readiness without an operator
// hand-start". A single anonymous receipt cannot prove that: the autostart
// receipt carries no contributor binding, so an operator could capture one
// receipt on their own host and present it (or copies of it) as evidence for
// "several contributors". This set verifier closes that gap by requiring each
// receipt to be paired with a distinct contributor ref (a public-safe pylonRef),
// auditing every receipt with the single-receipt gate, rejecting any reused
// contributor ref, AND rejecting a byte-identical receipt artifact reused across
// distinct contributor refs. That last check matters because the autostart
// receipt carries no contributor binding: without it, one operator could capture
// a single receipt on their own host and pair copies of it with several
// fabricated contributor refs — every entry would pass (distinct refs + a valid
// receipt) and the set would falsely attest "several distinct contributors". Two
// genuinely independent captures differ at least in `observedAt`, so an exact
// duplicate is evidence of replication, not a second contributor. It mirrors the
// cross-contributor settlement integrity rule in the scale-methodology verifier.
// Pure and side-effect-free; captures nothing.
// ---------------------------------------------------------------------------

export type SparkHelperAutostartContributorEntry = {
  // A public-safe contributor identifier (pylonRef). Never a raw address,
  // balance, credential, or machine identifier.
  contributorRef: string
  // The captured receipt artifact for that contributor (untrusted; audited).
  receipt: unknown
}

export type SparkHelperAutostartReceiptSetVerification = {
  // Every entry is structurally well-formed, each receipt is valid, and the
  // contributor refs are non-empty and distinct.
  valid: boolean
  // True only when valid AND at least one distinct contributor's receipt clears
  // the single-receipt bar. This is the set-level bar a body of captured
  // evidence must meet before it could be cited to clear the blocker.
  clearsBlocker: boolean
  // Count of distinct contributor refs whose receipt cleared the single bar.
  distinctContributorCount: number
  // Per-entry verdicts, in input order, for auditor traceability.
  perEntry: Array<{
    contributorRef: string
    verification: SparkHelperAutostartReceiptVerification
  }>
  reasons: string[]
}

// A public-safe contributor ref must be a non-empty, single-token string (no
// whitespace) — this rejects free-text and the obvious shapes a raw address or
// credential would take if smuggled in as a "contributor ref".
function isContributorRef(value: unknown): value is string {
  return typeof value === "string" && value.length > 0 && !/\s/.test(value)
}

// Stable fingerprint of an (already-valid) receipt for exact-duplicate detection.
// A valid receipt carries only the closed allowlist of primitive-valued keys, so
// serializing them in a fixed key order is deterministic and order-independent of
// the candidate object's own key insertion order.
const FINGERPRINT_KEY_ORDER: readonly string[] = [...ALLOWED_RECEIPT_KEYS].sort()

function canonicalReceiptFingerprint(receipt: unknown): string {
  const record = receipt as Record<string, unknown>
  return FINGERPRINT_KEY_ORDER.map(
    (key) => `${key}=${JSON.stringify(record[key])}`,
  ).join("|")
}

/**
 * Audit a SET of captured autostart receipts, each bound to a contributor.
 *
 * Pure and side-effect-free. `valid` requires every entry to be structurally
 * well-formed (a non-empty, whitespace-free contributor ref + a receipt that
 * passes `verifySparkHelperAutostartReceipt`), all contributor refs to be
 * distinct, AND no receipt artifact to be reused across entries — so neither a
 * reused ref nor a replicated receipt can pass one host off as many contributors.
 * `clearsBlocker` additionally requires at least one distinct contributor. This does NOT clear
 * the blocker; it is the deterministic set-level gate an auditor runs over real
 * captured evidence before any such citation could be made.
 */
export function verifySparkHelperAutostartReceiptSet(
  entries: ReadonlyArray<SparkHelperAutostartContributorEntry>,
): SparkHelperAutostartReceiptSetVerification {
  const reasons: string[] = []

  if (!Array.isArray(entries)) {
    return {
      valid: false,
      clearsBlocker: false,
      distinctContributorCount: 0,
      perEntry: [],
      reasons: ["not-an-array"],
    }
  }

  if (entries.length === 0) {
    return {
      valid: false,
      clearsBlocker: false,
      distinctContributorCount: 0,
      perEntry: [],
      reasons: ["empty-set"],
    }
  }

  const perEntry: SparkHelperAutostartReceiptSetVerification["perEntry"] = []
  const seenContributorRefs = new Set<string>()
  const clearedContributorRefs = new Set<string>()
  // Maps a valid receipt's canonical fingerprint to the entry label that first
  // presented it, so a replicated artifact under a different ref is caught.
  const seenReceiptFingerprints = new Map<string, string>()

  for (let index = 0; index < entries.length; index++) {
    const entry = entries[index]
    const refOk = isContributorRef(entry?.contributorRef)
    const contributorRef = refOk ? entry.contributorRef : ""

    if (!refOk) {
      reasons.push(`bad-contributor-ref:${index}`)
    } else {
      if (seenContributorRefs.has(contributorRef)) {
        reasons.push(`duplicate-contributor-ref:${contributorRef}`)
      }
      seenContributorRefs.add(contributorRef)
    }

    const entryLabel = refOk ? contributorRef : String(index)
    const verification = verifySparkHelperAutostartReceipt(entry?.receipt)
    if (!verification.valid) {
      reasons.push(`entry-receipt-invalid:${entryLabel}`)
    } else {
      // The receipt is well-formed; guard against the same captured artifact being
      // replicated across entries to fake additional contributors.
      const fingerprint = canonicalReceiptFingerprint(entry?.receipt)
      if (seenReceiptFingerprints.has(fingerprint)) {
        reasons.push(`duplicate-receipt-artifact:${entryLabel}`)
      } else {
        seenReceiptFingerprints.set(fingerprint, entryLabel)
      }
    }
    if (refOk && verification.clearsBlocker) {
      clearedContributorRefs.add(contributorRef)
    }

    perEntry.push({ contributorRef, verification })
  }

  const valid = reasons.length === 0
  const distinctContributorCount = clearedContributorRefs.size

  return {
    valid,
    clearsBlocker: valid && distinctContributorCount >= 1,
    distinctContributorCount,
    perEntry,
    reasons,
  }
}

// ---------------------------------------------------------------------------
// Fail-closed capture: classify -> build -> SELF-VERIFY -> canonical serialize.
//
// The classifier, receipt builder, and verifiers above are all the pieces a
// capture needs, but until now NO code path actually produced a gate-valid
// artifact: the capture runbook's "build the receipt and write the JSON" step
// was manual prose. That left a real integrity hole — a capture could write an
// artifact that does not pass `verifySparkHelperAutostartReceipt` (e.g. a future
// builder change, a hand-edited file, or a non-canonical timestamp), and only an
// auditor running the verifier afterwards would notice.
//
// `captureSparkHelperAutostartReceipt` closes that hole by being fail-closed: it
// only returns a `captured: true` result when the receipt it built passes its
// OWN single-receipt audit AND survives a JSON round-trip re-audit. So any
// artifact emitted by the self-serve capture path is gate-valid by construction;
// if anything is off, it returns `captured: false` with reasons instead of
// emitting a bad/leaky artifact. Pure and side-effect-free: it builds the
// in-memory artifact + its canonical serialization but writes nothing itself.
// ---------------------------------------------------------------------------

// Canonical key order for the serialized receipt artifact. Must exactly cover
// the closed allowlist the verifier enforces, so a serialized artifact can never
// carry a key the auditor would reject (and never omit one it requires).
const RECEIPT_KEY_ORDER: readonly (keyof SparkHelperAutostartReceipt)[] = [
  "schema",
  "ref",
  "payoutReady",
  "derivedFromReceiveState",
  "operatorHandStartRequired",
  "observedAt",
  "contentRedacted",
]

/**
 * Serialize a receipt to canonical, deterministic JSON over the closed key
 * allowlist (fixed key order, trailing newline). Independent of the receipt
 * object's own key insertion order, so two captures of the same observation
 * serialize identically — and any two genuinely independent captures differ at
 * least in `observedAt`.
 */
export function serializeSparkHelperAutostartReceipt(
  receipt: SparkHelperAutostartReceipt,
): string {
  const source = receipt as Record<string, unknown>
  const ordered: Record<string, unknown> = {}
  for (const key of RECEIPT_KEY_ORDER) {
    ordered[key] = source[key]
  }
  return `${JSON.stringify(ordered, null, 2)}\n`
}

export type SparkHelperAutostartCaptureResult =
  | {
      captured: false
      // Why no artifact was emitted (e.g. `not-autostart-ready:<state>`,
      // `receipt-not-built`, `self-verify-failed:<reason>`, `round-trip-failed:<reason>`).
      reasons: string[]
      // The classifier projection this attempt was derived from, for traceability.
      projection: SparkHelperAutostartProjection
    }
  | {
      captured: true
      projection: SparkHelperAutostartProjection
      // The redacted, public-safe receipt. Guaranteed to pass
      // `verifySparkHelperAutostartReceipt` with `clearsBlocker: true`.
      receipt: SparkHelperAutostartReceipt
      // The self-audit verdict over `receipt` (always valid here, surfaced for
      // the capture log).
      verification: SparkHelperAutostartReceiptVerification
      // Canonical JSON the self-serve path should persist verbatim. Do NOT
      // hand-edit it: re-audit any edited artifact with the verifier.
      serialized: string
    }

/**
 * Capture a Spark-helper autostart receipt from a live receive projection,
 * fail-closed.
 *
 * Pure and side-effect-free: it classifies, builds, self-verifies, and produces
 * the canonical serialization, but writes nothing. The self-serve capture path
 * calls this and persists `result.serialized` only when `result.captured` — so
 * an artifact that does not pass the audit (or would leak a field) is never
 * emitted. This does NOT clear the blocker; clearing it still needs a REAL
 * captured receipt from a normal contributor that passes the verifier.
 */
export function captureSparkHelperAutostartReceipt(
  receive: SparkBackupReceiveProjection,
  observedAt: string,
  options: SparkHelperAutostartOptions = {},
): SparkHelperAutostartCaptureResult {
  const projection = classifySparkHelperAutostart(receive, options)

  if (projection.state !== "autostart-ready") {
    return {
      captured: false,
      reasons: [`not-autostart-ready:${projection.state}`],
      projection,
    }
  }

  const receipt = buildSparkHelperAutostartReceipt(projection, observedAt)
  if (receipt === null) {
    return { captured: false, reasons: ["receipt-not-built"], projection }
  }

  // Self-audit: never emit an artifact that does not pass its own gate.
  const verification = verifySparkHelperAutostartReceipt(receipt)
  if (!verification.clearsBlocker) {
    return {
      captured: false,
      reasons: verification.reasons.map((r) => `self-verify-failed:${r}`),
      projection,
    }
  }

  // Round-trip guard: the persisted form is what an auditor re-reads, so verify
  // the serialized-then-parsed artifact too. This catches any serializer drift
  // before the artifact is ever written.
  const serialized = serializeSparkHelperAutostartReceipt(receipt)
  const roundTrip = verifySparkHelperAutostartReceipt(JSON.parse(serialized))
  if (!roundTrip.clearsBlocker) {
    return {
      captured: false,
      reasons: roundTrip.reasons.map((r) => `round-trip-failed:${r}`),
      projection,
    }
  }

  return { captured: true, projection, receipt, verification, serialized }
}
