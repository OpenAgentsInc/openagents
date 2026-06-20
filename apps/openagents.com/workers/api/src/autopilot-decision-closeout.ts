import type {
  AutopilotWorkReviewAction,
  AutopilotWorkReviewDecisionRecord,
} from './autopilot-work-routes'

// #5004 receipt-backed command closeout — LIVE worker-api review path.
//
// `autopilot-decision-routes.ts#actOnDecision` is the one decision-queue act
// that is actually wired today: it records a work-order review decision
// (accept / reject / request_changes) and returns the refreshed projection. But
// it produces NO canonical closeout artifact — there is nothing a later audit
// can dereference to prove "this queued decision was resolved, here is exactly
// what happened, exactly once". The protocol-side `DecisionCloseoutReceipt`
// (packages/autopilot-control-protocol) covers the REMOTE Pylon-bridge path; it
// is a different surface and is not reachable from the worker-api review store.
//
// This module is the missing closeout for the live HTTP path: a pure,
// tamper-verifiable receipt built from the recorded review decision plus the
// store's idempotency verdict. It mirrors the protocol-side receipt's contract
// (deterministic `line`, validator that reconstructs the line so any field
// tamper invalidates the receipt) so the two surfaces stay legible to one audit.
//
// Pure: no I/O, no time, no store. The caller passes the recorded facts and the
// resolution timestamp; this module classifies and formats. Evidence-only — the
// receipt carries refs, never raw payloads or secrets.

// The terminal outcome of a worker-api review closeout. `applied` is the first
// recording of a decision; `duplicate` is the store's idempotent replay of the
// same `Idempotency-Key` (still a closeout, never a second effect).
export type AutopilotDecisionCloseoutOutcome = 'applied' | 'duplicate'

// The resolved work-order state a review action maps to (mirrors the store's
// `recordReviewDecision` state argument).
export type AutopilotDecisionCloseoutState =
  | 'accepted'
  | 'rejected'
  | 'revision_required'

export type AutopilotDecisionCloseoutReceipt = Readonly<{
  kind: 'autopilot_decision_closeout_receipt'
  // The queue decision ref that was resolved.
  decisionRef: string
  // The work order the decision belongs to.
  workOrderRef: string
  // The review action the owner took.
  action: AutopilotWorkReviewAction
  // The resolved work-order state.
  resolvedState: AutopilotDecisionCloseoutState
  // applied (first) vs duplicate (idempotent replay).
  outcome: AutopilotDecisionCloseoutOutcome
  // Who resolved it (the acting agent's user id — already public-safe).
  actorAgentUserId: string
  // The exactly-once closeout key a downstream ledger attributes the receipt to.
  closeoutRef: string
  // ISO timestamp the decision was resolved at.
  decidedAt: string
  // Public-safe receipt refs captured at resolution (no raw payloads/secrets).
  receiptRefs: ReadonlyArray<string>
  // Whether the resolution carried any free-text answer/context (always false on
  // the review path today; reserved so the field is stable across surfaces).
  hasAnswer: boolean
  // Deterministic, human-readable digest; reconstructed by the validator.
  line: string
}>

// Public-safe ref shape — no raw payloads, secrets, or free text in a receipt.
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{2,240}$/

const normalizeRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      refs.map(ref => ref.trim()).filter(ref => safeRefPattern.test(ref)),
    ),
  ].sort()

const resolvedStateForAction = (
  action: AutopilotWorkReviewAction,
): AutopilotDecisionCloseoutState =>
  action === 'accept'
    ? 'accepted'
    : action === 'reject'
      ? 'rejected'
      : 'revision_required'

// The receipt refs the review decision recorded, by action — the same public
// refs the completed-decision projection surfaces.
const reviewReceiptRefs = (
  reviewDecision: AutopilotWorkReviewDecisionRecord,
  workOrderRef: string,
): ReadonlyArray<string> =>
  normalizeRefs([
    `receipt.review.${reviewDecision.action}.${workOrderRef}`,
    ...reviewDecision.decisionRefs,
    ...reviewDecision.rejectionRefs,
    ...reviewDecision.revisionRequestRefs,
  ])

const formatCloseoutLine = (input: {
  decisionRef: string
  action: AutopilotWorkReviewAction
  resolvedState: AutopilotDecisionCloseoutState
  outcome: AutopilotDecisionCloseoutOutcome
  actorAgentUserId: string
  decidedAt: string
}): string =>
  `Autopilot decision ${input.decisionRef} ${input.action} closed out as ` +
  `${input.outcome} (${input.resolvedState}) by ${input.actorAgentUserId} ` +
  `at ${input.decidedAt}.`

// The exactly-once closeout key for a resolved review decision. A re-recorded
// (idempotent) decision yields the SAME ref, so a downstream ledger dedups it to
// one canonical closeout. Exported as the single source of truth so the audit
// reconciler derives the same key the builder stamps.
export const autopilotDecisionCloseoutRef = (
  action: AutopilotWorkReviewAction,
  workOrderRef: string,
): string => `decision.closeout.${action}.${workOrderRef}`

export type BuildAutopilotDecisionCloseoutReceiptInput = Readonly<{
  decisionRef: string
  workOrderRef: string
  reviewDecision: AutopilotWorkReviewDecisionRecord
  // The store's idempotency verdict: true → this was a replay (duplicate).
  idempotent: boolean
  decidedAt: string
}>

// Build the canonical closeout receipt for a resolved review decision. Pure.
export const buildAutopilotDecisionCloseoutReceipt = (
  input: BuildAutopilotDecisionCloseoutReceiptInput,
): AutopilotDecisionCloseoutReceipt => {
  const { reviewDecision, workOrderRef, decisionRef } = input
  const resolvedState = resolvedStateForAction(reviewDecision.action)
  const outcome: AutopilotDecisionCloseoutOutcome = input.idempotent
    ? 'duplicate'
    : 'applied'

  return {
    kind: 'autopilot_decision_closeout_receipt',
    decisionRef,
    workOrderRef,
    action: reviewDecision.action,
    resolvedState,
    outcome,
    actorAgentUserId: reviewDecision.actorAgentUserId,
    // Exactly-once key: a re-recorded (idempotent) decision yields the SAME
    // closeoutRef, so a downstream ledger dedups it to one canonical closeout.
    closeoutRef: autopilotDecisionCloseoutRef(reviewDecision.action, workOrderRef),
    decidedAt: input.decidedAt,
    receiptRefs: reviewReceiptRefs(reviewDecision, workOrderRef),
    hasAnswer: false,
    line: formatCloseoutLine({
      decisionRef,
      action: reviewDecision.action,
      resolvedState,
      outcome,
      actorAgentUserId: reviewDecision.actorAgentUserId,
      decidedAt: input.decidedAt,
    }),
  }
}

const REVIEW_ACTIONS: ReadonlySet<string> = new Set<AutopilotWorkReviewAction>([
  'accept',
  'reject',
  'request_changes',
])

const CLOSEOUT_OUTCOMES: ReadonlySet<string> =
  new Set<AutopilotDecisionCloseoutOutcome>(['applied', 'duplicate'])

const CLOSEOUT_STATES: ReadonlySet<string> =
  new Set<AutopilotDecisionCloseoutState>([
    'accepted',
    'rejected',
    'revision_required',
  ])

const isStringArray = (value: unknown): value is ReadonlyArray<string> =>
  Array.isArray(value) && value.every(item => typeof item === 'string')

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

// Validate a closeout receipt and verify its line was not tampered with. Returns
// true only when every field is well-typed, consistent, and the canonical line
// reconstructs byte-identically from the validated fields.
export const validateAutopilotDecisionCloseoutReceipt = (
  receipt: unknown,
): boolean => {
  if (!isRecord(receipt)) return false
  if (receipt.kind !== 'autopilot_decision_closeout_receipt') return false
  if (typeof receipt.decisionRef !== 'string') return false
  if (typeof receipt.workOrderRef !== 'string') return false
  if (typeof receipt.action !== 'string' || !REVIEW_ACTIONS.has(receipt.action)) {
    return false
  }
  if (
    typeof receipt.resolvedState !== 'string' ||
    !CLOSEOUT_STATES.has(receipt.resolvedState)
  ) {
    return false
  }
  if (
    typeof receipt.outcome !== 'string' ||
    !CLOSEOUT_OUTCOMES.has(receipt.outcome)
  ) {
    return false
  }
  if (typeof receipt.actorAgentUserId !== 'string') return false
  if (typeof receipt.closeoutRef !== 'string') return false
  if (typeof receipt.decidedAt !== 'string') return false
  if (!isStringArray(receipt.receiptRefs)) return false
  if (typeof receipt.hasAnswer !== 'boolean') return false
  if (typeof receipt.line !== 'string') return false

  const action = receipt.action as AutopilotWorkReviewAction
  const resolvedState = receipt.resolvedState as AutopilotDecisionCloseoutState
  const outcome = receipt.outcome as AutopilotDecisionCloseoutOutcome

  // The action must map to the recorded resolved state.
  if (resolvedStateForAction(action) !== resolvedState) {
    return false
  }

  return (
    receipt.line ===
    formatCloseoutLine({
      decisionRef: receipt.decisionRef,
      action,
      resolvedState,
      outcome,
      actorAgentUserId: receipt.actorAgentUserId,
      decidedAt: receipt.decidedAt,
    })
  )
}
