import { Match as M, Schema as S } from 'effect'

import {
  PublicProjectionStalenessContract,
  liveAtReadStaleness,
} from './public-projection-staleness'

export const AutopilotAttentionEventKind = S.Literals([
  'background_completed',
  'background_failed',
  'delivered',
  'diagnostic_warning',
  'login_needed',
  'session_resumed',
  'session_forked',
  'waiting_for_approval',
  'waiting_for_background_decision',
  'waiting_for_input',
])
export type AutopilotAttentionEventKind =
  typeof AutopilotAttentionEventKind.Type

export const AutopilotAttentionPriority = S.Literals([
  'critical',
  'high',
  'normal',
  'low',
])
export type AutopilotAttentionPriority = typeof AutopilotAttentionPriority.Type

export const AutopilotAttentionChannel = S.Literals([
  'disabled',
  'desktop_hook',
  'email',
  'in_app',
  'terminal',
])
export type AutopilotAttentionChannel = typeof AutopilotAttentionChannel.Type

export const AutopilotAttentionPrivacyClass = S.Literals([
  'public_ref',
  'team_summary',
  'owner_summary',
  'operator_summary',
])
export type AutopilotAttentionPrivacyClass =
  typeof AutopilotAttentionPrivacyClass.Type

export const AutopilotAttentionStatus = S.Literals([
  'current',
  'folded',
  'invalidated',
  'resolved',
])
export type AutopilotAttentionStatus = typeof AutopilotAttentionStatus.Type

export class AutopilotPackASupervisionUnsafe extends S.TaggedErrorClass<AutopilotPackASupervisionUnsafe>()(
  'AutopilotPackASupervisionUnsafe',
  {
    reason: S.String,
  },
) {}

export class AutopilotAttentionEventRecord extends S.Class<AutopilotAttentionEventRecord>(
  'AutopilotAttentionEventRecord',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  createdAt: S.String,
  decisionRef: S.NullOr(S.String),
  dedupeKey: S.String,
  eventRef: S.String,
  foldsWith: S.Array(S.String),
  invalidates: S.Array(S.String),
  kind: AutopilotAttentionEventKind,
  missionRef: S.String,
  preferredChannel: AutopilotAttentionChannel,
  priority: AutopilotAttentionPriority,
  privacyClass: AutopilotAttentionPrivacyClass,
  resolvedAt: S.NullOr(S.String),
  runRef: S.String,
  safeSummaryRef: S.String,
  timeoutAt: S.NullOr(S.String),
  workOrderRef: S.String,
}) {}

export class AutopilotAttentionDeliveryReceipt extends S.Class<AutopilotAttentionDeliveryReceipt>(
  'AutopilotAttentionDeliveryReceipt',
)({
  attemptedAt: S.String,
  channel: AutopilotAttentionChannel,
  deliveryRef: S.String,
  errorSummaryRef: S.NullOr(S.String),
  eventRef: S.String,
  idempotencyKey: S.String,
  status: S.Literals(['delivered', 'failed', 'skipped']),
}) {}

export class AutopilotCurrentAttentionItem extends S.Class<AutopilotCurrentAttentionItem>(
  'AutopilotCurrentAttentionItem',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  channel: AutopilotAttentionChannel,
  dedupeKey: S.String,
  eventRef: S.String,
  kind: AutopilotAttentionEventKind,
  priority: AutopilotAttentionPriority,
  receiptRefs: S.Array(S.String),
  safeSummaryRef: S.String,
  status: AutopilotAttentionStatus,
  timeoutAt: S.NullOr(S.String),
  waitingState: S.Literals([
    'completion_available',
    'failure_available',
    'login_needed',
    'none',
    'waiting_for_approval',
    'waiting_for_background_decision',
    'waiting_for_input',
  ]),
  workOrderRef: S.String,
}) {}

export class AutopilotAttentionProjection extends S.Class<AutopilotAttentionProjection>(
  'AutopilotAttentionProjection',
)({
  current: S.Array(AutopilotCurrentAttentionItem),
  deliveryFailures: S.Array(S.String),
  generatedAt: S.String,
  projectionRef: S.Literal('openagents.autopilot_pack_a_attention.v1'),
  staleness: PublicProjectionStalenessContract,
}) {}

export const AutopilotCompanionRunStatus = S.Literals([
  'blocked',
  'cancelled',
  'completed',
  'failed',
  'private_only',
  'running',
  'scheduled',
  'stale',
  'waiting',
])
export type AutopilotCompanionRunStatus =
  typeof AutopilotCompanionRunStatus.Type

export const AutopilotCompanionActionKind = S.Literals([
  'answer',
  'approve',
  'cancel',
  'deny',
  'pause',
  'resume',
  'send_bounded_instruction',
])
export type AutopilotCompanionActionKind =
  typeof AutopilotCompanionActionKind.Type

export class AutopilotCompanionProjectionInput extends S.Class<AutopilotCompanionProjectionInput>(
  'AutopilotCompanionProjectionInput',
)({
  artifactRefs: S.Array(S.String),
  attentionItems: S.Array(AutopilotCurrentAttentionItem),
  budgetStatusRef: S.String,
  caveatRefs: S.Array(S.String),
  latestPublicProgressRef: S.NullOr(S.String),
  missionRef: S.String,
  runRef: S.String,
  status: AutopilotCompanionRunStatus,
  updatedAt: S.String,
  waitingDecisionRef: S.NullOr(S.String),
  workOrderRef: S.String,
}) {}

export class AutopilotCompanionProjectionRow extends S.Class<AutopilotCompanionProjectionRow>(
  'AutopilotCompanionProjectionRow',
)({
  actionRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  budgetStatusRef: S.String,
  caveatRefs: S.Array(S.String),
  generatedAt: S.String,
  latestPublicProgressRef: S.NullOr(S.String),
  missionRef: S.String,
  projectionRef: S.Literal('openagents.autopilot_pack_a_companion.v1'),
  runRef: S.String,
  staleness: PublicProjectionStalenessContract,
  status: AutopilotCompanionRunStatus,
  updatedAt: S.String,
  waitingDecisionRef: S.NullOr(S.String),
  workOrderRef: S.String,
}) {}

export class AutopilotCompanionActionRequest extends S.Class<AutopilotCompanionActionRequest>(
  'AutopilotCompanionActionRequest',
)({
  actionKind: AutopilotCompanionActionKind,
  actorMembership: S.Literals(['member', 'non_member']),
  decisionRef: S.NullOr(S.String),
  idempotencyKey: S.String,
  requestedAt: S.String,
  requestRef: S.String,
  workOrderRef: S.String,
}) {}

export class AutopilotCompanionActionDecision extends S.Class<AutopilotCompanionActionDecision>(
  'AutopilotCompanionActionDecision',
)({
  actionKind: AutopilotCompanionActionKind,
  decision: S.Literals(['accepted', 'rejected']),
  directEffectPermitted: S.Literal(false),
  reasonRef: S.String,
  receiptRef: S.String,
  requestRef: S.String,
}) {}

export const AutopilotPermissionActionKind = S.Literals([
  'api_action',
  'approval',
  'cancellation',
  'continuation',
  'file',
  'notification',
  'provider',
  'schedule',
  'shell',
  'spend',
  'workspace',
])
export type AutopilotPermissionActionKind =
  typeof AutopilotPermissionActionKind.Type

export const AutopilotPermissionMode = S.Literals([
  'accept_edits',
  'bypass_with_hard_checks',
  'classifier',
  'default',
  'do_not_ask_headless',
  'plan_read_only',
])
export type AutopilotPermissionMode = typeof AutopilotPermissionMode.Type

export const AutopilotPermissionDecisionKind = S.Literals([
  'allow',
  'ask',
  'deny',
  'passthrough',
])
export type AutopilotPermissionDecisionKind =
  typeof AutopilotPermissionDecisionKind.Type

export class AutopilotPermissionRequest extends S.Class<AutopilotPermissionRequest>(
  'AutopilotPermissionRequest',
)({
  actionKind: AutopilotPermissionActionKind,
  allowRuleRefs: S.Array(S.String),
  askRuleRefs: S.Array(S.String),
  background: S.Boolean,
  classifierAvailable: S.Boolean,
  denyRuleRefs: S.Array(S.String),
  hardSafetyCheckRefs: S.Array(S.String),
  mode: AutopilotPermissionMode,
  promptAvailable: S.Boolean,
  remoteApprovalAvailable: S.Boolean,
  requestRef: S.String,
  riskRef: S.String,
  runRef: S.String,
}) {}

export class AutopilotPermissionDecision extends S.Class<AutopilotPermissionDecision>(
  'AutopilotPermissionDecision',
)({
  auditRef: S.String,
  decision: AutopilotPermissionDecisionKind,
  decisionReasonRef: S.String,
  persistTo: S.Literals([
    'none',
    'decision_queue',
    'session_policy',
    'operator_audit',
  ]),
  redactionClass: AutopilotAttentionPrivacyClass,
  requestRef: S.String,
  source: S.Literals([
    'hard_check',
    'mode_policy',
    'prompt',
    'remote_approval',
    'saved_rule',
  ]),
  updateHintRefs: S.Array(S.String),
  waitingState: S.Literals(['none', 'waiting_for_approval']),
}) {}

export const AutopilotInteractionMode = S.Literals([
  'ci',
  'headless_service',
  'interactive_tui',
  'json',
  'plain_terminal',
  'screen_reader',
])
export type AutopilotInteractionMode = typeof AutopilotInteractionMode.Type

export class AutopilotInteractionModeContract extends S.Class<AutopilotInteractionModeContract>(
  'AutopilotInteractionModeContract',
)({
  approvals: S.Boolean,
  deploys: S.Boolean,
  headless: S.Boolean,
  liveSpend: S.Boolean,
  mode: AutopilotInteractionMode,
  notifications: S.Boolean,
  prompts: S.Boolean,
  providerMutation: S.Boolean,
  push: S.Boolean,
  remoteBridges: S.Boolean,
}) {}

export class AutopilotStructuredOutputEnvelope extends S.Class<AutopilotStructuredOutputEnvelope>(
  'AutopilotStructuredOutputEnvelope',
)({
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  decisionRefs: S.Array(S.String),
  exitCode: S.Number,
  generatedAt: S.String,
  mode: AutopilotInteractionMode,
  receiptRefs: S.Array(S.String),
  status: S.Literals(['blocked', 'failed', 'ok', 'waiting']),
  taskRefs: S.Array(S.String),
}) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/
const unsafeMaterialPattern =
  /(@|access[_-]?token|auth\.json|bearer|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|secret|sk-[a-z0-9]|token|wallet|webhook[_-]?secret|\/Users\/|\/home\/)/i
const privateArtifactPattern =
  /(artifact\.private|private[_-]?artifact|raw[_-]?(patch|source|prompt|log)|workroom\.private)/i

const failUnsafe = (reason: string): never => {
  const error = new AutopilotPackASupervisionUnsafe({ reason })

  throw error
}

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertSafeRefs = (label: string, refs: ReadonlyArray<string>): void => {
  const unsafe = uniqueRefs(refs).find(
    ref => !safeRefPattern.test(ref) || unsafeMaterialPattern.test(ref),
  )

  if (unsafe !== undefined) {
    failUnsafe(
      `${label} contains private, secret, provider, wallet, payment, customer, local-path, or raw payload material.`,
    )
  }
}

const publicArtifactRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const malformed = uniqueRefs(refs).find(ref => !safeRefPattern.test(ref))

  if (malformed !== undefined) {
    failUnsafe(
      'artifact refs contain malformed local-path or non-ref material.',
    )
  }

  assertSafeRefs(
    'artifact refs',
    refs.filter(ref => !privateArtifactPattern.test(ref)),
  )

  return uniqueRefs(refs).filter(ref => !privateArtifactPattern.test(ref))
}

const statusForEvent = (
  event: AutopilotAttentionEventRecord,
  invalidatedDedupeKeys: ReadonlySet<string>,
): AutopilotAttentionStatus => {
  if (event.resolvedAt !== null) {
    return 'resolved'
  }

  if (invalidatedDedupeKeys.has(event.dedupeKey)) {
    return 'invalidated'
  }

  return 'current'
}

const waitingStateForKind = (
  kind: AutopilotAttentionEventKind,
): AutopilotCurrentAttentionItem['waitingState'] =>
  M.value(kind).pipe(
    M.withReturnType<AutopilotCurrentAttentionItem['waitingState']>(),
    M.when('background_completed', () => 'completion_available'),
    M.when('background_failed', () => 'failure_available'),
    M.when('delivered', () => 'completion_available'),
    M.when('login_needed', () => 'login_needed'),
    M.when('waiting_for_approval', () => 'waiting_for_approval'),
    M.when(
      'waiting_for_background_decision',
      () => 'waiting_for_background_decision',
    ),
    M.when('waiting_for_input', () => 'waiting_for_input'),
    M.orElse(() => 'none'),
  )

export const projectAutopilotAttention = (
  input: Readonly<{
    deliveryReceipts: ReadonlyArray<AutopilotAttentionDeliveryReceipt>
    events: ReadonlyArray<AutopilotAttentionEventRecord>
    nowIso: string
  }>,
): AutopilotAttentionProjection => {
  input.events.forEach(event => {
    assertSafeRefs('attention event refs', [
      event.eventRef,
      event.runRef,
      event.workOrderRef,
      event.missionRef,
      event.decisionRef ?? '',
      event.safeSummaryRef,
    ])
    assertSafeRefs('attention blocker refs', event.blockerRefs)
    assertSafeRefs('attention invalidation refs', event.invalidates)
    assertSafeRefs('attention folding refs', event.foldsWith)
  })
  input.deliveryReceipts.forEach(receipt => {
    assertSafeRefs('attention delivery receipt refs', [
      receipt.deliveryRef,
      receipt.eventRef,
      receipt.errorSummaryRef ?? '',
      receipt.idempotencyKey,
    ])
  })

  const invalidatedDedupeKeys = new Set(
    input.events.flatMap(event => event.invalidates),
  )
  const latestByDedupeKey = new Map(
    [...input.events]
      .sort((left, right) => left.createdAt.localeCompare(right.createdAt))
      .map(event => [event.dedupeKey, event] as const),
  )
  const receiptRefsByEventRef = new Map(
    input.events.map(
      event =>
        [
          event.eventRef,
          uniqueRefs(
            input.deliveryReceipts
              .filter(receipt => receipt.eventRef === event.eventRef)
              .map(receipt => receipt.deliveryRef),
          ),
        ] as const,
    ),
  )
  const current = [...latestByDedupeKey.values()]
    .map(event => ({
      artifactRefs: publicArtifactRefs(event.artifactRefs),
      blockerRefs: uniqueRefs(event.blockerRefs),
      channel: event.preferredChannel,
      dedupeKey: event.dedupeKey,
      eventRef: event.eventRef,
      kind: event.kind,
      priority: event.priority,
      receiptRefs: receiptRefsByEventRef.get(event.eventRef) ?? [],
      safeSummaryRef: event.safeSummaryRef,
      status: statusForEvent(event, invalidatedDedupeKeys),
      timeoutAt: event.timeoutAt,
      waitingState: waitingStateForKind(event.kind),
      workOrderRef: event.workOrderRef,
    }))
    .filter(item => item.status === 'current')

  return new AutopilotAttentionProjection({
    current,
    deliveryFailures: uniqueRefs(
      input.deliveryReceipts
        .filter(receipt => receipt.status === 'failed')
        .map(receipt => receipt.deliveryRef),
    ),
    generatedAt: input.nowIso,
    projectionRef: 'openagents.autopilot_pack_a_attention.v1',
    staleness: liveAtReadStaleness([
      'autopilot_attention_event_recorded',
      'autopilot_attention_delivery_receipt_recorded',
      'autopilot_decision_resolved',
      'autopilot_run_cancelled',
    ]),
  })
}

export const selectAttentionDeliveryChannel = (
  input: Readonly<{
    disabledChannels: ReadonlyArray<AutopilotAttentionChannel>
    event: AutopilotAttentionEventRecord
    fallbackChannels: ReadonlyArray<AutopilotAttentionChannel>
  }>,
): AutopilotAttentionChannel => {
  const allowed = (channel: AutopilotAttentionChannel): boolean =>
    channel !== 'disabled' && !input.disabledChannels.includes(channel)

  return allowed(input.event.preferredChannel)
    ? input.event.preferredChannel
    : (input.fallbackChannels.find(allowed) ?? 'disabled')
}

export const projectAutopilotCompanionRow = (
  input: AutopilotCompanionProjectionInput,
  generatedAt: string,
): AutopilotCompanionProjectionRow => {
  assertSafeRefs('companion refs', [
    input.budgetStatusRef,
    input.latestPublicProgressRef ?? '',
    input.missionRef,
    input.runRef,
    input.waitingDecisionRef ?? '',
    input.workOrderRef,
  ])
  assertSafeRefs('companion caveat refs', input.caveatRefs)

  const waitingActionRefs =
    input.waitingDecisionRef === null
      ? []
      : [
          `action.${input.waitingDecisionRef}.approve`,
          `action.${input.waitingDecisionRef}.deny`,
          `action.${input.waitingDecisionRef}.answer`,
        ]
  const controlActionRefs = M.value(input.status).pipe(
    M.withReturnType<ReadonlyArray<string>>(),
    M.when('running', () => [
      `action.${input.workOrderRef}.pause`,
      `action.${input.workOrderRef}.cancel`,
      `action.${input.workOrderRef}.send_bounded_instruction`,
    ]),
    M.when('blocked', () => [`action.${input.workOrderRef}.cancel`]),
    M.when('scheduled', () => [`action.${input.workOrderRef}.cancel`]),
    M.when('waiting', () => [`action.${input.workOrderRef}.cancel`]),
    M.orElse(() => []),
  )

  return new AutopilotCompanionProjectionRow({
    actionRefs: uniqueRefs([...waitingActionRefs, ...controlActionRefs]),
    artifactRefs: publicArtifactRefs(input.artifactRefs),
    budgetStatusRef: input.budgetStatusRef,
    caveatRefs: uniqueRefs([
      ...input.caveatRefs,
      ...(input.artifactRefs.length >
      publicArtifactRefs(input.artifactRefs).length
        ? ['caveat.private_artifacts_redacted']
        : []),
    ]),
    generatedAt,
    latestPublicProgressRef: input.latestPublicProgressRef,
    missionRef: input.missionRef,
    projectionRef: 'openagents.autopilot_pack_a_companion.v1',
    runRef: input.runRef,
    staleness: liveAtReadStaleness([
      'autopilot_work_order_state_transition',
      'autopilot_attention_event_recorded',
      'autopilot_artifact_receipt_recorded',
      'autopilot_budget_state_changed',
    ]),
    status: input.status,
    updatedAt: input.updatedAt,
    waitingDecisionRef: input.waitingDecisionRef,
    workOrderRef: input.workOrderRef,
  })
}

export const decideAutopilotCompanionAction = (
  input: Readonly<{
    projection: AutopilotCompanionProjectionRow
    request: AutopilotCompanionActionRequest
    stale: boolean
  }>,
): AutopilotCompanionActionDecision => {
  assertSafeRefs('companion action refs', [
    input.request.decisionRef ?? '',
    input.request.idempotencyKey,
    input.request.requestRef,
    input.request.workOrderRef,
  ])

  const reasonRef =
    input.request.actorMembership === 'non_member'
      ? 'blocker.autopilot_companion.non_member'
      : input.stale
        ? 'blocker.autopilot_companion.stale_decision'
        : input.request.decisionRef !== null &&
            input.request.decisionRef !== input.projection.waitingDecisionRef
          ? 'blocker.autopilot_companion.decision_ref_mismatch'
          : 'receipt.autopilot_companion.action_accepted'

  return new AutopilotCompanionActionDecision({
    actionKind: input.request.actionKind,
    decision: reasonRef.startsWith('blocker.') ? 'rejected' : 'accepted',
    directEffectPermitted: false,
    reasonRef,
    receiptRef: `receipt.${input.request.requestRef}`,
    requestRef: input.request.requestRef,
  })
}

export const decideAutopilotPermission = (
  request: AutopilotPermissionRequest,
): AutopilotPermissionDecision => {
  assertSafeRefs('permission request refs', [
    request.requestRef,
    request.riskRef,
    request.runRef,
    ...request.allowRuleRefs,
    ...request.askRuleRefs,
    ...request.denyRuleRefs,
    ...request.hardSafetyCheckRefs,
  ])

  const base = {
    auditRef: `audit.${request.requestRef}`,
    redactionClass: 'operator_summary' as const,
    requestRef: request.requestRef,
  }
  const hardDenied = request.hardSafetyCheckRefs.length > 0
  const deniedByRule = request.denyRuleRefs.length > 0
  const askedByRule = request.askRuleRefs.length > 0
  const headlessWithoutResolver =
    !request.promptAvailable && !request.remoteApprovalAvailable
  const classifierUnavailable =
    request.mode === 'classifier' && !request.classifierAvailable
  const allowedByRule =
    request.allowRuleRefs.length > 0 &&
    !hardDenied &&
    !deniedByRule &&
    !askedByRule

  if (hardDenied || deniedByRule) {
    return new AutopilotPermissionDecision({
      ...base,
      decision: 'deny',
      decisionReasonRef: hardDenied
        ? 'permission.hard_check_denied'
        : 'permission.deny_rule_matched',
      persistTo: 'operator_audit',
      source: hardDenied ? 'hard_check' : 'saved_rule',
      updateHintRefs: ['hint.permission.request_must_change'],
      waitingState: 'none',
    })
  }

  if (classifierUnavailable || headlessWithoutResolver) {
    return new AutopilotPermissionDecision({
      ...base,
      decision: 'deny',
      decisionReasonRef: classifierUnavailable
        ? 'permission.classifier_unavailable_fail_closed'
        : 'permission.prompt_unavailable_no_remote_resolver',
      persistTo: 'operator_audit',
      source: 'mode_policy',
      updateHintRefs: ['hint.permission.configure_remote_approval'],
      waitingState: 'none',
    })
  }

  if (
    askedByRule ||
    request.background ||
    request.mode === 'do_not_ask_headless'
  ) {
    return new AutopilotPermissionDecision({
      ...base,
      decision: 'ask',
      decisionReasonRef: request.remoteApprovalAvailable
        ? 'permission.remote_approval_required'
        : 'permission.prompt_approval_required',
      persistTo: 'decision_queue',
      source: request.remoteApprovalAvailable ? 'remote_approval' : 'prompt',
      updateHintRefs: ['hint.permission.approval_expires_with_request'],
      waitingState: 'waiting_for_approval',
    })
  }

  if (allowedByRule || request.mode === 'accept_edits') {
    return new AutopilotPermissionDecision({
      ...base,
      decision: 'allow',
      decisionReasonRef: allowedByRule
        ? 'permission.allow_rule_matched'
        : 'permission.accept_edits_mode',
      persistTo: allowedByRule ? 'session_policy' : 'none',
      source: allowedByRule ? 'saved_rule' : 'mode_policy',
      updateHintRefs: [],
      waitingState: 'none',
    })
  }

  return new AutopilotPermissionDecision({
    ...base,
    decision: 'passthrough',
    decisionReasonRef: 'permission.no_direct_effect_requested',
    persistTo: 'none',
    source: 'mode_policy',
    updateHintRefs: [],
    waitingState: 'none',
  })
}

export const interactionModeContract = (
  mode: AutopilotInteractionMode,
): AutopilotInteractionModeContract =>
  M.value(mode).pipe(
    M.withReturnType<AutopilotInteractionModeContract>(),
    M.when(
      'interactive_tui',
      () =>
        new AutopilotInteractionModeContract({
          approvals: true,
          deploys: false,
          headless: false,
          liveSpend: false,
          mode,
          notifications: true,
          prompts: true,
          providerMutation: false,
          push: false,
          remoteBridges: true,
        }),
    ),
    M.when(
      'plain_terminal',
      () =>
        new AutopilotInteractionModeContract({
          approvals: true,
          deploys: false,
          headless: false,
          liveSpend: false,
          mode,
          notifications: true,
          prompts: true,
          providerMutation: false,
          push: false,
          remoteBridges: true,
        }),
    ),
    M.when(
      'screen_reader',
      () =>
        new AutopilotInteractionModeContract({
          approvals: true,
          deploys: false,
          headless: false,
          liveSpend: false,
          mode,
          notifications: true,
          prompts: true,
          providerMutation: false,
          push: false,
          remoteBridges: true,
        }),
    ),
    M.when(
      'json',
      () =>
        new AutopilotInteractionModeContract({
          approvals: false,
          deploys: false,
          headless: true,
          liveSpend: false,
          mode,
          notifications: false,
          prompts: false,
          providerMutation: false,
          push: false,
          remoteBridges: true,
        }),
    ),
    M.when(
      'ci',
      () =>
        new AutopilotInteractionModeContract({
          approvals: false,
          deploys: false,
          headless: true,
          liveSpend: false,
          mode,
          notifications: false,
          prompts: false,
          providerMutation: false,
          push: false,
          remoteBridges: false,
        }),
    ),
    M.when(
      'headless_service',
      () =>
        new AutopilotInteractionModeContract({
          approvals: false,
          deploys: false,
          headless: true,
          liveSpend: false,
          mode,
          notifications: true,
          prompts: false,
          providerMutation: false,
          push: false,
          remoteBridges: true,
        }),
    ),
    M.exhaustive,
  )

export const structuredOutputEnvelope = (
  input: Readonly<{
    artifactRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    caveatRefs: ReadonlyArray<string>
    decisionRefs: ReadonlyArray<string>
    generatedAt: string
    mode: AutopilotInteractionMode
    receiptRefs: ReadonlyArray<string>
    status: AutopilotStructuredOutputEnvelope['status']
    taskRefs: ReadonlyArray<string>
  }>,
): AutopilotStructuredOutputEnvelope => {
  assertSafeRefs('structured output refs', [
    ...input.blockerRefs,
    ...input.caveatRefs,
    ...input.decisionRefs,
    ...input.receiptRefs,
    ...input.taskRefs,
  ])

  const exitCode = M.value(input.status).pipe(
    M.withReturnType<number>(),
    M.when('ok', () => 0),
    M.when('waiting', () => 2),
    M.when('blocked', () => 3),
    M.when('failed', () => 1),
    M.exhaustive,
  )

  return new AutopilotStructuredOutputEnvelope({
    artifactRefs: publicArtifactRefs(input.artifactRefs),
    blockerRefs: uniqueRefs(input.blockerRefs),
    caveatRefs: uniqueRefs(input.caveatRefs),
    decisionRefs: uniqueRefs(input.decisionRefs),
    exitCode,
    generatedAt: input.generatedAt,
    mode: input.mode,
    receiptRefs: uniqueRefs(input.receiptRefs),
    status: input.status,
    taskRefs: uniqueRefs(input.taskRefs),
  })
}
