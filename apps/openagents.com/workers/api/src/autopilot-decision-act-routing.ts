import {
  AUTOPILOT_DECISION_ACTIONABLE_KINDS,
  type AutopilotDecisionActionableKind,
  type AutopilotDecisionActTarget,
} from './autopilot-decision-act'
import type {
  CodingAutopilotDecisionActionKind,
  CodingAutopilotDecisionActionProjection,
  CodingAutopilotDecisionActionStatus,
} from './coding-autopilot-decision-actions'

// Route-side classification for the autopilot decision queue.
//
// `autopilot-decision-routes.ts#actOnDecision` today hard-codes ONE path: it
// only resolves `approve_pr_draft` through the work-order review store and
// rejects every other decision kind ("Only approve_pr_draft decision actions
// are actionable through the decision queue."). The pure act contract
// (`authorizeAutopilotDecisionAct`) already accepts the full vocabulary, but
// the route has no logic to decide WHICH handler a given stored decision
// should flow to. That branch decision is this module — pure, testable, and
// independent of the store/transport so the route can be wired without
// re-deriving it.
//
// Three mutually-exclusive routes:
//   - `work_order_review`  → the legacy PR-approval path (recordReviewDecision).
//   - `evidence_command`   → the full-vocabulary path; carries the
//                            AutopilotDecisionActTarget the authorizer needs.
//   - `not_actionable`     → the decision is informational/blocked and must NOT
//                            be acted on through the queue (e.g. a blocked
//                            customer-input prompt, a follow-up mission notice).
//
// Routing is decided by the decision KIND only; status (available / blocked /
// completed / …) is carried on the target so `authorizeAutopilotDecisionAct`
// remains the single place that enforces actionability. Keeping the two
// concerns apart means a recommended-then-completed decision routes
// consistently and the authorizer alone owns the "too late" refusal.

// The single review-routed kind: it has a dedicated work-order review store
// and predates the evidence-only command path.
export const AUTOPILOT_DECISION_REVIEW_KIND = 'approve_pr_draft' as const

export type AutopilotDecisionActRouteKind =
  | 'work_order_review'
  | 'evidence_command'
  | 'not_actionable'

export type AutopilotDecisionActRouting =
  | Readonly<{ route: 'work_order_review' }>
  | Readonly<{ route: 'evidence_command'; target: AutopilotDecisionActTarget }>
  | Readonly<{ route: 'not_actionable'; reason: string }>

const EVIDENCE_COMMAND_KINDS: ReadonlySet<AutopilotDecisionActionableKind> =
  new Set(
    AUTOPILOT_DECISION_ACTIONABLE_KINDS.filter(
      kind => kind !== AUTOPILOT_DECISION_REVIEW_KIND,
    ),
  )

const isEvidenceCommandKind = (
  kind: CodingAutopilotDecisionActionKind,
): kind is AutopilotDecisionActionableKind =>
  EVIDENCE_COMMAND_KINDS.has(kind as AutopilotDecisionActionableKind)

// `true` when this decision must be resolved through the work-order review
// store rather than the evidence-only command path.
export const isWorkOrderReviewDecision = (
  kind: CodingAutopilotDecisionActionKind,
): boolean => kind === AUTOPILOT_DECISION_REVIEW_KIND

// Decide how the route should handle an act against a stored decision
// projection (the public-safe shape the queue actually serves). Pure: no I/O,
// no store, no time. The caller still passes the resulting target through
// `authorizeAutopilotDecisionAct`, which owns status/kind-mismatch enforcement.
export const classifyAutopilotDecisionActRoute = (
  projection: Readonly<{
    actionKind: CodingAutopilotDecisionActionKind
    actionRef: string
    status: CodingAutopilotDecisionActionStatus
  }>,
): AutopilotDecisionActRouting => {
  if (projection.actionKind === AUTOPILOT_DECISION_REVIEW_KIND) {
    return { route: 'work_order_review' }
  }

  if (isEvidenceCommandKind(projection.actionKind)) {
    return {
      route: 'evidence_command',
      target: {
        decisionRef: projection.actionRef,
        actionKind: projection.actionKind,
        status: projection.status,
      },
    }
  }

  return {
    route: 'not_actionable',
    reason: `Decision kind ${projection.actionKind} is informational and cannot be resolved through the decision queue.`,
  }
}

// Convenience over the full projection (what the queue list serves).
export const classifyAutopilotDecisionProjection = (
  projection: CodingAutopilotDecisionActionProjection,
): AutopilotDecisionActRouting =>
  classifyAutopilotDecisionActRoute({
    actionKind: projection.actionKind,
    actionRef: projection.actionRef,
    status: projection.status,
  })
