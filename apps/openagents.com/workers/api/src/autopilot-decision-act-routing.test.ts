import { describe, expect, it } from 'vitest'

import {
  AUTOPILOT_DECISION_REVIEW_KIND,
  classifyAutopilotDecisionActRoute,
  classifyAutopilotDecisionProjection,
  isWorkOrderReviewDecision,
} from './autopilot-decision-act-routing'
import { authorizeAutopilotDecisionAct } from './autopilot-decision-act'
import type {
  CodingAutopilotDecisionActionKind,
  CodingAutopilotDecisionActionProjection,
  CodingAutopilotDecisionActionStatus,
} from './coding-autopilot-decision-actions'

const facts = (
  actionKind: CodingAutopilotDecisionActionKind,
  status: CodingAutopilotDecisionActionStatus = 'available',
): Readonly<{
  actionKind: CodingAutopilotDecisionActionKind
  actionRef: string
  status: CodingAutopilotDecisionActionStatus
}> => ({
  actionKind,
  actionRef: `decision_action.work_order_1.${actionKind}`,
  status,
})

const projection = (
  actionKind: CodingAutopilotDecisionActionKind,
  status: CodingAutopilotDecisionActionStatus = 'available',
): CodingAutopilotDecisionActionProjection =>
  ({
    accountLeaseRefs: [],
    actionKind,
    actionLabel: actionKind,
    actionRef: `decision_action.work_order_1.${actionKind}`,
    actionSubmissionRefs: [],
    actionSubmissionRequired: true,
    assignmentRefs: [],
    audience: 'customer',
    blockedReasonRefs: [],
    createdAtDisplay: 'just now',
    customerNextActionRef: 'next_action.x',
    directEffectPermitted: false,
    evidenceRefs: [],
    id: `decision_action_work_order_1_${actionKind}`,
    missionRef: 'mission.work_order_1',
    prerequisiteRefs: [],
    programRunRef: null,
    receiptRefs: [],
    routeRefs: [],
    safeSummaryRef: 'summary.x',
    sourceAuthorityRefs: [],
    status,
    statusLabel: status,
    updatedAtDisplay: 'just now',
    workroomRefs: [],
  }) as CodingAutopilotDecisionActionProjection

describe('isWorkOrderReviewDecision', () => {
  it('flags approve_pr_draft as review-routed and others as not', () => {
    expect(isWorkOrderReviewDecision('approve_pr_draft')).toBe(true)
    expect(isWorkOrderReviewDecision('continue')).toBe(false)
    expect(isWorkOrderReviewDecision('request_customer_input')).toBe(false)
  })
})

describe('classifyAutopilotDecisionActRoute', () => {
  it('routes approve_pr_draft to the work-order review store', () => {
    const routing = classifyAutopilotDecisionActRoute(facts('approve_pr_draft'))
    expect(routing.route).toBe('work_order_review')
    expect(AUTOPILOT_DECISION_REVIEW_KIND).toBe('approve_pr_draft')
  })

  it('routes every other actionable kind to the evidence-command path', () => {
    for (const kind of [
      'continue',
      'create_followup_mission',
      'provide_context',
      'rerun_tests',
      'retry_account',
      'steer',
      'stop',
    ] as const) {
      const routing = classifyAutopilotDecisionActRoute(facts(kind))
      expect(routing.route).toBe('evidence_command')
      if (routing.route === 'evidence_command') {
        expect(routing.target.actionKind).toBe(kind)
        expect(routing.target.decisionRef).toBe(
          `decision_action.work_order_1.${kind}`,
        )
        expect(routing.target.status).toBe('available')
      }
    }
  })

  it('marks informational/blocked kinds as not actionable', () => {
    for (const kind of [
      'mark_unavailable',
      'request_customer_input',
    ] as const) {
      const routing = classifyAutopilotDecisionActRoute(facts(kind))
      expect(routing.route).toBe('not_actionable')
      if (routing.route === 'not_actionable') {
        expect(routing.reason).toContain(kind)
      }
    }
  })

  it('routes by kind alone, carrying status for the authorizer to enforce', () => {
    const routing = classifyAutopilotDecisionActRoute(
      facts('continue', 'completed'),
    )
    expect(routing.route).toBe('evidence_command')
    if (routing.route === 'evidence_command') {
      expect(routing.target.status).toBe('completed')
    }
  })

  it('produces a target the authorizer accepts for an actionable decision', () => {
    const routing = classifyAutopilotDecisionActRoute(facts('steer'))
    expect(routing.route).toBe('evidence_command')
    if (routing.route !== 'evidence_command') return
    const authorized = authorizeAutopilotDecisionAct({
      request: {
        resolution: 'steer',
        verb: 'submit',
        contextRefs: ['context.redirect_to_login_flow'],
      },
      target: routing.target,
    })
    expect(authorized.ok).toBe(true)
    if (authorized.ok) {
      expect(authorized.command.authorityBoundary).toBe('evidence_only')
      expect(authorized.command.closeoutRef).toBe(
        'decision.closeout.submit.decision_action.work_order_1.steer',
      )
    }
  })

  it('produces a target the authorizer refuses once the decision is completed', () => {
    const routing = classifyAutopilotDecisionActRoute(
      facts('continue', 'completed'),
    )
    expect(routing.route).toBe('evidence_command')
    if (routing.route !== 'evidence_command') return
    const authorized = authorizeAutopilotDecisionAct({
      request: { resolution: 'continue', verb: 'submit' },
      target: routing.target,
    })
    expect(authorized.ok).toBe(false)
    if (!authorized.ok) {
      expect(authorized.errors.some(e => e.code === 'not_actionable')).toBe(true)
    }
  })

  it('accepts a full queue projection via the convenience wrapper', () => {
    const routing = classifyAutopilotDecisionProjection(projection('rerun_tests'))
    expect(routing.route).toBe('evidence_command')
    if (routing.route === 'evidence_command') {
      expect(routing.target.actionKind).toBe('rerun_tests')
    }
  })
})
