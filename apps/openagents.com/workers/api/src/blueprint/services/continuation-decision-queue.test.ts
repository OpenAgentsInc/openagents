import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_CONTINUATION_DECISION_FIXTURES,
} from '../fixtures/continuation-decision-fixtures'
import {
  BlueprintContinuationDecisionQueueProjection as BlueprintContinuationDecisionQueueProjectionSchema,
  type BlueprintContinuationDecisionQueueSource,
} from '../schemas/continuation-decision-queue'
import { decideBlueprintContinuation } from './continuation-decision'
import {
  blueprintContinuationDecisionQueueProjectionHasCustomerPrivateMaterial,
  buildBlueprintContinuationDecisionQueueProjection,
} from './continuation-decision-queue'

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

describe('Blueprint continuation Decision Queue projection', () => {
  test('projects fixture-backed operator queue items with next orders and refs', async () => {
    const projection = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'operator',
    )

    expect(
      S.decodeUnknownSync(BlueprintContinuationDecisionQueueProjectionSchema)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.empty).toBe(false)
    expect(projection.items).toHaveLength(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.length,
    )
    expect(projection.items.map(item => item.action)).toStrictEqual(
      BLUEPRINT_CONTINUATION_DECISION_FIXTURES.map(
        fixture => fixture.expectedDecision,
      ),
    )
    expect(
      projection.items.every(
        item =>
          item.recommendedNextOrderRef.startsWith('next.continuation.') &&
          item.evidenceRefs.length > 0 &&
          item.receiptRefs.length > 0 &&
          item.programSignatureId.startsWith('program_signature.autopilot.'),
      ),
    ).toBe(true)
  })

  test('preserves blockers, approvals, retries, account-failover, and stop conditions', async () => {
    const projection = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'operator',
    )
    const byAction = new Map(projection.items.map(item => [item.action, item]))

    expect(byAction.get('fix')?.blockerRefs).toContain(
      'failure.site_builder.redacted_build_error',
    )
    expect(byAction.get('request_context')?.blockerRefs).toContain(
      'context.customer_visual_assets_needed',
    )
    expect(byAction.get('retry_account')?.accountFailoverNeeded).toBe(true)
    expect(byAction.get('retry_account')?.accountFailoverRefs).toContain(
      'provider_account.capacity.redacted_low_credit',
    )
    expect(byAction.get('retry_account')?.retryRefs).toContain(
      'provider_account.capacity.redacted_low_credit',
    )
    expect(byAction.get('prepare_review')?.approvalRefs).toContain(
      'approval.customer_or_operator_review',
    )
    expect(byAction.get('escalate')?.approvalRefs).toContain(
      'approval.operator_attention',
    )
    expect(byAction.get('stop')?.stopConditionRefs).toContain(
      'stop.user_or_policy_requested',
    )
  })

  test('customer projection keeps queue visibility but redacts account and source details', async () => {
    const projection = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'customer',
    )
    const retry = projection.items.find(item => item.action === 'retry_account')

    expect(retry?.accountFailoverNeeded).toBe(true)
    expect(retry?.accountFailoverRefs).toEqual([])
    expect(retry?.retryRefs).toEqual(['retry.account_failover_needed'])
    expect(projection.items.every(item => item.sourceAuthorityRefs.length === 0))
      .toBe(true)
    expect(
      blueprintContinuationDecisionQueueProjectionHasCustomerPrivateMaterial(
        projection,
      ),
    ).toBe(false)
  })

  test('detects customer-private material in queue projections', async () => {
    const projection = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'customer',
    )

    expect(
      blueprintContinuationDecisionQueueProjectionHasCustomerPrivateMaterial({
        ...projection,
        items: [
          {
            ...projection.items[0]!,
            sourceAuthorityRefs: ['provider_account.private_detail'],
          },
        ],
      }),
    ).toBe(true)
  })
})
