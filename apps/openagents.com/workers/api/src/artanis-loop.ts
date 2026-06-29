import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisLoopAudience = S.Literals([
  'public',
  'agent',
  'customer',
  'team',
  'operator',
])
export type ArtanisLoopAudience = typeof ArtanisLoopAudience.Type

export const ArtanisLoopState = S.Literals([
  'blocked',
  'completed',
  'failed',
  'paused',
  'queued',
  'running',
  'waiting_for_approval',
])
export type ArtanisLoopState = typeof ArtanisLoopState.Type

export const ArtanisActionKind = S.Literals([
  'eval_launch',
  'executor_trace_replay',
  'forum_publication',
  'model_lab_inspection',
  'provider_mutation',
  'pylon_triage',
  'runtime_promotion',
  'status_projection',
  'training_launch',
  'wallet_spend',
])
export type ArtanisActionKind = typeof ArtanisActionKind.Type

export const ArtanisActionRisk = S.Literals([
  'approval_required',
  'blocked',
  'safe',
])
export type ArtanisActionRisk = typeof ArtanisActionRisk.Type

export const ArtanisApprovalState = S.Literals([
  'approved',
  'denied',
  'expired',
  'pending',
])
export type ArtanisApprovalState = typeof ArtanisApprovalState.Type

export const ArtanisLoopAuthorityBoundary = S.Literals([
  'read_only_artanis_loop',
])
export type ArtanisLoopAuthorityBoundary =
  typeof ArtanisLoopAuthorityBoundary.Type

export class ArtanisLoopAuthority extends S.Class<ArtanisLoopAuthority>(
  'ArtanisLoopAuthority',
)({
  authorityBoundary: ArtanisLoopAuthorityBoundary,
  noDeployment: S.Boolean,
  noEvalLaunch: S.Boolean,
  noForumPublish: S.Boolean,
  noPaymentSpend: S.Boolean,
  noProviderMutation: S.Boolean,
  noRuntimePromotion: S.Boolean,
  noTrainingLaunch: S.Boolean,
  noWalletSpend: S.Boolean,
}) {}

export class ArtanisActionProposalRecord extends S.Class<ArtanisActionProposalRecord>(
  'ArtanisActionProposalRecord',
)({
  actionRef: S.String,
  approvalRequirementRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  kind: ArtanisActionKind,
  risk: ArtanisActionRisk,
}) {}

export class ArtanisApprovalRequirementRecord extends S.Class<ArtanisApprovalRequirementRecord>(
  'ArtanisApprovalRequirementRecord',
)({
  actionRef: S.String,
  approvalRef: S.String,
  authorityRef: S.String,
  caveatRefs: S.Array(S.String),
  expiresAtIso: S.NullOr(S.String),
  state: ArtanisApprovalState,
}) {}

export class ArtanisLoopTickRecord extends S.Class<ArtanisLoopTickRecord>(
  'ArtanisLoopTickRecord',
)({
  actionProposals: S.Array(ArtanisActionProposalRecord),
  approvalRequirements: S.Array(ArtanisApprovalRequirementRecord),
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  createdAtIso: S.String,
  forumPublicationIntentRefs: S.Array(S.String),
  goalRef: S.String,
  idempotencyKey: S.String,
  loopRef: S.String,
  nextTickAtIso: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  selectedContextRefs: S.Array(S.String),
  state: ArtanisLoopState,
  tickRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisLoopRecord extends S.Class<ArtanisLoopRecord>(
  'ArtanisLoopRecord',
)({
  active: S.Boolean,
  agentId: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  goalRefs: S.Array(S.String),
  loopRef: S.String,
  scopeRef: S.String,
  state: ArtanisLoopState,
  ticks: S.Array(ArtanisLoopTickRecord),
  updatedAtIso: S.String,
}) {}

export class ArtanisLoopLedgerRecord extends S.Class<ArtanisLoopLedgerRecord>(
  'ArtanisLoopLedgerRecord',
)({
  agentId: S.String,
  authority: ArtanisLoopAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  ledgerRef: S.String,
  loops: S.Array(ArtanisLoopRecord),
  updatedAtIso: S.String,
}) {}

export class ArtanisLoopTickProjection extends S.Class<ArtanisLoopTickProjection>(
  'ArtanisLoopTickProjection',
)({
  actionProposals: S.Array(ArtanisActionProposalRecord),
  approvalRequirements: S.Array(ArtanisApprovalRequirementRecord),
  artifactRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  forumPublicationIntentRefs: S.Array(S.String),
  goalRef: S.String,
  idempotencyKey: S.String,
  loopRef: S.String,
  nextTickDisplay: S.NullOr(S.String),
  receiptRefs: S.Array(S.String),
  selectedContextRefs: S.Array(S.String),
  state: ArtanisLoopState,
  tickRef: S.String,
  updatedAtDisplay: S.String,
}) {}

export class ArtanisLoopProjection extends S.Class<ArtanisLoopProjection>(
  'ArtanisLoopProjection',
)({
  active: S.Boolean,
  agentId: S.String,
  blockerRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  duplicateTickRefs: S.Array(S.String),
  goalRefs: S.Array(S.String),
  loopRef: S.String,
  scopeRef: S.String,
  state: ArtanisLoopState,
  tickCount: S.Number,
  ticks: S.Array(ArtanisLoopTickProjection),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisLoopLedgerProjection extends S.Class<ArtanisLoopLedgerProjection>(
  'ArtanisLoopLedgerProjection',
)({
  agentId: S.String,
  audience: ArtanisLoopAudience,
  authority: ArtanisLoopAuthority,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  deploymentAllowed: S.Boolean,
  evalLaunchAllowed: S.Boolean,
  forumPublishAllowed: S.Boolean,
  ledgerRef: S.String,
  loopCount: S.Number,
  loops: S.Array(ArtanisLoopProjection),
  paymentSpendAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  runtimePromotionAllowed: S.Boolean,
  trainingLaunchAllowed: S.Boolean,
  updatedAtDisplay: S.String,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisLoopUnsafe extends S.TaggedErrorClass<ArtanisLoopUnsafe>()(
  'ArtanisLoopUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_LOOP_READ_ONLY_AUTHORITY: ArtanisLoopAuthority = {
  authorityBoundary: 'read_only_artanis_loop',
  noDeployment: true,
  noEvalLaunch: true,
  noForumPublish: true,
  noPaymentSpend: true,
  noProviderMutation: true,
  noRuntimePromotion: true,
  noTrainingLaunch: true,
  noWalletSpend: true,
}

const riskyActionKinds: ReadonlyArray<ArtanisActionKind> = [
  'eval_launch',
  'forum_publication',
  'provider_mutation',
  'runtime_promotion',
  'training_launch',
  'wallet_spend',
]
const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeLoopRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(action\.private|approval\.private|artifact\.private|blocker\.private|caveat\.private|context\.private|evidence\.private|forum\.private|goal\.private|ledger\.private|loop\.private|receipt\.private|scope\.private|tick\.private)/i
const agentUnsafeRefPattern =
  /(action\.private|approval\.private|artifact\.private|context\.private|evidence\.private|goal\.private|ledger\.private|loop\.private|receipt\.private|scope\.private|tick\.private)/i
const customerUnsafeRefPattern = agentUnsafeRefPattern

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeLoopRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisLoopUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw timestamp material.`,
    })
  }
}

const audienceUnsafePattern = (audience: ArtanisLoopAudience): RegExp | null => {
  if (audience === 'public') {
    return publicUnsafeRefPattern
  }

  if (audience === 'agent') {
    return agentUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  return null
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisLoopAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const refForAudience = (
  label: string,
  ref: string,
  audience: ArtanisLoopAudience,
  redactedRef: string,
): string => refsForAudience(label, [ref], audience)[0] ?? redactedRef

const assertReadOnlyAuthority = (authority: ArtanisLoopAuthority): void => {
  if (
    authority.noDeployment !== true ||
    authority.noEvalLaunch !== true ||
    authority.noForumPublish !== true ||
    authority.noPaymentSpend !== true ||
    authority.noProviderMutation !== true ||
    authority.noRuntimePromotion !== true ||
    authority.noTrainingLaunch !== true ||
    authority.noWalletSpend !== true
  ) {
    throw new ArtanisLoopUnsafe({
      reason:
        'Artanis loop records are not authority to deploy, launch evals or training, publish Forum posts, spend, mutate providers, promote runtime behavior, or spend from wallets.',
    })
  }
}

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisLoopUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertMaybeIso = (label: string, iso: string | null): void => {
  if (iso !== null) {
    assertValidIso(label, iso)
  }
}

const assertActionProposal = (
  action: ArtanisActionProposalRecord,
): void => {
  assertSafeRefs('Artanis action ref', [action.actionRef])
  assertSafeRefs(
    'Artanis action approval requirement refs',
    action.approvalRequirementRefs,
  )
  assertSafeRefs('Artanis action artifact refs', action.artifactRefs)
  assertSafeRefs(
    'Artanis action authority receipt refs',
    action.authorityReceiptRefs,
  )
  assertSafeRefs('Artanis action caveat refs', action.caveatRefs)
  assertSafeRefs('Artanis action evidence refs', action.evidenceRefs)

  if (
    riskyActionKinds.includes(action.kind) &&
    action.risk === 'safe'
  ) {
    throw new ArtanisLoopUnsafe({
      reason: 'Risky Artanis action kinds cannot be marked safe.',
    })
  }

  if (
    riskyActionKinds.includes(action.kind) &&
    (!hasAny(action.approvalRequirementRefs) ||
      !hasAny(action.authorityReceiptRefs))
  ) {
    throw new ArtanisLoopUnsafe({
      reason:
        'Risky Artanis action proposals require approval requirements and separate authority receipt refs.',
    })
  }
}

const assertApprovalRequirement = (
  approval: ArtanisApprovalRequirementRecord,
): void => {
  assertMaybeIso('Artanis approval expiry', approval.expiresAtIso)
  assertSafeRefs('Artanis approval action ref', [approval.actionRef])
  assertSafeRefs('Artanis approval ref', [approval.approvalRef])
  assertSafeRefs('Artanis approval authority ref', [approval.authorityRef])
  assertSafeRefs('Artanis approval caveat refs', approval.caveatRefs)

  if (
    approval.state === 'pending' &&
    !hasAny(approval.caveatRefs)
  ) {
    throw new ArtanisLoopUnsafe({
      reason: 'Pending Artanis approvals require caveat refs.',
    })
  }
}

const assertTick = (tick: ArtanisLoopTickRecord): void => {
  assertValidIso('tick.createdAtIso', tick.createdAtIso)
  assertValidIso('tick.updatedAtIso', tick.updatedAtIso)
  assertMaybeIso('tick.nextTickAtIso', tick.nextTickAtIso)
  assertSafeRefs('Artanis tick ref', [tick.tickRef])
  assertSafeRefs('Artanis tick idempotency key', [tick.idempotencyKey])
  assertSafeRefs('Artanis tick loop ref', [tick.loopRef])
  assertSafeRefs('Artanis tick goal ref', [tick.goalRef])
  assertSafeRefs('Artanis selected context refs', tick.selectedContextRefs)
  assertSafeRefs('Artanis blocker refs', tick.blockerRefs)
  assertSafeRefs('Artanis tick caveat refs', tick.caveatRefs)
  assertSafeRefs('Artanis tick receipt refs', tick.receiptRefs)
  assertSafeRefs('Artanis closeout receipt refs', tick.closeoutReceiptRefs)
  assertSafeRefs('Artanis artifact refs', tick.artifactRefs)
  assertSafeRefs(
    'Artanis Forum publication intent refs',
    tick.forumPublicationIntentRefs,
  )
  tick.actionProposals.forEach(assertActionProposal)
  tick.approvalRequirements.forEach(assertApprovalRequirement)

  if (tick.state === 'blocked' && !hasAny(tick.blockerRefs)) {
    throw new ArtanisLoopUnsafe({
      reason: 'Blocked Artanis ticks require blocker refs.',
    })
  }

  if (
    tick.state === 'waiting_for_approval' &&
    !hasAny(tick.approvalRequirements)
  ) {
    throw new ArtanisLoopUnsafe({
      reason: 'Approval-waiting Artanis ticks require approval requirements.',
    })
  }

  if (
    tick.state === 'completed' &&
    (!hasAny(tick.closeoutReceiptRefs) ||
      !hasAny(tick.artifactRefs) ||
      !hasAny(tick.forumPublicationIntentRefs) ||
      tick.nextTickAtIso === null)
  ) {
    throw new ArtanisLoopUnsafe({
      reason:
        'Completed Artanis ticks require closeout receipts, artifacts, Forum publication intents, and a next tick schedule.',
    })
  }
}

const canonicalTicks = (
  ticks: ReadonlyArray<ArtanisLoopTickRecord>,
): ReadonlyArray<ArtanisLoopTickRecord> =>
  ticks.filter(
    (tick, index) =>
      ticks.findIndex(other => other.idempotencyKey === tick.idempotencyKey) ===
      index,
  )

const duplicateTickRefs = (
  ticks: ReadonlyArray<ArtanisLoopTickRecord>,
): ReadonlyArray<string> =>
  uniqueRefs(
    ticks
      .filter(
        (tick, index) =>
          ticks.findIndex(
            other => other.idempotencyKey === tick.idempotencyKey,
          ) !== index,
      )
      .map(tick => tick.tickRef),
  )

const assertLoop = (loop: ArtanisLoopRecord): void => {
  assertValidIso('loop.createdAtIso', loop.createdAtIso)
  assertValidIso('loop.updatedAtIso', loop.updatedAtIso)
  assertSafeRefs('Artanis loop agent id', [loop.agentId])
  assertSafeRefs('Artanis loop ref', [loop.loopRef])
  assertSafeRefs('Artanis scope ref', [loop.scopeRef])
  assertSafeRefs('Artanis loop goal refs', loop.goalRefs)
  assertSafeRefs('Artanis loop blocker refs', loop.blockerRefs)
  assertSafeRefs('Artanis loop caveat refs', loop.caveatRefs)

  if (loop.agentId !== 'agent_artanis') {
    throw new ArtanisLoopUnsafe({
      reason: 'Artanis loops must use agent_artanis.',
    })
  }

  if (!hasAny(loop.goalRefs) || !hasAny(loop.ticks)) {
    throw new ArtanisLoopUnsafe({
      reason: 'Artanis loops require goal refs and ticks.',
    })
  }

  if (loop.state === 'blocked' && !hasAny(loop.blockerRefs)) {
    throw new ArtanisLoopUnsafe({
      reason: 'Blocked Artanis loops require blocker refs.',
    })
  }

  loop.ticks.forEach(tick => {
    assertTick(tick)

    if (tick.loopRef !== loop.loopRef) {
      throw new ArtanisLoopUnsafe({
        reason: 'Artanis ticks must reference their parent loop.',
      })
    }
  })
}

const assertLedger = (ledger: ArtanisLoopLedgerRecord): void => {
  assertReadOnlyAuthority(ledger.authority)
  assertValidIso('createdAtIso', ledger.createdAtIso)
  assertValidIso('updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Artanis loop ledger agent id', [ledger.agentId])
  assertSafeRefs('Artanis loop ledger ref', [ledger.ledgerRef])
  assertSafeRefs('Artanis loop ledger caveat refs', ledger.caveatRefs)

  if (ledger.agentId !== 'agent_artanis') {
    throw new ArtanisLoopUnsafe({
      reason: 'Artanis loop ledgers must use agent_artanis.',
    })
  }

  if (!hasAny(ledger.loops)) {
    throw new ArtanisLoopUnsafe({
      reason: 'Artanis loop ledgers require loops.',
    })
  }

  ledger.loops.forEach(assertLoop)

  const activeScopes = ledger.loops
    .filter(loop => loop.active)
    .map(loop => loop.scopeRef)

  if (hasAny(activeScopes.filter((scope, index) => activeScopes.indexOf(scope) !== index))) {
    throw new ArtanisLoopUnsafe({
      reason: 'Only one active Artanis loop per scope is allowed.',
    })
  }
}

const redactActionProposal = (
  action: ArtanisActionProposalRecord,
  audience: ArtanisLoopAudience,
): ArtanisActionProposalRecord => ({
  ...action,
  approvalRequirementRefs: refsForAudience(
    'Artanis action approval requirement refs',
    action.approvalRequirementRefs,
    audience,
  ),
  artifactRefs: refsForAudience(
    'Artanis action artifact refs',
    action.artifactRefs,
    audience,
  ),
  authorityReceiptRefs: refsForAudience(
    'Artanis action authority receipt refs',
    action.authorityReceiptRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Artanis action caveat refs',
    action.caveatRefs,
    audience,
  ),
  evidenceRefs: refsForAudience(
    'Artanis action evidence refs',
    action.evidenceRefs,
    audience,
  ),
  actionRef: refForAudience(
    'Artanis action ref',
    action.actionRef,
    audience,
    'action.redacted.artanis_loop',
  ),
})

const redactApproval = (
  approval: ArtanisApprovalRequirementRecord,
  audience: ArtanisLoopAudience,
): ArtanisApprovalRequirementRecord => ({
  ...approval,
  actionRef: refForAudience(
    'Artanis approval action ref',
    approval.actionRef,
    audience,
    'action.redacted.artanis_loop',
  ),
  approvalRef: refForAudience(
    'Artanis approval ref',
    approval.approvalRef,
    audience,
    'approval.redacted.artanis_loop',
  ),
  authorityRef: refForAudience(
    'Artanis approval authority ref',
    approval.authorityRef,
    audience,
    'authority.redacted.artanis_loop',
  ),
  caveatRefs: refsForAudience(
    'Artanis approval caveat refs',
    approval.caveatRefs,
    audience,
  ),
  expiresAtIso: audience === 'operator' || audience === 'team'
    ? approval.expiresAtIso
    : null,
})

const projectTick = (
  tick: ArtanisLoopTickRecord,
  audience: ArtanisLoopAudience,
  nowIso: string,
): ArtanisLoopTickProjection => ({
  actionProposals: tick.actionProposals.map(action =>
    redactActionProposal(action, audience),
  ),
  approvalRequirements: tick.approvalRequirements.map(approval =>
    redactApproval(approval, audience),
  ),
  artifactRefs: refsForAudience(
    'Artanis tick artifact refs',
    tick.artifactRefs,
    audience,
  ),
  blockerRefs: refsForAudience(
    'Artanis tick blocker refs',
    tick.blockerRefs,
    audience,
  ),
  caveatRefs: refsForAudience(
    'Artanis tick caveat refs',
    tick.caveatRefs,
    audience,
  ),
  closeoutReceiptRefs: refsForAudience(
    'Artanis closeout receipt refs',
    tick.closeoutReceiptRefs,
    audience,
  ),
  createdAtDisplay: friendlyBlueprintMissionBriefingTime(
    tick.createdAtIso,
    nowIso,
  ),
  forumPublicationIntentRefs: refsForAudience(
    'Artanis Forum publication intent refs',
    tick.forumPublicationIntentRefs,
    audience,
  ),
  goalRef: refForAudience(
    'Artanis tick goal ref',
    tick.goalRef,
    audience,
    'goal.redacted.artanis_loop',
  ),
  idempotencyKey: refForAudience(
    'Artanis tick idempotency key',
    tick.idempotencyKey,
    audience,
    'idempotency.redacted.artanis_loop',
  ),
  loopRef: refForAudience(
    'Artanis tick loop ref',
    tick.loopRef,
    audience,
    'loop.redacted.artanis_loop',
  ),
  nextTickDisplay: tick.nextTickAtIso === null
    ? null
    : friendlyBlueprintMissionBriefingTime(tick.nextTickAtIso, nowIso),
  receiptRefs: refsForAudience(
    'Artanis tick receipt refs',
    tick.receiptRefs,
    audience,
  ),
  selectedContextRefs: refsForAudience(
    'Artanis selected context refs',
    tick.selectedContextRefs,
    audience,
  ),
  state: tick.state,
  tickRef: refForAudience(
    'Artanis tick ref',
    tick.tickRef,
    audience,
    'tick.redacted.artanis_loop',
  ),
  updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
    tick.updatedAtIso,
    nowIso,
  ),
})

const projectLoop = (
  loop: ArtanisLoopRecord,
  audience: ArtanisLoopAudience,
  nowIso: string,
): ArtanisLoopProjection => {
  const ticks = canonicalTicks(loop.ticks)

  return {
    active: loop.active,
    agentId: loop.agentId,
    blockerRefs: refsForAudience(
      'Artanis loop blocker refs',
      loop.blockerRefs,
      audience,
    ),
    caveatRefs: refsForAudience(
      'Artanis loop caveat refs',
      loop.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      loop.createdAtIso,
      nowIso,
    ),
    duplicateTickRefs: refsForAudience(
      'Artanis duplicate tick refs',
      duplicateTickRefs(loop.ticks),
      audience,
    ),
    goalRefs: refsForAudience('Artanis loop goal refs', loop.goalRefs, audience),
    loopRef: refForAudience(
      'Artanis loop ref',
      loop.loopRef,
      audience,
      'loop.redacted.artanis_loop',
    ),
    scopeRef: refForAudience(
      'Artanis scope ref',
      loop.scopeRef,
      audience,
      'scope.redacted.artanis_loop',
    ),
    state: loop.state,
    tickCount: ticks.length,
    ticks: ticks.map(tick => projectTick(tick, audience, nowIso)),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      loop.updatedAtIso,
      nowIso,
    ),
  }
}

export const projectArtanisLoopLedger = (
  ledger: ArtanisLoopLedgerRecord,
  audience: ArtanisLoopAudience,
  nowIso: string,
): ArtanisLoopLedgerProjection => {
  assertLedger(ledger)

  return {
    agentId: ledger.agentId,
    audience,
    authority: ledger.authority,
    caveatRefs: refsForAudience(
      'Artanis loop ledger caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    deploymentAllowed: !ledger.authority.noDeployment,
    evalLaunchAllowed: !ledger.authority.noEvalLaunch,
    forumPublishAllowed: !ledger.authority.noForumPublish,
    ledgerRef: refForAudience(
      'Artanis loop ledger ref',
      ledger.ledgerRef,
      audience,
      'ledger.redacted.artanis_loop',
    ),
    loopCount: ledger.loops.length,
    loops: ledger.loops.map(loop => projectLoop(loop, audience, nowIso)),
    paymentSpendAllowed: !ledger.authority.noPaymentSpend,
    providerMutationAllowed: !ledger.authority.noProviderMutation,
    runtimePromotionAllowed: !ledger.authority.noRuntimePromotion,
    trainingLaunchAllowed: !ledger.authority.noTrainingLaunch,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
    walletSpendAllowed: !ledger.authority.noWalletSpend,
  }
}

const projectionStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionStringValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionStringValues)
  }

  return []
}

const allowedProjectionLiteralValues = new Set<string>([
  'agent',
  'approval_required',
  'approved',
  'blocked',
  'completed',
  'customer',
  'denied',
  'eval_launch',
  'expired',
  'executor_trace_replay',
  'failed',
  'forum_publication',
  'model_lab_inspection',
  'operator',
  'paused',
  'pending',
  'provider_mutation',
  'public',
  'pylon_triage',
  'queued',
  'read_only_artanis_loop',
  'running',
  'runtime_promotion',
  'safe',
  'status_projection',
  'team',
  'training_launch',
  'waiting_for_approval',
  'wallet_spend',
])

export const artanisLoopProjectionHasPrivateMaterial = (
  projection: ArtanisLoopLedgerProjection,
): boolean =>
  projectionStringValues(projection).some(
    value =>
      !allowedProjectionLiteralValues.has(value) &&
      (unsafeLoopRefPattern.test(value) || rawTimestampPattern.test(value)),
  )

export const exampleArtanisLoopLedger = (): ArtanisLoopLedgerRecord => ({
  agentId: 'agent_artanis',
  authority: ARTANIS_LOOP_READ_ONLY_AUTHORITY,
  caveatRefs: ['caveat.public.artanis_loop_evidence_only'],
  createdAtIso: '2026-06-07T00:50:00.000Z',
  ledgerRef: 'ledger.public.artanis.autonomous_loop',
  loops: [
    {
      active: true,
      agentId: 'agent_artanis',
      blockerRefs: [],
      caveatRefs: ['caveat.public.loop_does_not_execute_risky_actions'],
      createdAtIso: '2026-06-07T00:51:00.000Z',
      goalRefs: ['goal.public.artanis.pylon_model_lab'],
      loopRef: 'loop.public.artanis.primary',
      scopeRef: 'scope.public.artanis.global',
      state: 'running',
      ticks: [
        {
          actionProposals: [
            {
              actionRef: 'action.public.artanis.status_projection',
              approvalRequirementRefs: [],
              artifactRefs: ['artifact.public.artanis.status_packet'],
              authorityReceiptRefs: [],
              caveatRefs: ['caveat.public.safe_status_projection'],
              evidenceRefs: ['evidence.public.artanis.loop_context'],
              kind: 'status_projection',
              risk: 'safe',
            },
          ],
          approvalRequirements: [],
          artifactRefs: ['artifact.public.artanis.status_packet'],
          blockerRefs: [],
          caveatRefs: ['caveat.public.tick_evidence_only'],
          closeoutReceiptRefs: ['receipt.public.artanis.tick_closeout'],
          createdAtIso: '2026-06-07T00:52:00.000Z',
          forumPublicationIntentRefs: ['forum.public.artanis.status_intent'],
          goalRef: 'goal.public.artanis.pylon_model_lab',
          idempotencyKey: 'tick.public.artanis.20260607T0052',
          loopRef: 'loop.public.artanis.primary',
          nextTickAtIso: '2026-06-07T01:10:00.000Z',
          receiptRefs: ['receipt.public.artanis.context_loaded'],
          selectedContextRefs: [
            'context.public.artanis.pylon_readiness',
            'context.public.artanis.model_lab_report',
          ],
          state: 'completed',
          tickRef: 'tick.public.artanis.20260607T0052',
          updatedAtIso: '2026-06-07T00:56:00.000Z',
        },
      ],
      updatedAtIso: '2026-06-07T00:56:00.000Z',
    },
  ],
  updatedAtIso: '2026-06-07T00:56:00.000Z',
})
