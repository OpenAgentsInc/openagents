import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  CodingAutopilotSituationalAwarenessAggregate,
  CodingAutopilotSituationalAwarenessProjection,
  CodingAutopilotSituationalAwarenessUnsafe,
  aggregateCodingAutopilotSituationalAwarenessRecords,
  codingAutopilotSituationalAwarenessAggregateHasPrivateMaterial,
  codingAutopilotSituationalAwarenessProjectionHasPrivateMaterial,
  exampleCodingAutopilotSituationalAwarenessRecord,
  projectCodingAutopilotSituationalAwarenessRecord,
} from './coding-autopilot-situational-awareness'

const nowIso = '2026-06-06T21:05:00.000Z'

describe('Coding on Autopilot situational awareness', () => {
  test('projects briefing metrics with Coding on Autopilot refs', () => {
    const record = exampleCodingAutopilotSituationalAwarenessRecord()
    const customerProjection = projectCodingAutopilotSituationalAwarenessRecord(
      record,
      'customer',
      nowIso,
    )
    const operatorProjection = projectCodingAutopilotSituationalAwarenessRecord(
      record,
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotSituationalAwarenessProjection)(
      customerProjection,
    )).toEqual(customerProjection)
    expect(customerProjection).toMatchObject({
      accountFailoverRefs: ['account_failover.provider_rate_limit.redacted'],
      artifactRefs: [
        'artifact.diff_summary.otec_revision_4',
        'artifact.test_run.otec_revision_4',
      ],
      comprehensionResult: 'understood',
      createdAtDisplay: '5 minutes ago',
      decisionActionRefs: [
        'decision_action.otec_revision_4.continue',
        'decision_action.otec_revision_4.retry_account',
      ],
      followUpAction: 'resumed_work',
      missingContextRefs: [],
      missionRefs: ['mission.otec_revision_4'],
      repoTrustRefs: ['repo_trust.public_repo.low'],
      reviewerKind: 'operator',
      underTwoMinuteTargetMet: true,
    })
    expect(customerProjection.metric.privateFeedbackNoteRef).toBeNull()
    expect(operatorProjection.metric.privateFeedbackNoteRef).toBe(
      'private_feedback.operator_note.redacted',
    )
    expect(codingAutopilotSituationalAwarenessProjectionHasPrivateMaterial(
      customerProjection,
    )).toBe(false)
  })

  test('redacts audience-specific refs and raw timestamps', () => {
    const publicProjection = projectCodingAutopilotSituationalAwarenessRecord({
      ...exampleCodingAutopilotSituationalAwarenessRecord(),
      artifactRefs: [
        'artifact.diff_summary.otec_revision_4',
        'artifact.private.operator_only',
      ],
      decisionActionRefs: [
        'decision_action.otec_revision_4.continue',
        'decision_action.private.route_selection',
      ],
      repoTrustRefs: [
        'repo_trust.public_repo.low',
        'repo_trust.private.customer_only',
      ],
    }, 'public', nowIso)
    const serialized = JSON.stringify(publicProjection)

    expect(publicProjection.artifactRefs).toEqual([
      'artifact.diff_summary.otec_revision_4',
    ])
    expect(publicProjection.decisionActionRefs).toEqual([
      'decision_action.otec_revision_4.continue',
    ])
    expect(publicProjection.repoTrustRefs).toEqual([
      'repo_trust.public_repo.low',
    ])
    expect(serialized).not.toContain('2026-06-06T21:00:00.000Z')
    expect(serialized).not.toContain('artifact.private')
    expect(serialized).not.toContain('decision_action.private')
    expect(serialized).not.toContain('repo_trust.private')
  })

  test('aggregates safe counts and refs using the base briefing metric model', () => {
    const records = [
      exampleCodingAutopilotSituationalAwarenessRecord(),
      {
        ...exampleCodingAutopilotSituationalAwarenessRecord(),
        accountFailoverRefs: ['account_failover.none'],
        id: 'situational_awareness.otec_revision_4.customer_1',
        metric: {
          ...exampleCodingAutopilotSituationalAwarenessRecord().metric,
          comprehensionResult: 'not_understood' as const,
          elapsedTimeBucket: 'over_2m' as const,
          followUpAction: 'asked_followup' as const,
          id: 'briefing_metric.otec_revision_4.customer_1',
          missingContextRefs: ['missing_context.latest_revision_location'],
          privateFeedbackNoteRef: null,
          reviewerKind: 'customer' as const,
        },
        metricRef: 'briefing_metric.otec_revision_4.customer_1',
      },
    ]
    const aggregate = aggregateCodingAutopilotSituationalAwarenessRecords(
      records,
      'customer',
      nowIso,
    )

    expect(S.decodeUnknownSync(CodingAutopilotSituationalAwarenessAggregate)(
      aggregate,
    )).toEqual(aggregate)
    expect(aggregate.totalCount).toBe(2)
    expect(aggregate.underTwoMinutePercent).toBe(50)
    expect(aggregate.understoodPercent).toBe(50)
    expect(aggregate.improvementNeeded).toBe(true)
    expect(aggregate.metricAggregate.notUnderstoodCount).toBe(1)
    expect(aggregate.accountFailoverRefs).toEqual([
      'account_failover.none',
      'account_failover.provider_rate_limit.redacted',
    ])
    expect(codingAutopilotSituationalAwarenessAggregateHasPrivateMaterial(
      aggregate,
    )).toBe(false)
  })

  test('rejects unsafe extra refs while preserving metric redaction behavior', () => {
    expect(() =>
      projectCodingAutopilotSituationalAwarenessRecord({
        ...exampleCodingAutopilotSituationalAwarenessRecord(),
        accountFailoverRefs: ['provider_account.codex_3'],
      }, 'operator', nowIso),
    ).toThrow(CodingAutopilotSituationalAwarenessUnsafe)
    expect(() =>
      projectCodingAutopilotSituationalAwarenessRecord({
        ...exampleCodingAutopilotSituationalAwarenessRecord(),
        artifactRefs: ['raw_runner_log.otec_revision_4'],
      }, 'customer', nowIso),
    ).toThrow(CodingAutopilotSituationalAwarenessUnsafe)
    expect(() =>
      projectCodingAutopilotSituationalAwarenessRecord({
        ...exampleCodingAutopilotSituationalAwarenessRecord(),
        missionRefs: ['mission.2026-06-06T21:00:00.000Z'],
      }, 'public', nowIso),
    ).toThrow(CodingAutopilotSituationalAwarenessUnsafe)
  })
})
