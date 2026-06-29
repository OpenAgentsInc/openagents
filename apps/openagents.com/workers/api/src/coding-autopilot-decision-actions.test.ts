import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotDecisionActionProjection,
  CodingAutopilotDecisionActionRecord,
  CodingAutopilotDecisionActionUnsafe,
  codingAutopilotDecisionActionProjectionHasPrivateMaterial,
  exampleCodingAutopilotDecisionActions,
  projectCodingAutopilotDecisionActionRecord,
} from './coding-autopilot-decision-actions'

const nowIso = '2026-06-06T21:05:00.000Z'

describe('Coding on Autopilot Decision Queue actions', () => {
  test('projects typed continue and retry-account actions without direct effects', () => {
    const [continueAction, retryAction] = exampleCodingAutopilotDecisionActions()
    const publicContinue = projectCodingAutopilotDecisionActionRecord(
      continueAction!,
      'public',
      nowIso,
    )
    const customerRetry = projectCodingAutopilotDecisionActionRecord(
      retryAction!,
      'customer',
      nowIso,
    )
    const operatorRetry = projectCodingAutopilotDecisionActionRecord(
      retryAction!,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotDecisionActionRecord)(continueAction))
      .toEqual(continueAction)
    expect(S.decodeUnknownSync(CodingAutopilotDecisionActionProjection)(publicContinue))
      .toEqual(publicContinue)
    expect(publicContinue).toMatchObject({
      actionKind: 'continue',
      actionLabel: 'Continue',
      actionSubmissionRequired: true,
      audience: 'public',
      directEffectPermitted: false,
      status: 'recommended',
      statusLabel: 'Recommended',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(customerRetry.accountLeaseRefs).toEqual([])
    expect(customerRetry.routeRefs).toEqual([])
    expect(customerRetry.sourceAuthorityRefs).toEqual([])
    expect(customerRetry.actionSubmissionRefs).toEqual([])
    expect(customerRetry.blockedReasonRefs).toEqual(['blocked.provider_rate_limit'])
    expect(operatorRetry.accountLeaseRefs).toEqual([
      'account_lease.codex_3.run_otec_revision_4',
    ])
    expect(operatorRetry.routeRefs).toEqual([
      'route_scorecard.codex_account_fleet',
    ])
    expect(operatorRetry.sourceAuthorityRefs).toEqual([
      'source_authority.account_fleet_health',
    ])
  })

  test('supports every planned Coding on Autopilot action kind and status', () => {
    const kinds = [
      'approve_pr_draft',
      'continue',
      'create_followup_mission',
      'mark_unavailable',
      'provide_context',
      'request_customer_input',
      'rerun_tests',
      'retry_account',
      'steer',
      'stop',
    ] as const
    const statuses = [
      'available',
      'blocked',
      'cancelled',
      'completed',
      'draft',
      'recommended',
    ] as const
    const projections = kinds.map((actionKind, index) =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        actionKind,
        actionRef: `decision_action.kind_${actionKind}`,
        id: `decision_action_kind_${actionKind}`,
        status: statuses[index % statuses.length]!,
      }, 'customer', nowIso),
    )

    expect(projections.map(projection => projection.actionKind)).toEqual(kinds)
    expect(projections.every(projection => projection.directEffectPermitted === false))
      .toBe(true)
    expect(projections.every(projection => projection.actionSubmissionRequired))
      .toBe(true)
  })

  test('does not expose raw timestamps in projections', () => {
    const projection = projectCodingAutopilotDecisionActionRecord(
      exampleCodingAutopilotDecisionActions()[0]!,
      'customer',
      nowIso,
    )
    const serialized = JSON.stringify(projection)

    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(serialized).not.toContain('2026-06-06T20:30:00.000Z')
    expect(codingAutopilotDecisionActionProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects provider, runner, token, private repo, and customer material', () => {
    expect(() =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        evidenceRefs: ['raw_runner_payload:mission'],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotDecisionActionUnsafe)
    expect(() =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        routeRefs: ['provider_account.codex_private'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotDecisionActionUnsafe)
    expect(() =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        prerequisiteRefs: ['private_repo:https://github.com/customer/private-repo'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotDecisionActionUnsafe)
    expect(() =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        blockedReasonRefs: ['secret:provider-token'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotDecisionActionUnsafe)
    expect(() =>
      projectCodingAutopilotDecisionActionRecord({
        ...exampleCodingAutopilotDecisionActions()[0]!,
        workroomRefs: ['ben@example.com'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotDecisionActionUnsafe)
  })
})
