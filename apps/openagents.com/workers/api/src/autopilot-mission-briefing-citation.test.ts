import { describe, expect, test } from 'vitest'

import type { AutopilotMissionBriefingProjection } from './autopilot-mission-briefing'
import {
  AutopilotMissionBriefingCitationUnsafe,
  missionBriefingCitation,
  missionCitesBriefing,
} from './autopilot-mission-briefing-citation'
import { CodingAutopilotMissionRecord } from './coding-autopilot-missions'

const nowIso = '2026-06-20T12:00:00.000Z'
const workOrderRef = 'autopilot_work_order.otec_revision_4'
const briefingRef = `briefing.${workOrderRef}`

const deliveredBriefing = (): AutopilotMissionBriefingProjection => ({
  briefingRef,
  costs: {
    amountCents: 0,
    buyerFundingState: 'not_required',
    currency: 'USD',
    fundedAmountCents: 0,
    paymentRequired: false,
    quoteRef: 'quote.public.otec_revision_4',
    settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
  },
  decisionsWaiting: {
    callerActionRefs: ['caller_action.review_delivered_work'],
    nextActionState: 'delivered',
    reasonRefs: ['next_action.review_delivered_work'],
    reviewAction: null,
    reviewRecordedAt: null,
  },
  drilldown: [
    { kind: 'artifact', refs: ['artifact.public.otec_revision_4.patch_summary'] },
    { kind: 'proof', refs: ['proof.public.otec_revision_4.worker_closeout'] },
  ],
  generatedAt: nowIso,
  kind: 'autopilot_mission_briefing',
  promiseRef: {
    blockerRefs: [
      'blocker.product_promises.mission_briefing_live_mission_citation_missing',
    ],
    promiseId: 'autopilot.mission_briefing.v1',
    registryVersion: '2026-06-19.8',
  },
  publicSafe: true,
  receipts: {
    authorityReceiptRefs: ['authority.public.otec_revision_4.writeback_ready'],
    buyerPaymentProofRef: null,
    proofRefs: ['proof.public.otec_revision_4.worker_closeout'],
    settlementEligible: false,
    verificationRefs: ['verification.public.otec_revision_4.bun_test'],
  },
  risk: {
    blockerCount: 0,
    changeCaptureStatus: 'review_ready',
    deliveryReadinessStatus: 'ready',
    level: 'attention',
    reviewCaveatRefs: ['review-caveat.public.otec_revision_4.summary_only'],
    settlementBlockedReasonRef: 'settlement.no_worker_payout_mode',
    worktreeIdentityStatus: 'ready',
  },
  state: 'delivered',
  whatChanged: {
    artifactRefs: ['artifact.public.otec_revision_4.patch_summary'],
    resultRefs: ['result.public.otec_revision_4.delivered'],
    runnerKind: 'requester_pylon',
    summaryRefs: ['summary.public.otec_revision_4.customer_safe'],
  },
  whatHappened: [
    {
      eventKind: 'queued',
      eventRef: 'event.public.otec_revision_4.queued',
      occurredAt: '2026-06-20T11:00:00.000Z',
      sequence: 1,
    },
    {
      eventKind: 'delivered',
      eventRef: 'event.public.otec_revision_4.delivered',
      occurredAt: '2026-06-20T11:58:00.000Z',
      sequence: 2,
    },
  ],
  whatIsBlocked: {
    accessRequirementRefs: [],
    blockerRefs: [],
    placementRefusalReasonRefs: [],
  },
  whatIsRunning: {
    pylonAssignmentIntentRefs: [],
    running: false,
    selectedRunnerKind: 'requester_pylon',
    taskRefs: ['task.public.otec_revision_4.site_build'],
  },
  workOrderRef,
})

const missionRecord = (
  overrides: Partial<CodingAutopilotMissionRecord> = {},
): CodingAutopilotMissionRecord =>
  new CodingAutopilotMissionRecord({
    accountLeaseRefs: [],
    artifactRefs: ['artifact.public.otec_revision_4.patch_summary'],
    assignmentRefs: [],
    blockerRefs: [],
    budgetRefs: [],
    createdAtIso: '2026-06-20T10:00:00.000Z',
    customerRefs: ['customer_ref.order_otec'],
    id: 'coding_mission.otec_revision_4',
    latestBriefingRef: briefingRef,
    missionRef: 'mission.otec_revision_4',
    nextOrderRefs: [],
    objectiveStackRefs: ['objective.public.otec_revision_4'],
    ownerRefs: [],
    routeScorecardRefs: [],
    status: 'waiting_for_review',
    teamRefs: [],
    updatedAtIso: '2026-06-20T11:58:00.000Z',
    workKind: 'site',
    workroomRefs: [],
    ...overrides,
  })

describe('Autopilot Mission Briefing citation', () => {
  test('links a live mission to the briefing JSON it cites', () => {
    const briefing = deliveredBriefing()
    const mission = missionRecord()

    expect(missionCitesBriefing(mission, briefing)).toBe(true)

    const citation = missionBriefingCitation({ briefing, mission, nowIso })

    expect(citation).toEqual({
      briefingCitedByMission: true,
      briefingRef,
      citationRef: `citation.mission.otec_revision_4.${briefingRef}`,
      citedBriefingRef: briefingRef,
      decisionNeeded: true,
      decisionReasonRefs: ['next_action.review_delivered_work'],
      generatedAt: nowIso,
      kind: 'autopilot_mission_briefing_citation',
      missionRef: 'mission.otec_revision_4',
      missionStatus: 'waiting_for_review',
      nextActionState: 'delivered',
      proofRefs: ['proof.public.otec_revision_4.worker_closeout'],
      publicSafe: true,
      riskLevel: 'attention',
      state: 'delivered',
      verificationRefs: ['verification.public.otec_revision_4.bun_test'],
      workOrderRef,
    })
  })

  test('flags a mission that cites a different briefing', () => {
    const briefing = deliveredBriefing()
    const mission = missionRecord({
      latestBriefingRef: 'briefing.autopilot_work_order.some_other_order',
    })

    expect(missionCitesBriefing(mission, briefing)).toBe(false)

    const citation = missionBriefingCitation({ briefing, mission, nowIso })

    expect(citation.briefingCitedByMission).toBe(false)
    expect(citation.citedBriefingRef).toBe(
      'briefing.autopilot_work_order.some_other_order',
    )
    expect(citation.briefingRef).toBe(briefingRef)
  })

  test('marks decisionNeeded false for an automated retry_later state', () => {
    const base = deliveredBriefing()
    const briefing: AutopilotMissionBriefingProjection = {
      ...base,
      decisionsWaiting: {
        ...base.decisionsWaiting,
        nextActionState: 'retry_later',
        reasonRefs: ['next_action.retry_later'],
      },
      state: 'queued_or_running',
    }

    const citation = missionBriefingCitation({
      briefing,
      mission: missionRecord({ status: 'running' }),
      nowIso,
    })

    expect(citation.decisionNeeded).toBe(false)
    expect(citation.nextActionState).toBe('retry_later')
  })

  test('does not leak private material and refuses unsafe proof refs', () => {
    const base = deliveredBriefing()
    const safeCitation = missionBriefingCitation({
      briefing: base,
      mission: missionRecord(),
      nowIso,
    })
    expect(JSON.stringify(safeCitation)).not.toMatch(
      /mnemonic|invoice|preimage|secret|token|wallet|\/Users\//i,
    )

    const unsafeBriefing: AutopilotMissionBriefingProjection = {
      ...base,
      receipts: {
        ...base.receipts,
        proofRefs: ['proof.private.lnbc_invoice_preimage'],
      },
    }
    expect(() =>
      missionBriefingCitation({
        briefing: unsafeBriefing,
        mission: missionRecord(),
        nowIso,
      }),
    ).toThrow(AutopilotMissionBriefingCitationUnsafe)
  })
})
