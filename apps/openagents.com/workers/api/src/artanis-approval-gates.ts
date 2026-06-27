import { Schema as S } from 'effect'

import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const ArtanisApprovalGateAudience = S.Literals([
  'operator',
  'public_artanis',
  'public_forum',
])
export type ArtanisApprovalGateAudience =
  typeof ArtanisApprovalGateAudience.Type

export const ArtanisRiskyActionKind = S.Literals([
  'adapter_install',
  'deployment',
  'eval_launch',
  'fleet_mutation',
  'forum_post',
  'l402_redemption',
  'provider_call',
  'public_claim_upgrade',
  'pylon_job_dispatch',
  'runtime_promotion',
  'settlement',
  'training_launch',
  'wallet_spend',
])
export type ArtanisRiskyActionKind = typeof ArtanisRiskyActionKind.Type

export const ArtanisApprovalGateState = S.Literals([
  'approved',
  'denied',
  'expired',
  'pending',
  'superseded',
])
export type ArtanisApprovalGateState = typeof ArtanisApprovalGateState.Type

export const ArtanisApprovalAuthoritySourceKind = S.Literals([
  'forum_post',
  'model_lab_record',
  'operator_approval',
  'operator_policy',
  'pylon_stats',
  'retained_failure',
])
export type ArtanisApprovalAuthoritySourceKind =
  typeof ArtanisApprovalAuthoritySourceKind.Type

export const ArtanisApprovalRollbackPosture = S.Literals([
  'not_reversible',
  'rollback_not_applicable',
  'rollback_plan_recorded',
  'rollback_receipt_recorded',
])
export type ArtanisApprovalRollbackPosture =
  typeof ArtanisApprovalRollbackPosture.Type

export class ArtanisApprovalGateRecord extends S.Class<ArtanisApprovalGateRecord>(
  'ArtanisApprovalGateRecord',
)({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  authoritySourceKinds: S.Array(ArtanisApprovalAuthoritySourceKind),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  expiresAtIso: S.String,
  gateRef: S.String,
  idempotencyKey: S.String,
  kind: ArtanisRiskyActionKind,
  operatorReceiptRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  resolvedAtIso: S.NullOr(S.String),
  rollbackPosture: ArtanisApprovalRollbackPosture,
  rollbackRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: ArtanisApprovalGateState,
  supersededByGateRef: S.NullOr(S.String),
  updatedAtIso: S.String,
}) {}

export class ArtanisApprovalGateLedgerRecord extends S.Class<ArtanisApprovalGateLedgerRecord>(
  'ArtanisApprovalGateLedgerRecord',
)({
  agentId: S.String,
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  gates: S.Array(ArtanisApprovalGateRecord),
  ledgerRef: S.String,
  updatedAtIso: S.String,
}) {}

export class ArtanisApprovalGateProjection extends S.Class<ArtanisApprovalGateProjection>(
  'ArtanisApprovalGateProjection',
)({
  actionRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  authoritySourceKinds: S.Array(ArtanisApprovalAuthoritySourceKind),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  effective: S.Boolean,
  expiresAtDisplay: S.String,
  gateRef: S.String,
  kind: ArtanisRiskyActionKind,
  label: S.String,
  operatorReceiptRefs: S.Array(S.String),
  policyRefs: S.Array(S.String),
  privateEvidenceRefs: S.Array(S.String),
  publicStatusRefs: S.Array(S.String),
  resolvedAtDisplay: S.NullOr(S.String),
  rollbackPosture: ArtanisApprovalRollbackPosture,
  rollbackRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  state: ArtanisApprovalGateState,
  supersededByGateRef: S.NullOr(S.String),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisApprovalGateLedgerProjection extends S.Class<ArtanisApprovalGateLedgerProjection>(
  'ArtanisApprovalGateLedgerProjection',
)({
  agentId: S.String,
  audience: ArtanisApprovalGateAudience,
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  effectiveGateRefs: S.Array(S.String),
  gateCount: S.Number,
  gates: S.Array(ArtanisApprovalGateProjection),
  ledgerRef: S.String,
  riskyActionKinds: S.Array(ArtanisRiskyActionKind),
  updatedAtDisplay: S.String,
}) {}

export class ArtanisApprovalGateUnsafe extends S.TaggedErrorClass<ArtanisApprovalGateUnsafe>()(
  'ArtanisApprovalGateUnsafe',
  {
    reason: S.String,
  },
) {}

export const ARTANIS_RISKY_ACTION_KINDS: ReadonlyArray<ArtanisRiskyActionKind> =
  [
    'adapter_install',
    'deployment',
    'eval_launch',
    'fleet_mutation',
    'forum_post',
    'l402_redemption',
    'provider_call',
    'public_claim_upgrade',
    'pylon_job_dispatch',
    'runtime_promotion',
    'settlement',
    'training_launch',
    'wallet_spend',
  ]

const rollbackRequiredKinds = new Set<ArtanisRiskyActionKind>([
  'adapter_install',
  'deployment',
  'eval_launch',
  'fleet_mutation',
  'provider_call',
  'public_claim_upgrade',
  'pylon_job_dispatch',
  'runtime_promotion',
  'training_launch',
])

const stateLabel: Record<ArtanisApprovalGateState, string> = {
  approved: 'Approved by operator',
  denied: 'Denied by operator',
  expired: 'Expired approval',
  pending: 'Pending operator review',
  superseded: 'Superseded approval',
}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/{}-]{0,260}$/
const unsafeApprovalRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|phone|prompt|record|value)|dataset\.raw|email[_-]?(address|body|html|raw|text)|full[_-]?(prompt|source|trace)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice[_-]?(id|raw)|lnbc|lntb|lnbcrt|lno1|lnurl|macaroon|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|model[_-]?(weights|raw|secret)|oauth|opencode_auth_content|payment[_-]?(hash|id|invoice|preimage|proof|secret)|payout[_-]?(address|destination|private|raw|target)|preimage|private[_-]?(archive|customer|dataset|key|prompt|source|trace|wallet)|provider[_-]?(account|credential|grant|payload|secret|token)|raw[_-]?(artifact|auth|customer|dataset|email|invoice|log|model|payment|payload|payout|prompt|provider|record|repo|runner|run[_-]?log|source|state|target|telemetry|text|trace|training|weights|webhook)|recovery[_-]?phrase|runner[_-]?(payload|secret|token)|secret|seed[_-]?phrase|sk-[a-z0-9]|source[_-]?(archive|raw)|token|wallet[._-]?(key|material|mnemonic|payment|preimage|secret|seed|spend)|weights\.(bin|gguf|safetensors|pt|pth))/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(authority\.private|evidence\.private|operator\.|receipt\.operator|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const hasAny = <A>(items: ReadonlyArray<A>): boolean => items.length > 0

const assertValidIso = (label: string, iso: string): void => {
  if (!Number.isFinite(Date.parse(iso))) {
    throw new ArtanisApprovalGateUnsafe({
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
      unsafeApprovalRefPattern.test(ref) ||
      rawTimestampPattern.test(ref),
  )

  if (unsafe !== undefined) {
    throw new ArtanisApprovalGateUnsafe({
      reason: `${label} contains unsafe provider, runner, wallet, payment, customer, private repo, secret, raw prompt, raw log, or raw timestamp material.`,
    })
  }
}

const refsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: ArtanisApprovalGateAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  if (audience === 'operator') {
    return uniqueRefs(refs)
  }

  return uniqueRefs(refs).filter(ref => !publicUnsafeRefPattern.test(ref))
}

const rollbackRequired = (kind: ArtanisRiskyActionKind): boolean =>
  rollbackRequiredKinds.has(kind)

const gateExpired = (
  gate: ArtanisApprovalGateRecord,
  nowIso: string,
): boolean => Date.parse(gate.expiresAtIso) <= Date.parse(nowIso)

export const artanisApprovalGateEffective = (
  gate: ArtanisApprovalGateRecord,
  nowIso: string,
): boolean =>
  gate.state === 'approved' &&
  !gateExpired(gate, nowIso) &&
  gate.supersededByGateRef === null &&
  gate.authoritySourceKinds.includes('operator_approval') &&
  hasAny(gate.authorityReceiptRefs)

const publicStateForGate = (
  gate: ArtanisApprovalGateRecord,
  nowIso: string,
): ArtanisApprovalGateState =>
  gate.state === 'approved' && gateExpired(gate, nowIso)
    ? 'expired'
    : gate.state

const assertApprovedGate = (gate: ArtanisApprovalGateRecord): void => {
  if (!gate.authoritySourceKinds.includes('operator_approval')) {
    throw new ArtanisApprovalGateUnsafe({
      reason:
        'Approved Artanis risky-action gates require operator approval as the authority source.',
    })
  }

  if (!hasAny(gate.authorityReceiptRefs)) {
    throw new ArtanisApprovalGateUnsafe({
      reason:
        'Approved Artanis risky-action gates require explicit authority receipt refs.',
    })
  }
}

const assertGate = (gate: ArtanisApprovalGateRecord): void => {
  assertValidIso('gate.createdAtIso', gate.createdAtIso)
  assertValidIso('gate.updatedAtIso', gate.updatedAtIso)
  assertValidIso('gate.expiresAtIso', gate.expiresAtIso)
  assertMaybeIso('gate.resolvedAtIso', gate.resolvedAtIso)
  assertSafeRefs('Artanis approval gate ref', [gate.gateRef])
  assertSafeRefs('Artanis approval gate idempotency key', [
    gate.idempotencyKey,
  ])
  assertSafeRefs('Artanis approval action ref', [gate.actionRef])
  assertSafeRefs(
    'Artanis approval authority receipt refs',
    gate.authorityReceiptRefs,
  )
  assertSafeRefs(
    'Artanis approval operator receipt refs',
    gate.operatorReceiptRefs,
  )
  assertSafeRefs('Artanis approval policy refs', gate.policyRefs)
  assertSafeRefs('Artanis approval caveat refs', gate.caveatRefs)
  assertSafeRefs('Artanis approval public status refs', gate.publicStatusRefs)
  assertSafeRefs('Artanis approval rollback refs', gate.rollbackRefs)
  assertSafeRefs('Artanis approval source refs', gate.sourceRefs)
  assertSafeRefs(
    'Artanis approval private evidence refs',
    gate.privateEvidenceRefs,
  )
  assertSafeRefs('Artanis superseded gate ref', [
    gate.supersededByGateRef ?? 'gate.none',
  ])

  if (!ARTANIS_RISKY_ACTION_KINDS.includes(gate.kind)) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Artanis approval gates must use an enumerated risky action kind.',
    })
  }

  if (
    !hasAny(gate.operatorReceiptRefs) ||
    !hasAny(gate.policyRefs) ||
    !hasAny(gate.caveatRefs) ||
    !hasAny(gate.publicStatusRefs)
  ) {
    throw new ArtanisApprovalGateUnsafe({
      reason:
        'Artanis risky-action gates require operator receipts, policy refs, caveat refs, and public status refs.',
    })
  }

  if (
    rollbackRequired(gate.kind) &&
    (gate.rollbackPosture === 'rollback_not_applicable' ||
      gate.rollbackPosture === 'not_reversible' ||
      !hasAny(gate.rollbackRefs))
  ) {
    throw new ArtanisApprovalGateUnsafe({
      reason:
        'This Artanis risky action requires a rollback plan or rollback receipt ref.',
    })
  }

  if (gate.state === 'approved') {
    assertApprovedGate(gate)
  }

  if (gate.state === 'superseded' && gate.supersededByGateRef === null) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Superseded Artanis approvals require a replacement gate ref.',
    })
  }

  if (
    gate.state !== 'superseded' &&
    gate.supersededByGateRef !== null
  ) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Only superseded Artanis approvals can carry a replacement gate ref.',
    })
  }

  if (
    gate.state !== 'pending' &&
    gate.resolvedAtIso === null
  ) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Resolved Artanis approval states require a resolved timestamp.',
    })
  }
}

const assertLedger = (ledger: ArtanisApprovalGateLedgerRecord): void => {
  assertValidIso('ledger.createdAtIso', ledger.createdAtIso)
  assertValidIso('ledger.updatedAtIso', ledger.updatedAtIso)
  assertSafeRefs('Artanis approval ledger agent id', [ledger.agentId])
  assertSafeRefs('Artanis approval ledger ref', [ledger.ledgerRef])
  assertSafeRefs('Artanis approval ledger caveat refs', ledger.caveatRefs)

  if (ledger.agentId !== 'agent_artanis') {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Artanis approval ledgers must use agent_artanis.',
    })
  }

  if (!hasAny(ledger.gates)) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Artanis approval ledgers require at least one gate.',
    })
  }

  ledger.gates.forEach(assertGate)
}

const projectGate = (
  gate: ArtanisApprovalGateRecord,
  audience: ArtanisApprovalGateAudience,
  nowIso: string,
): ArtanisApprovalGateProjection => {
  const state = publicStateForGate(gate, nowIso)

  return {
    actionRef: refsForAudience(
      'Artanis approval action ref',
      [gate.actionRef],
      audience,
    )[0] ?? 'action.redacted.artanis_approval',
    authorityReceiptRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis approval authority receipt refs',
          gate.authorityReceiptRefs,
          audience,
        )
      : [],
    authoritySourceKinds: audience === 'operator'
      ? [...gate.authoritySourceKinds].sort()
      : [],
    caveatRefs: refsForAudience(
      'Artanis approval caveat refs',
      gate.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      gate.createdAtIso,
      nowIso,
    ),
    effective: audience === 'operator' &&
      artanisApprovalGateEffective(gate, nowIso),
    expiresAtDisplay: friendlyBlueprintMissionBriefingTime(
      gate.expiresAtIso,
      nowIso,
    ),
    gateRef: refsForAudience(
      'Artanis approval gate ref',
      [gate.gateRef],
      audience,
    )[0] ?? 'gate.redacted.artanis_approval',
    kind: gate.kind,
    label: stateLabel[state],
    operatorReceiptRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis approval operator receipt refs',
          gate.operatorReceiptRefs,
          audience,
        )
      : [],
    policyRefs: refsForAudience(
      'Artanis approval policy refs',
      gate.policyRefs,
      audience,
    ),
    privateEvidenceRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis approval private evidence refs',
          gate.privateEvidenceRefs,
          audience,
        )
      : [],
    publicStatusRefs: refsForAudience(
      'Artanis approval public status refs',
      gate.publicStatusRefs,
      audience,
    ),
    resolvedAtDisplay: gate.resolvedAtIso === null
      ? null
      : friendlyBlueprintMissionBriefingTime(gate.resolvedAtIso, nowIso),
    rollbackPosture: gate.rollbackPosture,
    rollbackRefs: audience === 'operator'
      ? refsForAudience(
          'Artanis approval rollback refs',
          gate.rollbackRefs,
          audience,
        )
      : [],
    sourceRefs: refsForAudience(
      'Artanis approval source refs',
      gate.sourceRefs,
      audience,
    ),
    state,
    supersededByGateRef: audience === 'operator'
      ? gate.supersededByGateRef
      : null,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      gate.updatedAtIso,
      nowIso,
    ),
  }
}

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

export const artanisApprovalGateProjectionHasPrivateMaterial = (
  projection: ArtanisApprovalGateLedgerProjection,
): boolean =>
  projectionValues(projection).some(
    value =>
      !ARTANIS_RISKY_ACTION_KINDS.includes(value as ArtanisRiskyActionKind) &&
      (unsafeApprovalRefPattern.test(value) ||
        rawTimestampPattern.test(value) ||
        publicUnsafeRefPattern.test(value)),
  )

export const projectArtanisApprovalGateLedger = (
  ledger: ArtanisApprovalGateLedgerRecord,
  audience: ArtanisApprovalGateAudience,
  nowIso: string,
): ArtanisApprovalGateLedgerProjection => {
  assertLedger(ledger)

  const gates = ledger.gates.map(gate => projectGate(gate, audience, nowIso))
  const projection: ArtanisApprovalGateLedgerProjection = {
    agentId: ledger.agentId,
    audience,
    caveatRefs: refsForAudience(
      'Artanis approval ledger caveat refs',
      ledger.caveatRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.createdAtIso,
      nowIso,
    ),
    effectiveGateRefs: audience === 'operator'
      ? gates.filter(gate => gate.effective).map(gate => gate.gateRef)
      : [],
    gateCount: gates.length,
    gates,
    ledgerRef: refsForAudience(
      'Artanis approval ledger ref',
      [ledger.ledgerRef],
      audience,
    )[0] ?? 'ledger.redacted.artanis_approval',
    riskyActionKinds: [...ARTANIS_RISKY_ACTION_KINDS],
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      ledger.updatedAtIso,
      nowIso,
    ),
  }

  if (
    audience !== 'operator' &&
    artanisApprovalGateProjectionHasPrivateMaterial(projection)
  ) {
    throw new ArtanisApprovalGateUnsafe({
      reason: 'Public Artanis approval gate projection contains private material.',
    })
  }

  return projection
}

const gate = (
  input: Omit<
    ArtanisApprovalGateRecord,
    'actionRef' | 'gateRef' | 'idempotencyKey'
  > & {
    actionRefSuffix: string
    gateRefSuffix: string
  },
): ArtanisApprovalGateRecord => {
  const { actionRefSuffix, gateRefSuffix, ...record } = input

  return new ArtanisApprovalGateRecord({
    ...record,
    actionRef: `action.public.artanis.${actionRefSuffix}`,
    gateRef: `gate.public.artanis.${gateRefSuffix}`,
    idempotencyKey: `artanis-approval:${gateRefSuffix}:v1`,
  })
}

export const exampleArtanisApprovalGateLedger =
  new ArtanisApprovalGateLedgerRecord({
    agentId: 'agent_artanis',
    caveatRefs: ['caveat.public.approval_gates_are_not_execution'],
    createdAtIso: '2026-06-07T02:10:00.000Z',
    gates: [
      gate({
        actionRefSuffix: 'pylon_job_dispatch',
        authorityReceiptRefs: ['authority.public.artanis.pylon_dispatch.approved'],
        authoritySourceKinds: ['operator_approval', 'operator_policy'],
        caveatRefs: ['caveat.public.dispatch_scope_limited'],
        createdAtIso: '2026-06-07T02:11:00.000Z',
        expiresAtIso: '2026-06-07T05:11:00.000Z',
        gateRefSuffix: 'pylon_job_dispatch_approved',
        kind: 'pylon_job_dispatch',
        operatorReceiptRefs: ['receipt.operator.artanis.approve_pylon_dispatch'],
        policyRefs: ['policy.public.artanis.pylon_dispatch_bounded'],
        privateEvidenceRefs: ['evidence.private.artanis.operator_pylon_dispatch'],
        publicStatusRefs: ['approval.public.artanis.pylon_dispatch_approved'],
        resolvedAtIso: '2026-06-07T02:12:00.000Z',
        rollbackPosture: 'rollback_plan_recorded',
        rollbackRefs: ['rollback.public.artanis.cancel_pylon_dispatch'],
        sourceRefs: [
          'forum.public.artanis.work_routing',
          'model_lab.public.report.autopilot_benchmark_loop',
          'pylon.public.resource_modes',
        ],
        state: 'approved',
        supersededByGateRef: null,
        updatedAtIso: '2026-06-07T02:12:00.000Z',
      }),
      gate({
        actionRefSuffix: 'bitcoin_spend_review',
        authorityReceiptRefs: [],
        authoritySourceKinds: ['operator_policy'],
        caveatRefs: ['caveat.public.no_live_bitcoin_spend'],
        createdAtIso: '2026-06-07T02:13:00.000Z',
        expiresAtIso: '2026-06-07T05:13:00.000Z',
        gateRefSuffix: 'bitcoin_spend_denied',
        kind: 'wallet_spend',
        operatorReceiptRefs: ['receipt.operator.artanis.deny_bitcoin_spend'],
        policyRefs: ['policy.public.no_live_spend_without_named_cap'],
        privateEvidenceRefs: ['evidence.private.artanis.spend_review'],
        publicStatusRefs: ['approval.public.artanis.bitcoin_spend_denied'],
        resolvedAtIso: '2026-06-07T02:14:00.000Z',
        rollbackPosture: 'not_reversible',
        rollbackRefs: [],
        sourceRefs: ['forum.public.artanis.bitcoin_accounting'],
        state: 'denied',
        supersededByGateRef: null,
        updatedAtIso: '2026-06-07T02:14:00.000Z',
      }),
      gate({
        actionRefSuffix: 'training_launch',
        authorityReceiptRefs: [],
        authoritySourceKinds: ['operator_policy'],
        caveatRefs: ['caveat.public.training_launch_window_expired'],
        createdAtIso: '2026-06-07T00:00:00.000Z',
        expiresAtIso: '2026-06-07T01:00:00.000Z',
        gateRefSuffix: 'training_launch_expired',
        kind: 'training_launch',
        operatorReceiptRefs: ['receipt.operator.artanis.training_launch_review'],
        policyRefs: ['policy.public.training_launch_requires_fresh_approval'],
        privateEvidenceRefs: ['evidence.private.artanis.training_launch_review'],
        publicStatusRefs: ['approval.public.artanis.training_launch_expired'],
        resolvedAtIso: '2026-06-07T01:01:00.000Z',
        rollbackPosture: 'rollback_plan_recorded',
        rollbackRefs: ['rollback.public.artanis.stop_training_run'],
        sourceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
        state: 'expired',
        supersededByGateRef: null,
        updatedAtIso: '2026-06-07T01:01:00.000Z',
      }),
      gate({
        actionRefSuffix: 'runtime_promotion',
        authorityReceiptRefs: [],
        authoritySourceKinds: ['operator_policy'],
        caveatRefs: ['caveat.public.runtime_promotion_superseded'],
        createdAtIso: '2026-06-07T02:15:00.000Z',
        expiresAtIso: '2026-06-07T05:15:00.000Z',
        gateRefSuffix: 'runtime_promotion_superseded',
        kind: 'runtime_promotion',
        operatorReceiptRefs: ['receipt.operator.artanis.supersede_runtime_promotion'],
        policyRefs: ['policy.public.runtime_promotion_requires_latest_eval'],
        privateEvidenceRefs: ['evidence.private.artanis.runtime_promotion_review'],
        publicStatusRefs: ['approval.public.artanis.runtime_promotion_superseded'],
        resolvedAtIso: '2026-06-07T02:16:00.000Z',
        rollbackPosture: 'rollback_receipt_recorded',
        rollbackRefs: ['rollback.public.artanis.runtime_promotion_superseded'],
        sourceRefs: ['model_lab.public.report.autopilot_benchmark_loop'],
        state: 'superseded',
        supersededByGateRef: 'gate.public.artanis.runtime_promotion_recheck',
        updatedAtIso: '2026-06-07T02:16:00.000Z',
      }),
      gate({
        actionRefSuffix: 'l402_redemption',
        authorityReceiptRefs: [],
        authoritySourceKinds: ['operator_policy'],
        caveatRefs: ['caveat.public.l402_redemption_pending'],
        createdAtIso: '2026-06-07T02:17:00.000Z',
        expiresAtIso: '2026-06-07T05:17:00.000Z',
        gateRefSuffix: 'l402_redemption_pending',
        kind: 'l402_redemption',
        operatorReceiptRefs: ['receipt.operator.artanis.l402_review_opened'],
        policyRefs: ['policy.public.l402_requires_explicit_spend_authority'],
        privateEvidenceRefs: ['evidence.private.artanis.l402_review'],
        publicStatusRefs: ['approval.public.artanis.l402_redemption_pending'],
        resolvedAtIso: null,
        rollbackPosture: 'not_reversible',
        rollbackRefs: [],
        sourceRefs: ['forum.public.artanis.bitcoin_accounting'],
        state: 'pending',
        supersededByGateRef: null,
        updatedAtIso: '2026-06-07T02:17:00.000Z',
      }),
    ],
    ledgerRef: 'ledger.public.artanis.approval_gates',
    updatedAtIso: '2026-06-07T02:18:00.000Z',
  })
