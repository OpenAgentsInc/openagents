import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_CONTINUATION_DECISION_FIXTURES,
} from '../fixtures/continuation-decision-fixtures'
import type {
  BlueprintContinuationDecisionQueueSource,
} from '../schemas/continuation-decision-queue'
import {
  BlueprintMissionBriefingMetricAggregate,
  BlueprintMissionBriefingMetricProjection,
  BlueprintMissionBriefingMetricRecord,
} from '../schemas/mission-briefing-metric'
import { decideBlueprintContinuation } from './continuation-decision'
import {
  buildBlueprintContinuationDecisionQueueProjection,
} from './continuation-decision-queue'
import { buildBlueprintMissionBriefing } from './continuation-mission-briefing'
import {
  aggregateBlueprintMissionBriefingMetrics,
  blueprintMissionBriefingMetricMetTwoMinuteTarget,
  blueprintMissionBriefingMetricProjectionHasPrivateMaterial,
  projectBlueprintMissionBriefingMetric,
} from './mission-briefing-metric'

const nowIso = '2026-06-06T03:00:00.000Z'

const queueSources = async (): Promise<
  ReadonlyArray<BlueprintContinuationDecisionQueueSource>
> => {
  const decisions = await Promise.all(
    BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(fixture =>
      Effect.runPromise(decideBlueprintContinuation(fixture.turnResult)),
    ),
  )

  return BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map((fixture, index) => ({
    decision: decisions[index]!,
    orderRefs: [`order.${fixture.id}`],
    programRunRef: `program_run.${fixture.id}`,
    safeSummaryRef: fixture.publicSafeSummaryRef,
    siteRefs:
      fixture.turnResult.workRef === 'workroom.ben_otec_site'
        ? ['site.otec']
        : [],
    turnResult: fixture.turnResult,
    workroomRefs: [fixture.turnResult.workRef],
  }))
}

const metricRecord = (
  overrides: Partial<BlueprintMissionBriefingMetricRecord> = {},
): BlueprintMissionBriefingMetricRecord =>
  S.decodeUnknownSync(BlueprintMissionBriefingMetricRecord)({
    briefingRef: 'briefing.continuation.ben_otec.v1',
    comprehensionResult: 'understood',
    createdAt: nowIso,
    elapsedTimeBucket: 'under_2m',
    feedbackSummaryRef: 'feedback_summary.mission_briefing.clear',
    followUpAction: 'resumed_work',
    id: 'briefing_metric.ben_otec.1',
    missingContextRefs: [],
    privateFeedbackNoteRef: 'private_feedback.operator_note.redacted',
    programRunRef: 'program_run.ben_otec_revision_ready_for_review',
    receiptRefs: ['receipt.briefing_metric.ben_otec.1'],
    reviewerKind: 'operator',
    scorecardRefs: ['scorecard.briefing.two_minute_state_understanding'],
    workroomRef: 'workroom.ben_otec_site',
    ...overrides,
  })

describe('Blueprint Mission Briefing metric', () => {
  test('projects fixture-backed briefing review feedback safely', async () => {
    const queue = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'customer',
    )
    const briefing = buildBlueprintMissionBriefing({
      audience: 'customer',
      nowIso,
      queue,
      updatedAtIso: '2026-06-06T02:45:00.000Z',
      workKind: 'site',
      workroomRef: 'workroom.ben_otec_site',
    })
    const projection = projectBlueprintMissionBriefingMetric(
      metricRecord({
        briefingRef: `${briefing.workroomRef}.mission_briefing.latest`,
      }),
      'customer',
    )

    expect(S.decodeUnknownSync(BlueprintMissionBriefingMetricProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.briefingRef).toBe(
      'workroom.ben_otec_site.mission_briefing.latest',
    )
    expect(projection.privateFeedbackNoteRef).toBeNull()
    expect(projection.underTwoMinuteTargetMet).toBe(true)
    expect(blueprintMissionBriefingMetricProjectionHasPrivateMaterial(
      projection,
    )).toBe(false)
  })

  test('keeps raw private notes out of public projections and operator notes redacted', () => {
    const publicProjection = projectBlueprintMissionBriefingMetric(
      metricRecord({
        feedbackSummaryRef: 'customer_email_ben@example.com',
        missingContextRefs: ['provider_account.private_detail'],
        privateFeedbackNoteRef: 'raw_email_body_customer@example.com',
        receiptRefs: ['receipt.safe', '2026-06-06T02:45:00.000Z'],
      }),
      'public',
    )
    const operatorProjection = projectBlueprintMissionBriefingMetric(
      metricRecord(),
      'operator',
    )

    expect(publicProjection.feedbackSummaryRef).toBeNull()
    expect(publicProjection.privateFeedbackNoteRef).toBeNull()
    expect(publicProjection.missingContextRefs).toEqual([])
    expect(publicProjection.receiptRefs).toEqual(['receipt.safe'])
    expect(JSON.stringify(publicProjection)).not.toContain('@example.com')
    expect(JSON.stringify(publicProjection)).not.toContain('2026-06-06T')
    expect(operatorProjection.privateFeedbackNoteRef).toBe(
      'private_feedback.operator_note.redacted',
    )
  })

  test('aggregates safe comprehension and elapsed-time counts', () => {
    const metrics = [
      metricRecord({
        elapsedTimeBucket: 'under_30s',
        followUpAction: 'accepted',
        reviewerKind: 'customer',
      }),
      metricRecord({
        comprehensionResult: 'partially_understood',
        elapsedTimeBucket: 'over_2m',
        followUpAction: 'asked_followup',
        id: 'briefing_metric.ben_otec.2',
        reviewerKind: 'operator',
      }),
      metricRecord({
        comprehensionResult: 'not_understood',
        elapsedTimeBucket: 'not_understood',
        followUpAction: 'escalated',
        id: 'briefing_metric.ben_otec.3',
        reviewerKind: 'team',
      }),
    ]
    const aggregate = aggregateBlueprintMissionBriefingMetrics(
      metrics,
      'customer',
    )

    expect(S.decodeUnknownSync(BlueprintMissionBriefingMetricAggregate)(
      aggregate,
    )).toEqual(aggregate)
    expect(aggregate.totalCount).toBe(3)
    expect(aggregate.underTwoMinuteCount).toBe(1)
    expect(aggregate.understoodCount).toBe(1)
    expect(aggregate.partialCount).toBe(1)
    expect(aggregate.notUnderstoodCount).toBe(1)
    expect(aggregate.underTwoMinutePercent).toBe(33)
    expect(aggregate.understoodPercent).toBe(33)
    expect(aggregate.improvementNeeded).toBe(true)
    expect(aggregate.byFollowUpAction).toEqual([
      { count: 1, key: 'accepted' },
      { count: 1, key: 'asked_followup' },
      { count: 1, key: 'escalated' },
    ])
  })

  test('classifies the two-minute target buckets', () => {
    expect(
      blueprintMissionBriefingMetricMetTwoMinuteTarget(
        metricRecord({ elapsedTimeBucket: 'under_1m' }),
      ),
    ).toBe(true)
    expect(
      blueprintMissionBriefingMetricMetTwoMinuteTarget(
        metricRecord({ elapsedTimeBucket: 'over_2m' }),
      ),
    ).toBe(false)
  })
})
