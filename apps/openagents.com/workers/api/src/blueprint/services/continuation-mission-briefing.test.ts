import { Effect, Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BLUEPRINT_CONTINUATION_DECISION_FIXTURES,
} from '../fixtures/continuation-decision-fixtures'
import {
  type BlueprintContinuationDecisionQueueSource,
  BlueprintContinuationDecisionQueueProjection,
} from '../schemas/continuation-decision-queue'
import {
  BlueprintMissionBriefingProjection,
} from '../schemas/continuation-mission-briefing'
import { decideBlueprintContinuation } from './continuation-decision'
import {
  buildBlueprintContinuationDecisionQueueProjection,
} from './continuation-decision-queue'
import {
  blueprintMissionBriefingHasPrivateMaterial,
  buildBlueprintMissionBriefing,
  friendlyBlueprintMissionBriefingTime,
} from './continuation-mission-briefing'

const nowIso = '2026-06-06T02:00:00.000Z'

const emptyCustomerQueue = S.decodeUnknownSync(
  BlueprintContinuationDecisionQueueProjection,
)({
  audience: 'customer',
  blockerCount: 0,
  empty: true,
  items: [],
  pendingCount: 0,
  retryCount: 0,
  reviewCount: 0,
  stopCount: 0,
})

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

describe('Blueprint continuation Mission Briefing', () => {
  test('builds an empty customer briefing with a readable next action', () => {
    const briefing = buildBlueprintMissionBriefing({
      audience: 'customer',
      nowIso,
      queue: emptyCustomerQueue,
      updatedAtIso: '2026-06-06T01:45:00.000Z',
      workKind: 'site',
      workroomRef: 'workroom.ben_otec_site',
    })

    expect(S.decodeUnknownSync(BlueprintMissionBriefingProjection)(briefing))
      .toEqual(briefing)
    expect(briefing.empty).toBe(true)
    expect(briefing.generatedAtDisplay).toBe('15 minutes ago')
    expect(briefing.sections.nextAction).toEqual([
      {
        displayTime: null,
        kind: 'next_action',
        linkRefs: [],
        ref: 'next_action.awaiting_work',
        status: 'pending',
        summaryRef: 'next_action.awaiting_work:summary',
      },
    ])
  })

  test('renders a rich Site briefing from continuation queue and explicit refs', async () => {
    const queue = buildBlueprintContinuationDecisionQueueProjection(
      await queueSources(),
      'customer',
    )
    const briefing = buildBlueprintMissionBriefing({
      acceptanceRequestRefs: ['approval.customer_acceptance_requested'],
      audience: 'customer',
      buildRefs: ['build.site.revision_3.passed'],
      changedArtifactRefs: ['artifact.site.revision_3.diff'],
      costRefs: ['cost.autopilot.site_revision.redacted'],
      emailRefs: ['email.review_ready.sent'],
      evidenceRefs: ['evidence.public_source_cards'],
      nowIso,
      publicLinkRefs: ['https://sites.openagents.com/otec/revisions/3'],
      queue,
      routeRefs: ['route.site.fulfillment.adjutant'],
      testRefs: ['test.site.mobile_desktop_smoke.passed'],
      updatedAtIso: '2026-06-06T01:45:00.000Z',
      workKind: 'site',
      workroomRef: 'workroom.ben_otec_site',
    })

    expect(briefing.empty).toBe(false)
    expect(briefing.status).toBe('blocked')
    expect(briefing.sections.changed.map(item => item.ref)).toContain(
      'artifact.site.revision_3.diff',
    )
    expect(briefing.sections.verification.map(item => item.ref)).toEqual([
      'build.site.revision_3.passed',
      'test.site.mobile_desktop_smoke.passed',
    ])
    expect(briefing.sections.email[0]).toMatchObject({
      ref: 'email.review_ready.sent',
      status: 'sent',
    })
    expect(briefing.sections.links[0]?.ref).toBe(
      'https://sites.openagents.com/otec/revisions/3',
    )
    expect(briefing.sections.acceptanceRequest.map(item => item.ref))
      .toContain('approval.customer_or_operator_review')
    expect(briefing.sections.nextAction.map(item => item.ref)).toContain(
      'next.continuation.prepare_review',
    )
    expect(blueprintMissionBriefingHasPrivateMaterial(briefing)).toBe(false)
  })

  test('supports a coding workroom briefing with commits, tests, and route refs', () => {
    const briefing = buildBlueprintMissionBriefing({
      audience: 'team',
      buildRefs: ['build.pr.ci.passed'],
      changedArtifactRefs: ['commit.github.pr_branch_public_safe'],
      evidenceRefs: ['evidence.diff.summary'],
      nowIso,
      queue: emptyCustomerQueue,
      routeRefs: ['route.coding.pr_handoff'],
      testRefs: ['test.unit.worker_api.passed'],
      updatedAtIso: '2026-06-06T01:59:30.000Z',
      workKind: 'coding',
      workroomRef: 'workroom.customer_repo_pr',
    })

    expect(briefing.workKind).toBe('coding')
    expect(briefing.generatedAtDisplay).toBe('Just now')
    expect(briefing.sections.changed.map(item => item.ref)).toEqual([
      'commit.github.pr_branch_public_safe',
    ])
    expect(briefing.sections.route.map(item => item.ref)).toContain(
      'route.coding.pr_handoff',
    )
  })

  test('redacts unsafe refs for customer surfaces but keeps operator-safe account refs for operators', async () => {
    const sources = await queueSources()
    const customerQueue = buildBlueprintContinuationDecisionQueueProjection(
      sources,
      'customer',
    )
    const operatorQueue = buildBlueprintContinuationDecisionQueueProjection(
      sources,
      'operator',
    )
    const customerBriefing = buildBlueprintMissionBriefing({
      audience: 'customer',
      changedArtifactRefs: ['provider_account.private_detail'],
      emailRefs: ['customer_email_ben@example.com'],
      evidenceRefs: ['2026-06-06T01:45:00.000Z'],
      nowIso,
      publicLinkRefs: ['raw_run_log_private'],
      queue: customerQueue,
      updatedAtIso: '2026-06-06T01:45:00.000Z',
      workKind: 'site',
      workroomRef: 'workroom.ben_otec_site',
    })
    const operatorBriefing = buildBlueprintMissionBriefing({
      audience: 'operator',
      nowIso,
      queue: operatorQueue,
      updatedAtIso: '2026-06-06T01:45:00.000Z',
      workKind: 'site',
      workroomRef: 'workroom.ben_otec_site',
    })

    expect(blueprintMissionBriefingHasPrivateMaterial(customerBriefing)).toBe(
      false,
    )
    expect(JSON.stringify(customerBriefing)).not.toContain('provider_account')
    expect(JSON.stringify(customerBriefing)).not.toContain('@example.com')
    expect(JSON.stringify(customerBriefing)).not.toContain('2026-06-06T')
    expect(operatorBriefing.sections.blocked.map(item => item.ref)).toContain(
      'provider_account.capacity.redacted_low_credit',
    )
  })

  test('formats friendly Mission Briefing time labels deterministically', () => {
    expect(
      friendlyBlueprintMissionBriefingTime(
        '2026-06-06T01:59:00.000Z',
        nowIso,
      ),
    ).toBe('1 minute ago')
    expect(
      friendlyBlueprintMissionBriefingTime(
        '2026-06-05T01:59:00.000Z',
        nowIso,
      ),
    ).toBe('Yesterday')
  })
})
