import { Schema as S } from 'effect'

import { TASSADAR_EXECUTOR_CAPABILITY_REF } from '@openagentsinc/tassadar-executor'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisWorkRoutingAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisWorkRoutingAudience =
  typeof ArtanisWorkRoutingAudience.Type

export const ArtanisWorkRoutingTarget = S.Literals([
  'benchmark_cloud',
  'model_lab',
  'nexus',
  'probe',
  'psionic',
  'pylon',
  'runner',
])
export type ArtanisWorkRoutingTarget = typeof ArtanisWorkRoutingTarget.Type

export const ArtanisWorkRoutingCapability = S.Literals([
  'artifact_validation',
  'benchmark_evaluation',
  'coding_runtime_probe',
  'embedding_data_prep',
  'executor_trace_validation',
  'gepa_dspy_optimization',
  'inference',
  'lora_finetuning',
  'model_lab_evidence_review',
  'nexus_assignment',
  'pylon_training',
  'psionic_adapter_validation',
])
export type ArtanisWorkRoutingCapability =
  typeof ArtanisWorkRoutingCapability.Type

export const ArtanisWorkRoutingWorkClass = S.Literals([
  'benchmark_evaluation',
  'embedding_data_prep',
  'executor_trace_validation',
  'gepa_dspy_optimization',
  'inference',
  'lora_finetuning',
  'training',
  'validation',
])
export type ArtanisWorkRoutingWorkClass =
  typeof ArtanisWorkRoutingWorkClass.Type

export const ArtanisWorkRoutingResourceMode = S.Literals([
  'background',
  'dedicated',
  'not_applicable',
  'operator_selected',
  'overnight',
])
export type ArtanisWorkRoutingResourceMode =
  typeof ArtanisWorkRoutingResourceMode.Type

export const ArtanisWorkRoutingRisk = S.Literals([
  'approval_required',
  'blocked',
  'prohibited',
  'safe_read_only',
])
export type ArtanisWorkRoutingRisk = typeof ArtanisWorkRoutingRisk.Type

export const ArtanisWorkRoutingState = S.Literals([
  'accepted',
  'blocked',
  'completed',
  'dispatched',
  'proposed',
  'rejected',
])
export type ArtanisWorkRoutingState = typeof ArtanisWorkRoutingState.Type

export class ArtanisWorkRoutingAuthority extends S.Class<ArtanisWorkRoutingAuthority>(
  'ArtanisWorkRoutingAuthority',
)({
  dispatchAllowed: S.Boolean,
  providerMutationAllowed: S.Boolean,
  runtimeMutationAllowed: S.Boolean,
  settlementMutationAllowed: S.Boolean,
  walletSpendAllowed: S.Boolean,
}) {}

export class ArtanisWorkRoutingProposalRecord extends S.Class<ArtanisWorkRoutingProposalRecord>(
  'ArtanisWorkRoutingProposalRecord',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  approvalRequirementRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  capability: ArtanisWorkRoutingCapability,
  costCaveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  decidedAtIso: S.NullOr(S.String),
  operatorDetailRefs: S.Array(S.String),
  proposalRef: S.String,
  publicCaveatRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  resourceMode: ArtanisWorkRoutingResourceMode,
  risk: ArtanisWorkRoutingRisk,
  sourceEvidenceRefs: S.Array(S.String),
  spendLimitRefs: S.Array(S.String),
  state: ArtanisWorkRoutingState,
  target: ArtanisWorkRoutingTarget,
  targetCapabilityRefs: S.Array(S.String),
  traceableWorkRefs: S.Array(S.String),
  updatedAtIso: S.String,
  workClass: ArtanisWorkRoutingWorkClass,
}) {}

export class ArtanisWorkRoutingLedgerRecord extends S.Class<ArtanisWorkRoutingLedgerRecord>(
  'ArtanisWorkRoutingLedgerRecord',
)({
  agentId: S.String,
  authority: ArtanisWorkRoutingAuthority,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  ledgerRef: S.String,
  proposals: S.Array(ArtanisWorkRoutingProposalRecord),
  publicStatusRefs: S.Array(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisWorkRoutingProposalProjection extends S.Class<ArtanisWorkRoutingProposalProjection>(
  'ArtanisWorkRoutingProposalProjection',
)({
  acceptanceCriteriaRefs: S.Array(S.String),
  approvalRequirementRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
  capability: ArtanisWorkRoutingCapability,
  costCaveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  decidedAtDisplay: S.NullOr(S.String),
  operatorDetailRefs: S.Array(S.String),
  proposalRef: S.String,
  publicCaveatRefs: S.Array(S.String),
  receiptRefs: S.Array(S.String),
  resourceMode: ArtanisWorkRoutingResourceMode,
  risk: ArtanisWorkRoutingRisk,
  sourceEvidenceRefs: S.Array(S.String),
  spendLimitRefs: S.Array(S.String),
  state: ArtanisWorkRoutingState,
  target: ArtanisWorkRoutingTarget,
  targetCapabilityRefs: S.Array(S.String),
  traceableWorkRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
  workClass: ArtanisWorkRoutingWorkClass,
}) {}

export class ArtanisWorkRoutingLedgerProjection extends S.Class<ArtanisWorkRoutingLedgerProjection>(
  'ArtanisWorkRoutingLedgerProjection',
)({
  agentId: S.String,
  audience: ArtanisWorkRoutingAudience,
  authority: ArtanisWorkRoutingAuthority,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  ledgerRef: S.String,
  proposalCount: S.Number,
  proposals: S.Array(ArtanisWorkRoutingProposalProjection),
  publicStatusRefs: S.Array(S.String),
  riskyProposalRefs: S.Array(S.String),
  traceableWorkRefs: S.Array(S.String),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisWorkRoutingUnsafe extends S.TaggedErrorClass<ArtanisWorkRoutingUnsafe>()(
  'ArtanisWorkRoutingUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_WORK_ROUTING_WORK_CLASSES: ReadonlyArray<ArtanisWorkRoutingWorkClass> =
  [
    'benchmark_evaluation',
    'embedding_data_prep',
    'executor_trace_validation',
    'gepa_dspy_optimization',
    'inference',
    'lora_finetuning',
    'training',
    'validation',
  ]

export const ARTANIS_WORK_ROUTING_CAPABILITIES: ReadonlyArray<ArtanisWorkRoutingCapability> =
  [
    'artifact_validation',
    'benchmark_evaluation',
    'coding_runtime_probe',
    'embedding_data_prep',
    'executor_trace_validation',
    'gepa_dspy_optimization',
    'inference',
    'lora_finetuning',
    'model_lab_evidence_review',
    'nexus_assignment',
    'pylon_training',
    'psionic_adapter_validation',
  ]

export const ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY: ArtanisWorkRoutingAuthority =
  {
    dispatchAllowed: false,
    providerMutationAllowed: false,
    runtimeMutationAllowed: false,
    settlementMutationAllowed: false,
    walletSpendAllowed: false,
  }

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeRoutingPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(evidence\.private|operator\.|receipt\.operator|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisWorkRoutingUnsafe({
      reason: `${label} must be a valid ISO timestamp.`,
    })
  }
}

const assertMaybeIso = (label: string, iso: string | null): void => {
  if (iso !== null) {
    assertValidIso(label, iso)
  }
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(
    ref =>
      !safeRefPattern.test(ref) ||
      unsafeRoutingPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisWorkRoutingUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisWorkRoutingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const assertNoDirectAuthority = (
  authority: ArtanisWorkRoutingAuthority,
): void => {
  if (
    authority.dispatchAllowed ||
    authority.providerMutationAllowed ||
    authority.runtimeMutationAllowed ||
    authority.settlementMutationAllowed ||
    authority.walletSpendAllowed
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Artanis work-routing proposals are not authority to dispatch, mutate providers, spend from wallets, mutate settlement, or mutate runtime state.',
    })
  }
}

const assertProposal = (proposal: ArtanisWorkRoutingProposalRecord): void => {
  assertValidIso('proposal.createdAtIso', proposal.createdAtIso)
  assertValidIso('proposal.updatedAtIso', proposal.updatedAtIso)
  assertMaybeIso('proposal.decidedAtIso', proposal.decidedAtIso)
  assertSafeRefs('Artanis work-routing proposal ref', [proposal.proposalRef])
  assertSafeRefs(
    'Artanis work-routing acceptance criteria refs',
    proposal.acceptanceCriteriaRefs,
  )
  assertSafeRefs(
    'Artanis work-routing approval requirement refs',
    proposal.approvalRequirementRefs,
  )
  assertSafeRefs('Artanis work-routing blocker refs', proposal.blockerRefs)
  assertSafeRefs(
    'Artanis work-routing cost caveat refs',
    proposal.costCaveatRefs,
  )
  assertSafeRefs(
    'Artanis work-routing operator detail refs',
    proposal.operatorDetailRefs,
  )
  assertSafeRefs(
    'Artanis work-routing public caveat refs',
    proposal.publicCaveatRefs,
  )
  assertSafeRefs('Artanis work-routing receipt refs', proposal.receiptRefs)
  assertSafeRefs(
    'Artanis work-routing source evidence refs',
    proposal.sourceEvidenceRefs,
  )
  assertSafeRefs(
    'Artanis work-routing spend limit refs',
    proposal.spendLimitRefs,
  )
  assertSafeRefs(
    'Artanis work-routing target capability refs',
    proposal.targetCapabilityRefs,
  )
  assertSafeRefs(
    'Artanis work-routing traceable work refs',
    proposal.traceableWorkRefs,
  )

  if (!hasAny(proposal.sourceEvidenceRefs)) {
    throw new ArtanisWorkRoutingUnsafe({
      reason: 'Artanis work-routing proposals require source evidence refs.',
    })
  }

  if (
    !hasAny(proposal.targetCapabilityRefs) ||
    !hasAny(proposal.acceptanceCriteriaRefs)
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Artanis work-routing proposals require target capability and acceptance criteria refs.',
    })
  }

  if (
    proposal.risk === 'approval_required' &&
    (!hasAny(proposal.approvalRequirementRefs) ||
      !hasAny(proposal.costCaveatRefs) ||
      !hasAny(proposal.spendLimitRefs))
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Approval-required Artanis work-routing proposals require approval, spend-limit, and cost-caveat refs.',
    })
  }

  if (
    (proposal.state === 'accepted' ||
      proposal.state === 'dispatched' ||
      proposal.state === 'completed') &&
    (!hasAny(proposal.traceableWorkRefs) || !hasAny(proposal.receiptRefs))
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Accepted, dispatched, and completed Artanis proposals require traceable work refs and receipts.',
    })
  }

  if (
    (proposal.state === 'blocked' || proposal.state === 'rejected') &&
    (!hasAny(proposal.blockerRefs) || !hasAny(proposal.publicCaveatRefs))
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Blocked or rejected Artanis proposals require public-safe blockers and caveats.',
    })
  }

  if (proposal.state !== 'proposed' && proposal.decidedAtIso === null) {
    throw new ArtanisWorkRoutingUnsafe({
      reason: 'Decided Artanis work-routing proposals require decidedAtIso.',
    })
  }
}

const assertLedger = (ledger: ArtanisWorkRoutingLedgerRecord): void => {
  assertValidIso('ledger.createdAtIso', ledger.createdAtIso)
  assertValidIso('ledger.updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Artanis work-routing agent id', [ledger.agentId])
  assertSafeRefs('Artanis work-routing ledger ref', [ledger.ledgerRef])
  assertSafeRefs('Artanis work-routing caveat refs', ledger.caveatRefs)
  assertSafeRefs(
    'Artanis work-routing public status refs',
    ledger.publicStatusRefs,
  )
  assertNoDirectAuthority(ledger.authority)

  if (ledger.agentId !== 'agent_artanis') {
    throw new ArtanisWorkRoutingUnsafe({
      reason: 'Artanis work-routing ledgers must use agent_artanis.',
    })
  }

  if (!hasAny(ledger.proposals)) {
    throw new ArtanisWorkRoutingUnsafe({
      reason: 'Artanis work-routing ledgers require at least one proposal.',
    })
  }

  ledger.proposals.forEach(assertProposal)
}

const projectProposal = (
  proposal: ArtanisWorkRoutingProposalRecord,
  audience: ArtanisWorkRoutingAudience,
  nowIso: string,
): ArtanisWorkRoutingProposalProjection =>
  new ArtanisWorkRoutingProposalProjection({
    acceptanceCriteriaRefs: refsForAudience(
      'Artanis work-routing acceptance criteria refs',
      proposal.acceptanceCriteriaRefs,
      audience,
    ),
    approvalRequirementRefs: refsForAudience(
      'Artanis work-routing approval requirement refs',
      proposal.approvalRequirementRefs,
      audience,
    ),
    blockerRefs: refsForAudience(
      'Artanis work-routing blocker refs',
      proposal.blockerRefs,
      audience,
    ),
    capability: proposal.capability,
    costCaveatRefs: refsForAudience(
      'Artanis work-routing cost caveat refs',
      proposal.costCaveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      proposal.createdAtIso,
      nowIso,
    ),
    decidedAtDisplay: proposal.decidedAtIso === null
      ? null
      : friendlyBlueprintMissionBriefingTime(proposal.decidedAtIso, nowIso),
    operatorDetailRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis work-routing operator detail refs',
          proposal.operatorDetailRefs,
          audience,
        )
      : [],
    proposalRef: refsForAudience(
      'Artanis work-routing proposal ref',
      [proposal.proposalRef],
      audience,
    )[0] ?? 'work_routing.redacted.artanis_proposal',
    publicCaveatRefs: refsForAudience(
      'Artanis work-routing public caveat refs',
      proposal.publicCaveatRefs,
      audience,
    ),
    receiptRefs: refsForAudience(
      'Artanis work-routing receipt refs',
      proposal.receiptRefs,
      audience,
    ),
    resourceMode: proposal.resourceMode,
    risk: proposal.risk,
    sourceEvidenceRefs: refsForAudience(
      'Artanis work-routing source evidence refs',
      proposal.sourceEvidenceRefs,
      audience,
    ),
    spendLimitRefs: refsForAudience(
      'Artanis work-routing spend limit refs',
      proposal.spendLimitRefs,
      audience,
    ),
    state: proposal.state,
    target: proposal.target,
    targetCapabilityRefs: refsForAudience(
      'Artanis work-routing target capability refs',
      proposal.targetCapabilityRefs,
      audience,
    ),
    traceableWorkRefs: refsForAudience(
      'Artanis work-routing traceable work refs',
      proposal.traceableWorkRefs,
      audience,
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      proposal.updatedAtIso,
      nowIso,
    ),
    workClass: proposal.workClass,
  })

const projectionValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(projectionValues)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.values(value).flatMap(projectionValues)
  }

  return []
}

const allowedPublicLiteralValues = new Set<string>([
  ...ARTANIS_WORK_ROUTING_CAPABILITIES,
  ...ARTANIS_WORK_ROUTING_WORK_CLASSES,
  'accepted',
  'approval_required',
  'background',
  'benchmark_cloud',
  'blocked',
  'completed',
  'dedicated',
  'degraded',
  'dispatched',
  'model_lab',
  'nexus',
  'not_applicable',
  'operator',
  'operator_selected',
  'overnight',
  'probe',
  'prohibited',
  'proposed',
  'psionic',
  'public_artanis',
  'public_forum',
  'pylon',
  'rejected',
  'runner',
  'safe_read_only',
])

export const artanisWorkRoutingProjectionHasPrivateMaterial = (
  projection: ArtanisWorkRoutingLedgerProjection,
): boolean =>
  projectionValues(projection).some(
    value =>
      !allowedPublicLiteralValues.has(value) &&
      (unsafeRoutingPattern.test(value) ||
        rawTimestampPattern.test(value) ||
        publicUnsafeRefPattern.test(value)),
  )

export const projectArtanisWorkRoutingLedger = (
  ledger: ArtanisWorkRoutingLedgerRecord,
  audience: ArtanisWorkRoutingAudience,
  nowIso: string,
): ArtanisWorkRoutingLedgerProjection => {
  assertLedger(ledger)

  const proposals = ledger.proposals.map(proposal =>
    projectProposal(proposal, audience, nowIso),
  )
  const projection = new ArtanisWorkRoutingLedgerProjection({
    agentId: ledger.agentId,
    audience,
    authority: ledger.authority,
    caveatRefs: refsForAudience(
      'Artanis work-routing caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    ledgerRef: refsForAudience(
      'Artanis work-routing ledger ref',
      [ledger.ledgerRef],
      audience,
    )[0] ?? 'work_routing.redacted.artanis_ledger',
    proposalCount: proposals.length,
    proposals,
    publicStatusRefs: refsForAudience(
      'Artanis work-routing public status refs',
      ledger.publicStatusRefs,
      audience,
    ),
    riskyProposalRefs: proposals
      .filter(proposal => proposal.risk === 'approval_required')
      .map(proposal => proposal.proposalRef),
    traceableWorkRefs: uniqueRefs(
      proposals.flatMap(proposal => proposal.traceableWorkRefs),
    ),
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  })

  if (
    audience !== 'operator' &&
    artanisWorkRoutingProjectionHasPrivateMaterial(projection)
  ) {
    throw new ArtanisWorkRoutingUnsafe({
      reason:
        'Public Artanis work-routing projection contains private material.',
    })
  }

  return projection
}

const proposal = (
  input: Omit<ArtanisWorkRoutingProposalRecord, 'proposalRef'> & {
    proposalRefSuffix: string
  },
): ArtanisWorkRoutingProposalRecord => {
  const { proposalRefSuffix, ...record } = input

  return new ArtanisWorkRoutingProposalRecord({
    ...record,
    proposalRef: `work.public.artanis.${proposalRefSuffix}`,
  })
}

export const exampleArtanisWorkRoutingLedger =
  new ArtanisWorkRoutingLedgerRecord({
    agentId: 'agent_artanis',
    authority: ARTANIS_WORK_ROUTING_NO_DIRECT_AUTHORITY,
    caveatRefs: ['caveat.public.work_routing_not_execution'],
    createdAtIso: '2026-06-07T04:00:00.000Z',
    ledgerRef: 'ledger.public.artanis.work_routing',
    proposals: [
      proposal({
        acceptanceCriteriaRefs: ['criteria.public.benchmark_scorecard_delta'],
        approvalRequirementRefs: ['approval.public.artanis.eval_launch_pending'],
        blockerRefs: [],
        capability: 'benchmark_evaluation',
        costCaveatRefs: ['cost.public.model_lab_eval_budget'],
        createdAtIso: '2026-06-07T04:01:00.000Z',
        decidedAtIso: null,
        operatorDetailRefs: ['operator.artanis.route.eval_candidate'],
        proposalRefSuffix: 'benchmark_eval_proposed',
        publicCaveatRefs: ['caveat.public.eval_requires_operator_approval'],
        receiptRefs: [],
        resourceMode: 'not_applicable',
        risk: 'approval_required',
        sourceEvidenceRefs: [
          'model_lab.public.report.autopilot_benchmark_loop',
          'failure.public.retained.autopilot_eval_gap',
        ],
        spendLimitRefs: ['spend_limit.public.model_lab_eval_bounded'],
        state: 'proposed',
        target: 'benchmark_cloud',
        targetCapabilityRefs: ['capability.public.benchmark_cloud.eval'],
        traceableWorkRefs: [],
        updatedAtIso: '2026-06-07T04:01:00.000Z',
        workClass: 'benchmark_evaluation',
      }),
      proposal({
        acceptanceCriteriaRefs: ['criteria.public.pylon_inference_trace'],
        approvalRequirementRefs: [],
        blockerRefs: [],
        capability: 'inference',
        costCaveatRefs: ['cost.public.inference_low'],
        createdAtIso: '2026-06-07T04:02:00.000Z',
        decidedAtIso: '2026-06-07T04:03:00.000Z',
        operatorDetailRefs: ['operator.artanis.route.pylon_inference'],
        proposalRefSuffix: 'pylon_inference_accepted',
        publicCaveatRefs: ['caveat.public.inference_read_only'],
        receiptRefs: ['receipt.public.artanis.pylon_inference_accepted'],
        resourceMode: 'background',
        risk: 'safe_read_only',
        sourceEvidenceRefs: ['pylon.public.stats', 'nexus.public.stats'],
        spendLimitRefs: ['spend_limit.public.inference_zero_spend'],
        state: 'accepted',
        target: 'pylon',
        targetCapabilityRefs: ['capability.public.pylon.inference'],
        traceableWorkRefs: ['work.public.pylon.inference.trace_001'],
        updatedAtIso: '2026-06-07T04:03:00.000Z',
        workClass: 'inference',
      }),
      proposal({
        acceptanceCriteriaRefs: [
          'criteria.public.tassadar_executor_trace.digest_match',
          'criteria.public.tassadar_executor_trace.separate_replay_verdict',
        ],
        approvalRequirementRefs: [],
        blockerRefs: [],
        capability: 'executor_trace_validation',
        costCaveatRefs: ['cost.public.tassadar_executor_trace.no_spend_default'],
        createdAtIso: '2026-06-10T16:30:00.000Z',
        decidedAtIso: '2026-06-10T16:31:00.000Z',
        operatorDetailRefs: [
          'operator.artanis.route.tassadar_executor_trace',
        ],
        proposalRefSuffix: 'tassadar_executor_trace_dispatched',
        publicCaveatRefs: [
          'caveat.public.tassadar_executor_trace.no_spend_dispatch_only',
          'caveat.public.tassadar_executor_trace.copy_limited_to_safeCopy',
        ],
        receiptRefs: [
          'receipt.public.artanis.tassadar_executor_trace.dispatch_ready',
        ],
        resourceMode: 'background',
        risk: 'safe_read_only',
        sourceEvidenceRefs: [
          'docs/artanis/2026-06-10-executor-trace-loop-candidate.md',
          'promise.public.compute.tassadar_executor_poc.v1',
        ],
        spendLimitRefs: [
          'spend_limit.public.tassadar_executor_trace.zero_sats_default',
        ],
        state: 'dispatched',
        target: 'pylon',
        targetCapabilityRefs: [TASSADAR_EXECUTOR_CAPABILITY_REF],
        traceableWorkRefs: [
          'assignment.public.artanis.tassadar_executor_trace.template',
        ],
        updatedAtIso: '2026-06-10T16:31:00.000Z',
        workClass: 'executor_trace_validation',
      }),
      proposal({
        acceptanceCriteriaRefs: ['criteria.public.adapter_validation_report'],
        approvalRequirementRefs: ['approval.public.artanis.training_pending'],
        blockerRefs: ['blocker.public.no_training_spend_authority'],
        capability: 'lora_finetuning',
        costCaveatRefs: ['cost.public.training_requires_budget'],
        createdAtIso: '2026-06-07T04:04:00.000Z',
        decidedAtIso: '2026-06-07T04:05:00.000Z',
        operatorDetailRefs: ['operator.artanis.route.training_candidate'],
        proposalRefSuffix: 'lora_training_blocked',
        publicCaveatRefs: ['caveat.public.training_blocked_without_approval'],
        receiptRefs: [],
        resourceMode: 'overnight',
        risk: 'blocked',
        sourceEvidenceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
        spendLimitRefs: ['spend_limit.public.training_not_approved'],
        state: 'blocked',
        target: 'model_lab',
        targetCapabilityRefs: ['capability.public.model_lab.lora_finetuning'],
        traceableWorkRefs: [],
        updatedAtIso: '2026-06-07T04:05:00.000Z',
        workClass: 'lora_finetuning',
      }),
      proposal({
        acceptanceCriteriaRefs: ['criteria.public.data_package_validation'],
        approvalRequirementRefs: [],
        blockerRefs: ['blocker.public.data_rights_not_public_safe'],
        capability: 'embedding_data_prep',
        costCaveatRefs: ['cost.public.data_prep_blocked'],
        createdAtIso: '2026-06-07T04:06:00.000Z',
        decidedAtIso: '2026-06-07T04:07:00.000Z',
        operatorDetailRefs: ['operator.artanis.route.data_prep_candidate'],
        proposalRefSuffix: 'data_prep_rejected',
        publicCaveatRefs: ['caveat.public.data_prep_rejected_data_rights'],
        receiptRefs: [],
        resourceMode: 'operator_selected',
        risk: 'prohibited',
        sourceEvidenceRefs: ['model_lab.public.missing_data_rights'],
        spendLimitRefs: [],
        state: 'rejected',
        target: 'runner',
        targetCapabilityRefs: ['capability.public.runner.data_prep'],
        traceableWorkRefs: [],
        updatedAtIso: '2026-06-07T04:07:00.000Z',
        workClass: 'embedding_data_prep',
      }),
    ],
    publicStatusRefs: ['work_routing.public.artanis.ready_for_review'],
    updatedAtIso: '2026-06-07T04:08:00.000Z',
  })
