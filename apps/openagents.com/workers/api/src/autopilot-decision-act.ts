import { Schema as S } from 'effect'

import type {
  CodingAutopilotDecisionActionKind,
  CodingAutopilotDecisionActionStatus,
} from './coding-autopilot-decision-actions'

// Pure request -> command authorization for the autopilot decision queue.
//
// The HTTP queue surface (autopilot-decision-routes.ts) only resolves the
// `approve_pr_draft` review decision today; every other decision type the node
// can raise (continue / steer / provide context / rerun tests / retry account /
// stop) has no validated, route-authorized way in. This module is that missing
// contract: it turns a client-submitted act into a typed, evidence-only command
// the route/store can apply, OR a typed rejection a card renders without retry.
//
// Two invariants are enforced here, not at the call site:
//   1. ROUTE-AUTHORIZED — an act is only accepted against a decision whose
//      stored status is actually actionable (available / recommended). A
//      completed / cancelled / blocked / draft decision is refused.
//   2. EVIDENCE-ONLY — resolving a decision never carries a direct effect.
//      The produced command always reports `directEffectPermitted: false` and
//      `authorityBoundary: 'evidence_only'`; the runtime, not this queue,
//      performs any side effect, gated on its own capability.

// The decision types the queue can resolve beyond the legacy review approval.
// Subset of CodingAutopilotDecisionActionKind that a client may act on.
export const AUTOPILOT_DECISION_ACTIONABLE_KINDS = [
  'approve_pr_draft',
  'continue',
  'create_followup_mission',
  'provide_context',
  'rerun_tests',
  'retry_account',
  'steer',
  'stop',
] as const satisfies ReadonlyArray<CodingAutopilotDecisionActionKind>

export type AutopilotDecisionActionableKind =
  (typeof AUTOPILOT_DECISION_ACTIONABLE_KINDS)[number]

const ACTIONABLE_KIND_SET: ReadonlySet<string> = new Set(
  AUTOPILOT_DECISION_ACTIONABLE_KINDS,
)

// Statuses a decision must be in for an act to be accepted.
const ACTIONABLE_STATUSES = [
  'available',
  'recommended',
] as const satisfies ReadonlyArray<CodingAutopilotDecisionActionStatus>

const ACTIONABLE_STATUS_SET: ReadonlySet<string> = new Set(ACTIONABLE_STATUSES)

// Public-safe ref shape shared with the decision routes: no raw payloads,
// secrets, or free text ever reach the store via the queue — only refs.
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{2,240}$/

export const AutopilotDecisionActVerb = S.Literals(['submit', 'decline'])
export type AutopilotDecisionActVerb = typeof AutopilotDecisionActVerb.Type

export const AutopilotDecisionActResolution = S.Literals(
  AUTOPILOT_DECISION_ACTIONABLE_KINDS,
)
export type AutopilotDecisionActResolution =
  typeof AutopilotDecisionActResolution.Type

// The wire request a client sends to resolve one queued decision.
export const AutopilotDecisionActRequest = S.Struct({
  // The decision type being resolved; must match the stored decision's kind.
  resolution: AutopilotDecisionActResolution,
  // submit = enact the decision; decline = dismiss it (still a closeout).
  verb: AutopilotDecisionActVerb,
  // Public-safe refs the client attaches (required for context/steer submits).
  contextRefs: S.optionalKey(S.Array(S.String)),
})
export type AutopilotDecisionActRequest = typeof AutopilotDecisionActRequest.Type

// Minimal stored-decision facts the authorizer needs (route reads these from
// the persisted record before calling — no store dependency leaks in here).
export type AutopilotDecisionActTarget = Readonly<{
  decisionRef: string
  actionKind: CodingAutopilotDecisionActionKind
  status: CodingAutopilotDecisionActionStatus
}>

export type AutopilotDecisionActErrorCode =
  | 'unknown_resolution'
  | 'kind_mismatch'
  | 'not_actionable'
  | 'context_required'
  | 'unsafe_ref'

export type AutopilotDecisionActError = Readonly<{
  code: AutopilotDecisionActErrorCode
  reason: string
}>

// The route-authorized, evidence-only command the store applies. The closeout
// ref is the exactly-once key a receipt is later attributed to.
export type AutopilotDecisionActCommand = Readonly<{
  type: 'autopilot.decision.act'
  decisionRef: string
  resolution: AutopilotDecisionActionableKind
  verb: AutopilotDecisionActVerb
  contextRefs: ReadonlyArray<string>
  closeoutRef: string
  directEffectPermitted: false
  authorityBoundary: 'evidence_only'
}>

export type AutopilotDecisionActAuthorization =
  | Readonly<{ ok: true; command: AutopilotDecisionActCommand }>
  | Readonly<{ ok: false; errors: ReadonlyArray<AutopilotDecisionActError> }>

// Kinds that require the client to attach at least one context ref on submit.
const CONTEXT_BEARING_KINDS: ReadonlySet<AutopilotDecisionActionableKind> =
  new Set(['provide_context', 'steer'])

const normalizeRefs = (
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<string> =>
  [
    ...new Set(
      (refs ?? []).map(ref => ref.trim()).filter(ref => ref !== ''),
    ),
  ].sort()

// Authorize a decode-validated act against the stored decision facts. Pure: no
// I/O, no time, no store. Returns a single typed command or all rejections.
export const authorizeAutopilotDecisionAct = (input: {
  request: AutopilotDecisionActRequest
  target: AutopilotDecisionActTarget
}): AutopilotDecisionActAuthorization => {
  const { request, target } = input
  const errors: AutopilotDecisionActError[] = []

  if (!ACTIONABLE_KIND_SET.has(request.resolution)) {
    errors.push({
      code: 'unknown_resolution',
      reason: `Resolution ${request.resolution} is not an actionable decision type.`,
    })
  }

  if (request.resolution !== target.actionKind) {
    errors.push({
      code: 'kind_mismatch',
      reason: `Resolution ${request.resolution} does not match decision kind ${target.actionKind}.`,
    })
  }

  if (!ACTIONABLE_STATUS_SET.has(target.status)) {
    errors.push({
      code: 'not_actionable',
      reason: `Decision ${target.decisionRef} is ${target.status}; only available or recommended decisions can be resolved.`,
    })
  }

  const contextRefs = normalizeRefs(request.contextRefs)

  if (
    request.verb === 'submit' &&
    CONTEXT_BEARING_KINDS.has(request.resolution) &&
    contextRefs.length === 0
  ) {
    errors.push({
      code: 'context_required',
      reason: `Resolution ${request.resolution} requires at least one context ref on submit.`,
    })
  }

  const unsafeRef = contextRefs.find(ref => !safeRefPattern.test(ref))
  if (unsafeRef !== undefined) {
    errors.push({
      code: 'unsafe_ref',
      reason: 'Decision act context refs must be public-safe refs, not raw payloads.',
    })
  }

  if (errors.length > 0) {
    return { ok: false, errors }
  }

  return {
    ok: true,
    command: {
      type: 'autopilot.decision.act',
      decisionRef: target.decisionRef,
      resolution: request.resolution,
      verb: request.verb,
      contextRefs,
      closeoutRef: `decision.closeout.${request.verb}.${target.decisionRef}`,
      directEffectPermitted: false,
      authorityBoundary: 'evidence_only',
    },
  }
}
