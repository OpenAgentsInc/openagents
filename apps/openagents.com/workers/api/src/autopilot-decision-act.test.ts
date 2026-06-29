import { Schema as S } from 'effect'
import { describe, expect, it } from 'vitest'

import {
  AUTOPILOT_DECISION_ACTIONABLE_KINDS,
  AutopilotDecisionActRequest,
  type AutopilotDecisionActTarget,
  authorizeAutopilotDecisionAct,
} from './autopilot-decision-act'

const decode = (value: unknown): AutopilotDecisionActRequest =>
  S.decodeUnknownSync(AutopilotDecisionActRequest)(value)

const target = (
  overrides: Partial<AutopilotDecisionActTarget> = {},
): AutopilotDecisionActTarget => ({
  decisionRef: 'decision_action.work_order_1.continue',
  actionKind: 'continue',
  status: 'available',
  ...overrides,
})

describe('autopilot decision act request schema', () => {
  it('decodes a minimal submit request', () => {
    const request = decode({ resolution: 'continue', verb: 'submit' })
    expect(request.resolution).toBe('continue')
    expect(request.verb).toBe('submit')
    expect(request.contextRefs).toBeUndefined()
  })

  it('rejects an unknown resolution at decode time', () => {
    expect(() => decode({ resolution: 'delete_repo', verb: 'submit' })).toThrow()
  })

  it('rejects an unknown verb at decode time', () => {
    expect(() => decode({ resolution: 'stop', verb: 'maybe' })).toThrow()
  })
})

describe('authorizeAutopilotDecisionAct', () => {
  it('accepts a continue submit and stays evidence-only', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({ resolution: 'continue', verb: 'submit' }),
      target: target(),
    })
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error('expected ok')
    expect(result.command.type).toBe('autopilot.decision.act')
    expect(result.command.directEffectPermitted).toBe(false)
    expect(result.command.authorityBoundary).toBe('evidence_only')
    expect(result.command.closeoutRef).toBe(
      'decision.closeout.submit.decision_action.work_order_1.continue',
    )
  })

  it('covers every actionable decision type, not just approve_pr_draft', () => {
    for (const kind of AUTOPILOT_DECISION_ACTIONABLE_KINDS) {
      const needsContext = kind === 'provide_context' || kind === 'steer'
      const result = authorizeAutopilotDecisionAct({
        request: decode({
          resolution: kind,
          verb: 'submit',
          ...(needsContext ? { contextRefs: ['context.note.summary'] } : {}),
        }),
        target: target({
          actionKind: kind,
          decisionRef: `decision_action.work_order_1.${kind}`,
        }),
      })
      expect(result.ok).toBe(true)
    }
    expect(AUTOPILOT_DECISION_ACTIONABLE_KINDS).toContain('approve_pr_draft')
    expect(AUTOPILOT_DECISION_ACTIONABLE_KINDS).toContain(
      'create_followup_mission',
    )
    expect(AUTOPILOT_DECISION_ACTIONABLE_KINDS.length).toBeGreaterThan(1)
  })

  it('refuses an act whose resolution mismatches the stored decision kind', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({ resolution: 'stop', verb: 'submit' }),
      target: target({ actionKind: 'continue' }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error('expected rejection')
    expect(result.errors.map(error => error.code)).toContain('kind_mismatch')
  })

  it('refuses an act on a non-actionable (completed) decision', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({ resolution: 'continue', verb: 'submit' }),
      target: target({ status: 'completed' }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error('expected rejection')
    expect(result.errors.map(error => error.code)).toContain('not_actionable')
  })

  it('requires context refs when submitting provide_context', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({ resolution: 'provide_context', verb: 'submit' }),
      target: target({
        actionKind: 'provide_context',
        decisionRef: 'decision_action.work_order_1.provide_context',
      }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error('expected rejection')
    expect(result.errors.map(error => error.code)).toContain('context_required')
  })

  it('does not require context refs when declining provide_context', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({ resolution: 'provide_context', verb: 'decline' }),
      target: target({
        actionKind: 'provide_context',
        decisionRef: 'decision_action.work_order_1.provide_context',
      }),
    })
    expect(result.ok).toBe(true)
  })

  it('rejects unsafe (raw payload) context refs', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({
        resolution: 'steer',
        verb: 'submit',
        contextRefs: ['ghp_aaaaaaaaaaaaaaaaaaaa secret token'],
      }),
      target: target({
        actionKind: 'steer',
        decisionRef: 'decision_action.work_order_1.steer',
      }),
    })
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error('expected rejection')
    expect(result.errors.map(error => error.code)).toContain('unsafe_ref')
  })

  it('normalizes context refs (dedupes, trims, sorts) in the command', () => {
    const result = authorizeAutopilotDecisionAct({
      request: decode({
        resolution: 'steer',
        verb: 'submit',
        contextRefs: [' context.b ', 'context.a', 'context.b'],
      }),
      target: target({
        actionKind: 'steer',
        decisionRef: 'decision_action.work_order_1.steer',
      }),
    })
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error('expected ok')
    expect(result.command.contextRefs).toStrictEqual(['context.a', 'context.b'])
  })
})
