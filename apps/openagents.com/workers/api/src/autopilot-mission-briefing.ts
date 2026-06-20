import type {
  AutopilotWorkEventProjection,
  AutopilotWorkOrderProjection,
} from './autopilot-work-routes'

export type AutopilotMissionBriefingDrilldownGroup = Readonly<{
  kind:
    | 'artifact'
    | 'assignment'
    | 'blocker'
    | 'build'
    | 'closeout'
    | 'preview'
    | 'proof'
    | 'result'
    | 'summary'
    | 'test'
  refs: ReadonlyArray<string>
}>

export type AutopilotMissionBriefingProjection = Readonly<{
  briefingRef: string
  costs: Readonly<{
    amountCents: number
    buyerFundingState: AutopilotWorkOrderProjection['funding']['buyerFundingState']
    currency: 'USD'
    fundedAmountCents: number
    paymentRequired: boolean
    quoteRef: string
    settlementBlockedReasonRef: string
  }>
  decisionsWaiting: Readonly<{
    callerActionRefs: ReadonlyArray<string>
    nextActionState: AutopilotWorkOrderProjection['nextAction']['state']
    reasonRefs: ReadonlyArray<string>
    reviewAction: string | null
    reviewRecordedAt: string | null
  }>
  drilldown: ReadonlyArray<AutopilotMissionBriefingDrilldownGroup>
  generatedAt: string
  kind: 'autopilot_mission_briefing'
  promiseRef: AutopilotWorkOrderProjection['promiseRef']
  publicSafe: true
  receipts: Readonly<{
    authorityReceiptRefs: ReadonlyArray<string>
    buyerPaymentProofRef: string | null
    proofRefs: ReadonlyArray<string>
    settlementEligible: AutopilotWorkOrderProjection['funding']['settlementEligible']
    verificationRefs: ReadonlyArray<string>
  }>
  risk: Readonly<{
    blockerCount: number
    changeCaptureStatus: 'blocked' | 'review_ready' | 'stale' | null
    deliveryReadinessStatus: 'blocked' | 'ready' | 'scoped_exception' | null
    level: 'attention' | 'blocked' | 'clear'
    reviewCaveatRefs: ReadonlyArray<string>
    settlementBlockedReasonRef: AutopilotWorkOrderProjection['funding']['settlementBlockedReasonRef']
    worktreeIdentityStatus: 'blocked' | 'ready' | 'stale' | null
  }>
  state: AutopilotWorkOrderProjection['state']
  whatChanged: Readonly<{
    artifactRefs: ReadonlyArray<string>
    resultRefs: ReadonlyArray<string>
    runnerKind: string | null
    summaryRefs: ReadonlyArray<string>
  }>
  whatHappened: ReadonlyArray<
    Readonly<{
      eventKind: AutopilotWorkEventProjection['eventKind']
      eventRef: string
      occurredAt: string
      sequence: number
    }>
  >
  whatIsBlocked: Readonly<{
    accessRequirementRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    placementRefusalReasonRefs: ReadonlyArray<string>
  }>
  whatIsRunning: Readonly<{
    pylonAssignmentIntentRefs: ReadonlyArray<string>
    running: boolean
    selectedRunnerKind: string | null
    taskRefs: ReadonlyArray<string>
  }>
  workOrderRef: string
}>

const drilldownGroup = (
  kind: AutopilotMissionBriefingDrilldownGroup['kind'],
  refs: ReadonlyArray<string> | undefined,
): ReadonlyArray<AutopilotMissionBriefingDrilldownGroup> =>
  refs === undefined || refs.length === 0 ? [] : [{ kind, refs }]

export const missionBriefingForWorkOrder = (
  input: Readonly<{
    events: ReadonlyArray<AutopilotWorkEventProjection>
    nowIso: string
    work: AutopilotWorkOrderProjection
  }>,
): AutopilotMissionBriefingProjection => {
  const { events, nowIso, work } = input
  const closeout = work.executionCloseout

  const reviewCaveatRefs = closeout?.reviewCaveatRefs ?? []
  const blockerRefs = closeout?.blockerRefs ?? []
  const deliveryReadinessStatus = closeout?.deliveryReadinessStatus ?? null
  const changeCaptureStatus = closeout?.changeCaptureStatus ?? null
  const worktreeIdentityStatus = closeout?.worktreeIdentityStatus ?? null
  const hasBlockedStatus =
    deliveryReadinessStatus === 'blocked' ||
    changeCaptureStatus === 'blocked' ||
    worktreeIdentityStatus === 'blocked'
  const hasAttentionStatus =
    blockerRefs.length > 0 ||
    reviewCaveatRefs.length > 0 ||
    changeCaptureStatus === 'stale' ||
    worktreeIdentityStatus === 'stale' ||
    deliveryReadinessStatus === 'scoped_exception' ||
    work.placementDecision.source === 'none_available'
  const riskLevel: AutopilotMissionBriefingProjection['risk']['level'] =
    hasBlockedStatus ? 'blocked' : hasAttentionStatus ? 'attention' : 'clear'

  return {
    briefingRef: `briefing.${work.workOrderRef}`,
    costs: {
      amountCents: work.quote.amountCents,
      buyerFundingState: work.funding.buyerFundingState,
      currency: work.quote.currency,
      fundedAmountCents: work.funding.fundedAmountCents,
      paymentRequired: work.quote.paymentRequired,
      quoteRef: work.quote.quoteRef,
      settlementBlockedReasonRef: work.funding.settlementBlockedReasonRef,
    },
    decisionsWaiting: {
      callerActionRefs: work.nextAction.callerActionRefs,
      nextActionState: work.nextAction.state,
      reasonRefs: work.nextAction.reasonRefs,
      reviewAction: work.reviewDecision?.action ?? null,
      reviewRecordedAt: work.reviewDecision?.recordedAt ?? null,
    },
    drilldown: [
      ...drilldownGroup('artifact', closeout?.artifactRefs),
      ...drilldownGroup('assignment', closeout?.assignmentRefs),
      ...drilldownGroup('blocker', closeout?.blockerRefs),
      ...drilldownGroup('build', closeout?.buildRefs),
      ...drilldownGroup('closeout', closeout?.closeoutRefs),
      ...drilldownGroup('preview', closeout?.previewRefs),
      ...drilldownGroup('proof', closeout?.proofRefs),
      ...drilldownGroup('result', closeout?.resultRefs),
      ...drilldownGroup('summary', closeout?.summaryRefs),
      ...drilldownGroup('test', closeout?.testRefs),
    ],
    generatedAt: nowIso,
    kind: 'autopilot_mission_briefing',
    promiseRef: work.promiseRef,
    publicSafe: true,
    receipts: {
      authorityReceiptRefs: closeout?.authorityReceiptRefs ?? [],
      buyerPaymentProofRef: work.buyerPaymentProofRef,
      proofRefs: closeout?.proofRefs ?? [],
      settlementEligible: work.funding.settlementEligible,
      verificationRefs: closeout?.verificationRefs ?? [],
    },
    risk: {
      blockerCount: blockerRefs.length,
      changeCaptureStatus,
      deliveryReadinessStatus,
      level: riskLevel,
      reviewCaveatRefs,
      settlementBlockedReasonRef: work.funding.settlementBlockedReasonRef,
      worktreeIdentityStatus,
    },
    state: work.state,
    whatChanged: {
      artifactRefs: closeout?.artifactRefs ?? [],
      resultRefs: closeout?.resultRefs ?? [],
      runnerKind: closeout?.runnerKind ?? null,
      summaryRefs: closeout?.summaryRefs ?? [],
    },
    whatHappened: events.map(event => ({
      eventKind: event.eventKind,
      eventRef: event.eventRef,
      occurredAt: event.occurredAt,
      sequence: event.sequence,
    })),
    whatIsBlocked: {
      accessRequirementRefs: work.accessRequirements.map(
        requirement => requirement.accessRequestRef,
      ),
      blockerRefs: closeout?.blockerRefs ?? [],
      placementRefusalReasonRefs:
        work.placementDecision.source === 'none_available'
          ? work.placementDecision.refusalReasonRefs
          : [],
    },
    whatIsRunning: {
      pylonAssignmentIntentRefs: work.pylonAssignmentIntents.map(
        intent => intent.assignmentRef,
      ),
      running: work.state === 'queued_or_running',
      selectedRunnerKind: work.placementDecision.selectedRunnerKind ?? null,
      taskRefs: work.taskRefs,
    },
    workOrderRef: work.workOrderRef,
  }
}
