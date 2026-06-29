import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisOperatorSteeringAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisOperatorSteeringAudience =
  typeof ArtanisOperatorSteeringAudience.Type

export const ArtanisOperatorGoalCommandKind = S.Literals([
  'cancel_goal',
  'create_goal',
  'pause_goal',
  'reprioritize_goal',
  'resume_goal',
])
export type ArtanisOperatorGoalCommandKind =
  typeof ArtanisOperatorGoalCommandKind.Type

export const ArtanisOperatorCommandState = S.Literals([
  'accepted',
  'blocked',
  'completed',
  'superseded',
])
export type ArtanisOperatorCommandState =
  typeof ArtanisOperatorCommandState.Type

export const ArtanisOperatorApprovalDecisionState = S.Literals([
  'approved',
  'rejected',
])
export type ArtanisOperatorApprovalDecisionState =
  typeof ArtanisOperatorApprovalDecisionState.Type

export const ArtanisOperatorEndpointMethod = S.Literals(['GET', 'PATCH', 'POST'])
export type ArtanisOperatorEndpointMethod =
  typeof ArtanisOperatorEndpointMethod.Type

export class ArtanisOperatorEndpointRef extends S.Class<ArtanisOperatorEndpointRef>(
  'ArtanisOperatorEndpointRef',
)({
  action: ArtanisOperatorGoalCommandKind,
  href: S.String,
  method: ArtanisOperatorEndpointMethod,
}) {}

export class ArtanisOperatorGoalCommandRecord extends S.Class<ArtanisOperatorGoalCommandRecord>(
  'ArtanisOperatorGoalCommandRecord',
)({
  actionProposalRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  commandRef: S.String,
  createdAtIso: S.String,
  goalRef: S.String,
  idempotencyKey: S.String,
  kind: ArtanisOperatorGoalCommandKind,
  operatorReceiptRefs: S.Array(S.String),
  priority: S.Int,
  privateEvidenceRefs: S.Array(S.String),
  publicProjectionRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: ArtanisOperatorCommandState,
  updatedAtIso: S.String,
}) {}

export class ArtanisOperatorApprovalDecisionRecord extends S.Class<ArtanisOperatorApprovalDecisionRecord>(
  'ArtanisOperatorApprovalDecisionRecord',
)({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  decisionRef: S.String,
  idempotencyKey: S.String,
  operatorReceiptRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: ArtanisOperatorApprovalDecisionState,
  updatedAtIso: S.String,
}) {}

export class ArtanisOperatorSteeringWorkspaceRecord extends S.Class<ArtanisOperatorSteeringWorkspaceRecord>(
  'ArtanisOperatorSteeringWorkspaceRecord',
)({
  agentId: S.String,
  approvalDecisions: S.Array(ArtanisOperatorApprovalDecisionRecord),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  goalCommands: S.Array(ArtanisOperatorGoalCommandRecord),
  privateEvidencePackRefs: S.Array(S.String),
  rawWorkroomStateRefs: S.Array(S.String),
  teamRef: S.String,
  updatedAtIso: S.String,
  workspaceRef: S.String,
}) {}

export class ArtanisOperatorGoalCommandProjection extends S.Class<ArtanisOperatorGoalCommandProjection>(
  'ArtanisOperatorGoalCommandProjection',
)({
  actionProposalRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  commandRef: S.String,
  createdAtDisplay: S.String,
  goalRef: S.String,
  kind: ArtanisOperatorGoalCommandKind,
  operatorReceiptRefs: S.Array(S.String),
  priority: S.Int,
  privateEvidenceRefs: S.Array(S.String),
  publicProjectionRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: ArtanisOperatorCommandState,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisOperatorApprovalDecisionProjection extends S.Class<ArtanisOperatorApprovalDecisionProjection>(
  'ArtanisOperatorApprovalDecisionProjection',
)({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  decisionRef: S.String,
  operatorReceiptRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  rawWorkroomRefs: S.Array(S.String),
  state: ArtanisOperatorApprovalDecisionState,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisOperatorSteeringProjection extends S.Class<ArtanisOperatorSteeringProjection>(
  'ArtanisOperatorSteeringProjection',
)({
  agentId: S.String,
  approvalDecisions: S.Array(ArtanisOperatorApprovalDecisionProjection),
  audience: ArtanisOperatorSteeringAudience,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  goalCommands: S.Array(ArtanisOperatorGoalCommandProjection),
  operatorEndpoints: S.Array(ArtanisOperatorEndpointRef),
  privateEvidencePackRefs: S.Array(S.String),
  rawWorkroomStateRefs: S.Array(S.String),
  supportedApprovalActions: S.Array(S.String),
  supportedGoalActions: S.Array(ArtanisOperatorGoalCommandKind),
  teamRef: S.String,
  updatedAtDisplay: S.String,
  workspaceRef: S.String,
}) {}

export class ArtanisOperatorSteeringUnsafe extends S.TaggedErrorClass<ArtanisOperatorSteeringUnsafe>()(
  'ArtanisOperatorSteeringUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_OPERATOR_GOAL_ACTIONS: ReadonlyArray<ArtanisOperatorGoalCommandKind> =
  [
    'cancel_goal',
    'create_goal',
    'pause_goal',
    'reprioritize_goal',
    'resume_goal',
  ]

export const ARTANIS_OPERATOR_APPROVAL_ACTIONS = [
  'approve_risky_action',
  'reject_risky_action',
] as const

export const ARTANIS_AUTOPILOT_OPERATOR_ENDPOINTS: ReadonlyArray<ArtanisOperatorEndpointRef> =
  [
    {
      action: 'create_goal',
      href: '/api/operator/autopilot/goals',
      method: 'POST',
    },
    {
      action: 'pause_goal',
      href: '/api/operator/autopilot/goals/{goalId}/pause',
      method: 'POST',
    },
    {
      action: 'resume_goal',
      href: '/api/operator/autopilot/goals/{goalId}/resume',
      method: 'POST',
    },
    {
      action: 'cancel_goal',
      href: '/api/operator/autopilot/goals/{goalId}/clear',
      method: 'POST',
    },
    {
      action: 'reprioritize_goal',
      href: '/api/operator/autopilot/goals/{goalId}',
      method: 'PATCH',
    },
  ]

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeSteeringRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|seed[_-]?phrase|sk-[a-z0-9]|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(approval\.private|command\.private|evidence\.private|goal\.private|operator\.|receipt\.operator|steering\.private|team\.private|workroom\.private)/i
const operatorOnlyAudiences: ReadonlyArray<ArtanisOperatorSteeringAudience> = [
  'operator',
]

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeSteeringRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisOperatorSteeringAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (operatorOnlyAudiences.includes(audience)) {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const publicProjectionRefsForCommand = (
  command: ArtanisOperatorGoalCommandRecord,
  audience: ArtanisOperatorSteeringAudience,
): ReadonlyArray<string> => {
  if (command.state !== 'accepted' && command.state !== 'completed') {
    return []
  }

  return refsForAudience(
    'Artanis operator command public projection refs',
    command.publicProjectionRefs,
    audience,
  )
}

const assertCommand = (
  command: ArtanisOperatorGoalCommandRecord,
): void => {
  assertValidIso('command.createdAtIso', command.createdAtIso)
  assertValidIso('command.updatedAtIso', command.updatedAtIso)
  assertSafeRefs('Artanis operator command ref', [command.commandRef])
  assertSafeRefs(
    'Artanis operator command idempotency key',
    [command.idempotencyKey],
  )
  assertSafeRefs('Artanis operator command goal ref', [command.goalRef])
  assertSafeRefs(
    'Artanis operator command action proposal refs',
    command.actionProposalRefs,
  )
  assertSafeRefs('Artanis operator command blocker refs', command.blockerRefs)
  assertSafeRefs('Artanis operator command caveat refs', command.caveatRefs)
  assertSafeRefs(
    'Artanis operator command operator receipt refs',
    command.operatorReceiptRefs,
  )
  assertSafeRefs(
    'Artanis operator command private evidence refs',
    command.privateEvidenceRefs,
  )
  assertSafeRefs(
    'Artanis operator command public projection refs',
    command.publicProjectionRefs,
  )
  assertSafeRefs(
    'Artanis operator command raw workroom refs',
    command.rawWorkroomRefs,
  )

  if (command.priority < 0) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: 'Artanis operator command priority must be non-negative.',
    })
  }

  if (!hasAny(command.operatorReceiptRefs)) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: 'Artanis operator commands require operator receipt refs.',
    })
  }

  if (command.state === 'blocked' && !hasAny(command.blockerRefs)) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: 'Blocked Artanis operator commands require blocker refs.',
    })
  }

  if (
    (command.state === 'accepted' || command.state === 'completed') &&
    !hasAny(command.publicProjectionRefs)
  ) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason:
        'Accepted or completed Artanis operator commands require public projection refs.',
    })
  }
}

const assertApprovalDecision = (
  decision: ArtanisOperatorApprovalDecisionRecord,
): void => {
  assertValidIso('decision.createdAtIso', decision.createdAtIso)
  assertValidIso('decision.updatedAtIso', decision.updatedAtIso)
  assertSafeRefs('Artanis operator approval action ref', [decision.actionRef])
  assertSafeRefs(
    'Artanis operator approval authority receipt refs',
    decision.authorityReceiptRefs,
  )
  assertSafeRefs('Artanis operator approval caveat refs', decision.caveatRefs)
  assertSafeRefs('Artanis operator approval decision ref', [
    decision.decisionRef,
  ])
  assertSafeRefs('Artanis operator approval idempotency key', [
    decision.idempotencyKey,
  ])
  assertSafeRefs(
    'Artanis operator approval operator receipt refs',
    decision.operatorReceiptRefs,
  )
  assertSafeRefs(
    'Artanis operator approval private evidence refs',
    decision.privateEvidenceRefs,
  )
  assertSafeRefs(
    'Artanis operator approval public status refs',
    decision.publicStatusRefs,
  )
  assertSafeRefs(
    'Artanis operator approval raw workroom refs',
    decision.rawWorkroomRefs,
  )

  if (
    !hasAny(decision.operatorReceiptRefs) ||
    !hasAny(decision.authorityReceiptRefs)
  ) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason:
        'Artanis operator approval decisions require operator and authority receipt refs.',
    })
  }

  if (!hasAny(decision.publicStatusRefs)) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason:
        'Artanis operator approval decisions require public-safe status refs.',
    })
  }
}

export const artanisOperatorGoalLifecycleReady = (
  record: ArtanisOperatorSteeringWorkspaceRecord,
): boolean => {
  const commandKinds = new Set(record.goalCommands.map(command => command.kind))

  return ARTANIS_OPERATOR_GOAL_ACTIONS.every(action =>
    commandKinds.has(action),
  )
}

const assertWorkspace = (
  record: ArtanisOperatorSteeringWorkspaceRecord,
): void => {
  assertValidIso('workspace.createdAtIso', record.createdAtIso)
  assertValidIso('workspace.updatedAtIso', record.updatedAtIso)
  assertSafeRefs('Artanis operator steering agent id', [record.agentId])
  assertSafeRefs('Artanis operator steering workspace ref', [
    record.workspaceRef,
  ])
  assertSafeRefs('Artanis operator steering team ref', [record.teamRef])
  assertSafeRefs('Artanis operator steering caveat refs', record.caveatRefs)
  assertSafeRefs(
    'Artanis operator steering private evidence pack refs',
    record.privateEvidencePackRefs,
  )
  assertSafeRefs(
    'Artanis operator steering raw workroom state refs',
    record.rawWorkroomStateRefs,
  )

  if (record.agentId !== 'agent_artanis') {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: 'Artanis operator steering must target agent_artanis.',
    })
  }

  if (!hasAny(record.goalCommands)) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason: 'Artanis operator steering requires goal commands.',
    })
  }

  if (!artanisOperatorGoalLifecycleReady(record)) {
    throw new ArtanisOperatorSteeringUnsafe({
      reason:
        'Artanis operator steering must support create, pause, resume, cancel, and reprioritize goal commands.',
    })
  }

  record.goalCommands.forEach(assertCommand)
  record.approvalDecisions.forEach(assertApprovalDecision)
}

const projectCommand = (
  command: ArtanisOperatorGoalCommandRecord,
  audience: ArtanisOperatorSteeringAudience,
  nowIso: string,
): ArtanisOperatorGoalCommandProjection => ({
  actionProposalRefs: refsForAudience(
    'Artanis operator command action proposal refs',
    command.actionProposalRefs,
    audience,
  ),
  blockerRefs: refsForAudience(
    'Artanis operator command blocker refs',
    command.blockerRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Artanis operator command caveat refs',
    command.caveatRefs,
    audience,
  ),
  commandRef: refsForAudience(
    'Artanis operator command ref',
    [command.commandRef],
    audience,
  )[0] ?? 'command.redacted.artanis_operator',
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    command.createdAtIso,
    nowIso,
  ),
  goalRef: refsForAudience(
    'Artanis operator command goal ref',
    [command.goalRef],
    audience,
  )[0] ?? 'goal.redacted.artanis_operator',
  kind: command.kind,
  operatorReceiptRefs: refsForAudience(
    'Artanis operator command operator receipt refs',
    command.operatorReceiptRefs,
    audience,
  ),
  priority: command.priority,
  privateEvidenceRefs:
    audience === 'operator'
      ? refsForAudience(
          'Artanis operator command private evidence refs',
          command.privateEvidenceRefs,
          audience,
        )
      : [],
  publicProjectionRefs: publicProjectionRefsForCommand(command, audience),
  rawWorkroomRefs:
    audience === 'operator'
      ? refsForAudience(
          'Artanis operator command raw workroom refs',
          command.rawWorkroomRefs,
          audience,
        )
      : [],
  state: command.state,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    command.updatedAtIso,
    nowIso,
  ),
})

const projectApprovalDecision = (
  decision: ArtanisOperatorApprovalDecisionRecord,
  audience: ArtanisOperatorSteeringAudience,
  nowIso: string,
): ArtanisOperatorApprovalDecisionProjection => ({
  actionRef: refsForAudience(
    'Artanis operator approval action ref',
    [decision.actionRef],
    audience,
  )[0] ?? 'action.redacted.artanis_operator',
  authorityReceiptRefs: refsForAudience(
    'Artanis operator approval authority receipt refs',
    decision.authorityReceiptRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Artanis operator approval caveat refs',
    decision.caveatRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    decision.createdAtIso,
    nowIso,
  ),
  decisionRef: refsForAudience(
    'Artanis operator approval decision ref',
    [decision.decisionRef],
    audience,
  )[0] ?? 'decision.redacted.artanis_operator',
  operatorReceiptRefs: refsForAudience(
    'Artanis operator approval operator receipt refs',
    decision.operatorReceiptRefs,
    audience,
  ),
  privateEvidenceRefs:
    audience === 'operator'
      ? refsForAudience(
          'Artanis operator approval private evidence refs',
          decision.privateEvidenceRefs,
          audience,
        )
      : [],
  publicStatusRefs: refsForAudience(
    'Artanis operator approval public status refs',
    decision.publicStatusRefs,
    audience,
  ),
  rawWorkroomRefs:
    audience === 'operator'
      ? refsForAudience(
          'Artanis operator approval raw workroom refs',
          decision.rawWorkroomRefs,
          audience,
        )
      : [],
  state: decision.state,
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    decision.updatedAtIso,
    nowIso,
  ),
})

export const projectArtanisOperatorSteeringWorkspace = (
  record: ArtanisOperatorSteeringWorkspaceRecord,
  audience: ArtanisOperatorSteeringAudience,
  nowIso: string,
): ArtanisOperatorSteeringProjection => {
  assertWorkspace(record)

  return {
    agentId: record.agentId,
    approvalDecisions: record.approvalDecisions.map(decision =>
      projectApprovalDecision(decision, audience, nowIso),
    ),
    audience,
    caveatRefs: refsForAudience(
      'Artanis operator steering caveat refs',
      record.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    goalCommands: record.goalCommands
      .map(command => projectCommand(command, audience, nowIso))
      .sort((left, right) => left.priority - right.priority),
    operatorEndpoints:
      audience === 'operator' ? [...ARTANIS_AUTOPILOT_OPERATOR_ENDPOINTS] : [],
    privateEvidencePackRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis operator steering private evidence pack refs',
            record.privateEvidencePackRefs,
            audience,
          )
        : [],
    rawWorkroomStateRefs:
      audience === 'operator'
        ? refsForAudience(
            'Artanis operator steering raw workroom state refs',
            record.rawWorkroomStateRefs,
            audience,
          )
        : [],
    supportedApprovalActions:
      audience === 'operator' ? [...ARTANIS_OPERATOR_APPROVAL_ACTIONS] : [],
    supportedGoalActions:
      audience === 'operator' ? [...ARTANIS_OPERATOR_GOAL_ACTIONS] : [],
    teamRef:
      refsForAudience('Artanis operator steering team ref', [
        record.teamRef,
      ], audience)[0] ?? 'team.redacted.artanis_operator',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workspaceRef:
      refsForAudience('Artanis operator steering workspace ref', [
        record.workspaceRef,
      ], audience)[0] ?? 'workspace.redacted.artanis_operator',
  }
}

export const artanisOperatorProjectionHasPrivateMaterial = (
  projection: ArtanisOperatorSteeringProjection,
): boolean =>
  publicUnsafeRefPattern.test(JSON.stringify(projection)) ||
  unsafeSteeringRefPattern.test(JSON.stringify(projection)) ||
  rawTimestampPattern.test(JSON.stringify(projection))

const command = (
  input: Readonly<{
    kind: ArtanisOperatorGoalCommandKind
    priority: number
    state?: ArtanisOperatorCommandState | undefined
  }>,
): ArtanisOperatorGoalCommandRecord =>
  new ArtanisOperatorGoalCommandRecord({
    actionProposalRefs:
      input.kind === 'reprioritize_goal'
        ? ['action.public.artanis.reprioritize_model_lab']
        : [],
    blockerRefs: [],
    caveatRefs: ['caveat.public.operator_review_required_for_risk'],
    commandRef: `command.public.artanis.${input.kind}`,
    createdAtIso: '2026-06-06T17:00:00.000Z',
    goalRef: 'goal.public.artanis.pylon_model_lab',
    idempotencyKey: `artanis-operator:${input.kind}:v1`,
    kind: input.kind,
    operatorReceiptRefs: [`receipt.operator.artanis.${input.kind}`],
    priority: input.priority,
    privateEvidenceRefs: [`evidence.private.artanis.${input.kind}`],
    publicProjectionRefs: [`projection.public.artanis.${input.kind}`],
    rawWorkroomRefs: [`workroom.private.artanis.${input.kind}`],
    state: input.state ?? 'accepted',
    updatedAtIso: '2026-06-06T17:05:00.000Z',
  })

export const exampleArtanisOperatorSteeringWorkspace =
  new ArtanisOperatorSteeringWorkspaceRecord({
    agentId: 'agent_artanis',
    approvalDecisions: [
      new ArtanisOperatorApprovalDecisionRecord({
        actionRef: 'action.public.artanis.forum_status_post',
        authorityReceiptRefs: ['receipt.operator.artanis.forum_post_approval'],
        caveatRefs: ['caveat.public.no_wallet_or_provider_authority'],
        createdAtIso: '2026-06-06T17:10:00.000Z',
        decisionRef: 'decision.public.artanis.approve_forum_status_post',
        idempotencyKey: 'artanis-operator:approve-forum-status-post:v1',
        operatorReceiptRefs: ['receipt.operator.artanis.approve_forum_status'],
        privateEvidenceRefs: ['evidence.private.artanis.forum_status_draft'],
        publicStatusRefs: ['approval.public.artanis.forum_status_post_ready'],
        rawWorkroomRefs: ['workroom.private.artanis.forum_status_draft'],
        state: 'approved',
        updatedAtIso: '2026-06-06T17:11:00.000Z',
      }),
      new ArtanisOperatorApprovalDecisionRecord({
        actionRef: 'action.public.artanis.training_launch',
        authorityReceiptRefs: ['receipt.operator.artanis.training_denial'],
        caveatRefs: ['caveat.public.training_launch_not_approved'],
        createdAtIso: '2026-06-06T17:12:00.000Z',
        decisionRef: 'decision.public.artanis.reject_training_launch',
        idempotencyKey: 'artanis-operator:reject-training-launch:v1',
        operatorReceiptRefs: ['receipt.operator.artanis.reject_training'],
        privateEvidenceRefs: ['evidence.private.artanis.training_risk'],
        publicStatusRefs: ['approval.public.artanis.training_launch_rejected'],
        rawWorkroomRefs: ['workroom.private.artanis.training_risk'],
        state: 'rejected',
        updatedAtIso: '2026-06-06T17:13:00.000Z',
      }),
    ],
    caveatRefs: ['caveat.public.operator_console_only'],
    createdAtIso: '2026-06-06T17:00:00.000Z',
    goalCommands: [
      command({ kind: 'create_goal', priority: 1 }),
      command({ kind: 'reprioritize_goal', priority: 2 }),
      command({ kind: 'pause_goal', priority: 3 }),
      command({ kind: 'resume_goal', priority: 4 }),
      command({ kind: 'cancel_goal', priority: 5, state: 'completed' }),
    ],
    privateEvidencePackRefs: ['evidence.private.artanis.operator_pack'],
    rawWorkroomStateRefs: ['workroom.private.artanis.loop_state'],
    teamRef: 'team.private.openagents_core',
    updatedAtIso: '2026-06-06T17:15:00.000Z',
    workspaceRef: 'steering.private.artanis.operator_workspace',
  })
