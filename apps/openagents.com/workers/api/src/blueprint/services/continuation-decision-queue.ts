import { Match as M } from 'effect'

import type {
  BlueprintContinuationDecisionKind,
} from '../schemas/continuation-decision'
import type {
  BlueprintContinuationDecisionQueueItem,
  BlueprintContinuationDecisionQueueProjection,
  BlueprintContinuationDecisionQueueSource,
  BlueprintDecisionQueueAudience,
  BlueprintDecisionQueueItemStatus,
} from '../schemas/continuation-decision-queue'

const customerPrivateMaterialPattern =
  /(bearer\s+|cookie|customer[_-]?email|customer[_-]?name|mnemonic|oauth|oa_agent_|openagents_admin|password|preimage|private[_-]?key|provider[_-]?account|provider[_-]?token|raw[_-]?runner|runner[_-]?log|secret|sk-[a-z0-9]|token)/i

const itemStatusForAction = (
  action: BlueprintContinuationDecisionKind,
): BlueprintDecisionQueueItemStatus =>
  M.value(action).pipe(
    M.when('continue', () => 'pending' as const),
    M.when('test', () => 'pending' as const),
    M.when('fix', () => 'blocked' as const),
    M.when('summarize', () => 'pending' as const),
    M.when('request_context', () => 'blocked' as const),
    M.when('retry_account', () => 'retrying' as const),
    M.when('stop', () => 'terminal' as const),
    M.when('escalate', () => 'blocked' as const),
    M.when('prepare_review', () => 'needs_review' as const),
    M.exhaustive,
  )

const recommendedNextOrderForAction = (
  action: BlueprintContinuationDecisionKind,
): string =>
  M.value(action).pipe(
    M.when('continue', () => 'next.continuation.continue_run'),
    M.when('test', () => 'next.continuation.run_verification'),
    M.when('fix', () => 'next.continuation.repair_failure'),
    M.when('summarize', () => 'next.continuation.render_summary'),
    M.when('request_context', () => 'next.continuation.request_context'),
    M.when('retry_account', () => 'next.continuation.retry_or_rotate_account'),
    M.when('stop', () => 'next.continuation.stop_run'),
    M.when('escalate', () => 'next.continuation.operator_escalation'),
    M.when('prepare_review', () => 'next.continuation.prepare_review'),
    M.exhaustive,
  )

const blockerRefsForSource = (
  source: BlueprintContinuationDecisionQueueSource,
): ReadonlyArray<string> => [
  ...source.turnResult.blockerRefs,
  ...source.turnResult.buildFailureRefs,
  ...source.turnResult.missingContextRefs,
  ...source.turnResult.runtimeFailureRefs,
  ...source.turnResult.testFailureRefs,
]

const approvalRefsForAction = (
  action: BlueprintContinuationDecisionKind,
): ReadonlyArray<string> =>
  M.value(action).pipe(
    M.when('escalate', () => ['approval.operator_attention']),
    M.when('prepare_review', () => ['approval.customer_or_operator_review']),
    M.orElse(() => []),
  )

const stopConditionRefsForSource = (
  source: BlueprintContinuationDecisionQueueSource,
): ReadonlyArray<string> =>
  source.decision.action === 'stop'
    ? ['stop.user_or_policy_requested', ...source.turnResult.evidenceRefs]
    : []

const retryRefsForSource = (
  source: BlueprintContinuationDecisionQueueSource,
  audience: BlueprintDecisionQueueAudience,
): ReadonlyArray<string> => {
  if (source.decision.action !== 'retry_account') {
    return []
  }

  return audience === 'operator'
    ? source.turnResult.accountFailureRefs
    : ['retry.account_failover_needed']
}

const accountFailoverRefsForSource = (
  source: BlueprintContinuationDecisionQueueSource,
  audience: BlueprintDecisionQueueAudience,
): ReadonlyArray<string> =>
  source.decision.action === 'retry_account' && audience === 'operator'
    ? source.turnResult.accountFailureRefs
    : []

const sourceAuthorityRefsForAudience = (
  source: BlueprintContinuationDecisionQueueSource,
  audience: BlueprintDecisionQueueAudience,
): ReadonlyArray<string> =>
  audience === 'operator' ? source.decision.sourceAuthorityRefs : []

const safeRefsForAudience = (
  refs: ReadonlyArray<string>,
  audience: BlueprintDecisionQueueAudience,
): ReadonlyArray<string> =>
  audience === 'operator'
    ? refs
    : refs.filter(ref => !customerPrivateMaterialPattern.test(ref))

const queueItemFromSource = (
  source: BlueprintContinuationDecisionQueueSource,
  audience: BlueprintDecisionQueueAudience,
): BlueprintContinuationDecisionQueueItem => ({
  accountFailoverNeeded: source.decision.action === 'retry_account',
  accountFailoverRefs: accountFailoverRefsForSource(source, audience),
  action: source.decision.action,
  approvalRefs: approvalRefsForAction(source.decision.action),
  blockerRefs: safeRefsForAudience(blockerRefsForSource(source), audience),
  constraintRefs: safeRefsForAudience(source.decision.constraintRefs, audience),
  customerVisible: true,
  decisionRef: source.decision.decisionRef,
  evidenceRefs: safeRefsForAudience(source.decision.evidenceRefs, audience),
  id: `decision_queue.${source.decision.decisionRef}`,
  orderRefs: safeRefsForAudience(source.orderRefs, audience),
  programRunRef: source.programRunRef,
  programSignatureId: source.decision.programSignatureId,
  recommendedNextOrderRef: recommendedNextOrderForAction(source.decision.action),
  receiptRefs: safeRefsForAudience(source.decision.receiptRefs, audience),
  retryRefs: retryRefsForSource(source, audience),
  safeSummaryRef: source.safeSummaryRef,
  siteRefs: safeRefsForAudience(source.siteRefs, audience),
  sourceAuthorityRefs: sourceAuthorityRefsForAudience(source, audience),
  status: itemStatusForAction(source.decision.action),
  stopConditionRefs: safeRefsForAudience(
    stopConditionRefsForSource(source),
    audience,
  ),
  workRef: source.decision.workRef,
  workroomRefs: safeRefsForAudience(source.workroomRefs, audience),
})

export const buildBlueprintContinuationDecisionQueueProjection = (
  sources: ReadonlyArray<BlueprintContinuationDecisionQueueSource>,
  audience: BlueprintDecisionQueueAudience,
): BlueprintContinuationDecisionQueueProjection => {
  const items = sources
    .map(source => queueItemFromSource(source, audience))
    .filter(item => audience === 'operator' || item.customerVisible)

  return {
    audience,
    blockerCount: items.filter(item => item.blockerRefs.length > 0).length,
    empty: items.length === 0,
    items,
    pendingCount: items.filter(item => item.status === 'pending').length,
    retryCount: items.filter(item => item.accountFailoverNeeded).length,
    reviewCount: items.filter(item => item.status === 'needs_review').length,
    stopCount: items.filter(item => item.status === 'terminal').length,
  }
}

export const blueprintContinuationDecisionQueueProjectionHasCustomerPrivateMaterial =
  (
    projection: BlueprintContinuationDecisionQueueProjection,
  ): boolean =>
    customerPrivateMaterialPattern.test(JSON.stringify(projection))
