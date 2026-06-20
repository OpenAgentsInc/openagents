// #5004 decision-queue action layer. autopilot.decision_queue.v1 promises a queue
// of EIGHT named actions — continue, steer, provide context, rerun tests, retry
// with another account, stop, accept, create a follow-up mission — but the wire
// only speaks the 3-verb decision.resolve form (approve / deny / answer). This
// module is the missing typed API surface between them: it declares the explicit
// action enum, the authority each action exercises (so the route layer can gate
// owner approval), a deterministic idempotency key (the exactly-once command
// handle), and the lowering to the existing decision.resolve wire command.
//
// Pure + transport-agnostic, matching the rest of this package. It grants NO new
// authority: it only CLASSIFIES which actions need owner approval and refuses to
// build a command for an authority-bearing action that has not been approved.
// Resolving the command still flows through the capability-scoped bridge, which
// is enforced node-side against the stored pairing claims.

import type { DecisionResolveCommand } from "./decision-resolve-command.js"
import type { DecisionVerb } from "./decision.js"

// The eight decision-queue actions named in the autopilot.decision_queue.v1 claim.
export type DecisionQueueAction =
  | "continue"
  | "steer"
  | "provide_context"
  | "rerun_tests"
  | "retry_with_another_account"
  | "stop"
  | "accept"
  | "create_follow_up"

// The authority class an action exercises. The authorityBoundary for this promise
// is that a visible decision does NOT itself grant account, spend, deploy, or
// continuation authority — so any action above "none" must carry explicit owner
// approval before a command is built.
export type DecisionQueueAuthority =
  | "none" // informational / stop — no elevated authority
  | "continuation" // lets the agent keep working
  | "spend" // consumes compute (rerun)
  | "account" // switches the acting account
  | "mission_creation" // spawns follow-up work

// Whether the action carries a free-text argument lowered into the wire `answer`.
export type DecisionQueuePayload = "none" | "required" | "optional"

export type DecisionQueueActionSpec = {
  action: DecisionQueueAction
  // The wire verb this action lowers to.
  verb: DecisionVerb
  authority: DecisionQueueAuthority
  // Derived from authority: true unless authority === "none".
  requiresOwnerApproval: boolean
  payload: DecisionQueuePayload
  // Human label for the payload field (for UI + error copy).
  payloadLabel?: string
}

function spec(
  action: DecisionQueueAction,
  verb: DecisionVerb,
  authority: DecisionQueueAuthority,
  payload: DecisionQueuePayload,
  payloadLabel?: string,
): DecisionQueueActionSpec {
  return {
    action,
    verb,
    authority,
    requiresOwnerApproval: authority !== "none",
    payload,
    ...(payloadLabel === undefined ? {} : { payloadLabel }),
  }
}

export const DECISION_QUEUE_ACTION_SPECS: Readonly<Record<DecisionQueueAction, DecisionQueueActionSpec>> = {
  continue: spec("continue", "approve", "continuation", "none"),
  steer: spec("steer", "answer", "continuation", "required", "guidance"),
  provide_context: spec("provide_context", "answer", "none", "required", "context"),
  rerun_tests: spec("rerun_tests", "approve", "spend", "none"),
  retry_with_another_account: spec("retry_with_another_account", "approve", "account", "optional", "accountRef"),
  stop: spec("stop", "deny", "none", "none"),
  accept: spec("accept", "approve", "continuation", "none"),
  create_follow_up: spec("create_follow_up", "answer", "mission_creation", "required", "title"),
}

export const DECISION_QUEUE_ACTIONS: readonly DecisionQueueAction[] = Object.keys(
  DECISION_QUEUE_ACTION_SPECS,
) as DecisionQueueAction[]

export function isDecisionQueueAction(value: string): value is DecisionQueueAction {
  return Object.prototype.hasOwnProperty.call(DECISION_QUEUE_ACTION_SPECS, value)
}

// Deterministic exactly-once command handle. A decision resolves once; a network
// retry of the SAME action on the same request must dedup to this key (the node's
// exactly-once relay treats a repeat as a duplicate, not a new action), while a
// DIFFERENT action on the same request is a distinct key that the decision state
// machine rejects as already_resolved. Stable across desktop / web / Expo.
export function decisionQueueIdempotencyKey(requestId: string, action: DecisionQueueAction): string {
  return `dq:${requestId.trim()}:${action}`
}

export type BuildDecisionQueueCommandInput = {
  // The decision/approval requestId (the exactly-once decision key).
  requestId: string
  action: DecisionQueueAction | string
  // Free-text argument for steer / provide_context / create_follow_up (and the
  // optional account ref for retry_with_another_account).
  payload?: string
  // The caller (route layer) must pass true for authority-bearing actions to
  // confirm the owner approved this specific action. Defaults to false.
  ownerApproved?: boolean
}

export type DecisionQueueCommand = DecisionResolveCommand & {
  // The richer action this wire command was lowered from.
  action: DecisionQueueAction
  idempotencyKey: string
  authority: DecisionQueueAuthority
  requiresOwnerApproval: boolean
}

export type BuildDecisionQueueCommandResult = {
  ok: boolean
  command: DecisionQueueCommand | null
  errors: string[]
}

// Validate a decision-queue action and lower it to a receipt-backed
// decision.resolve command. Refuses (ok:false, command:null) when the request id
// is blank, the action is unknown, a required payload is missing, or an
// authority-bearing action lacks owner approval.
export function buildDecisionQueueCommand(
  input: BuildDecisionQueueCommandInput,
): BuildDecisionQueueCommandResult {
  const errors: string[] = []
  const ref = input.requestId.trim()
  if (ref.length === 0) errors.push("requestId is required")

  if (!isDecisionQueueAction(input.action)) {
    errors.push(
      `action must be one of: ${DECISION_QUEUE_ACTIONS.join(", ")}`,
    )
    return { ok: false, command: null, errors }
  }

  const spec = DECISION_QUEUE_ACTION_SPECS[input.action]
  const payload = input.payload?.trim()
  const hasPayload = payload !== undefined && payload.length > 0

  if (spec.payload === "required" && !hasPayload) {
    errors.push(`${spec.payloadLabel ?? "payload"} is required for ${spec.action}`)
  }

  if (spec.requiresOwnerApproval && input.ownerApproved !== true) {
    errors.push(`owner approval is required for ${spec.action} (${spec.authority})`)
  }

  if (errors.length > 0) {
    return { ok: false, command: null, errors }
  }

  const command: DecisionQueueCommand = {
    type: "decision.resolve",
    ref,
    choice: spec.verb,
    action: spec.action,
    idempotencyKey: decisionQueueIdempotencyKey(ref, spec.action),
    authority: spec.authority,
    requiresOwnerApproval: spec.requiresOwnerApproval,
    ...(hasPayload ? { answer: payload } : {}),
  }

  return { ok: true, command, errors: [] }
}
