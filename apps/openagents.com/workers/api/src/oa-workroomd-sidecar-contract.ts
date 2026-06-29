import { Schema as S } from 'effect'

import {
  friendlyBlueprintMissionBriefingTime,
} from './blueprint/services/continuation-mission-briefing'
import {
  OmniProjectionAudience,
} from './omni-data-classification'
import {
  OpenAgentsRunnerBackendKind,
  OpenAgentsRunnerWorkloadTrust,
} from './runner-backends'

export const OpenAgentsWorkroomdSessionStatus = S.Literals([
  'archived',
  'awaiting_review',
  'cancelled',
  'cancelling',
  'closed',
  'closeout_ready',
  'creating',
  'destroyed',
  'failed',
  'needs_context',
  'ready',
  'running',
])
export type OpenAgentsWorkroomdSessionStatus =
  typeof OpenAgentsWorkroomdSessionStatus.Type

export const OpenAgentsWorkroomdEventKind = S.Literals([
  'archive_recorded',
  'artifact_manifest_recorded',
  'assignment_received',
  'cancellation_acknowledged',
  'cancellation_requested',
  'closeout_emitted',
  'destroy_recorded',
  'failure_recorded',
  'grant_refs_resolved',
  'lifecycle_event_recorded',
  'session_created',
  'turn_started',
])
export type OpenAgentsWorkroomdEventKind =
  typeof OpenAgentsWorkroomdEventKind.Type

export const OpenAgentsWorkroomdGrantResolutionState = S.Literals([
  'blocked',
  'not_required',
  'refs_present',
  'resolved_by_daemon',
])
export type OpenAgentsWorkroomdGrantResolutionState =
  typeof OpenAgentsWorkroomdGrantResolutionState.Type

export const OpenAgentsWorkroomdCancellationState = S.Literals([
  'acknowledged',
  'cancelled',
  'none',
  'requested',
])
export type OpenAgentsWorkroomdCancellationState =
  typeof OpenAgentsWorkroomdCancellationState.Type

export const OpenAgentsWorkroomdArchiveState = S.Literals([
  'active',
  'archived',
  'destroyed',
])
export type OpenAgentsWorkroomdArchiveState =
  typeof OpenAgentsWorkroomdArchiveState.Type

export const OpenAgentsWorkroomdCloseoutState = S.Literals([
  'emitted',
  'failed',
  'none',
  'pending',
])
export type OpenAgentsWorkroomdCloseoutState =
  typeof OpenAgentsWorkroomdCloseoutState.Type

export class OpenAgentsWorkroomdSessionRecord extends S.Class<OpenAgentsWorkroomdSessionRecord>(
  'OpenAgentsWorkroomdSessionRecord',
)({
  archiveState: OpenAgentsWorkroomdArchiveState,
  artifactManifestRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  auditEvidenceRefs: S.Array(S.String),
  backendKind: OpenAgentsRunnerBackendKind,
  cancellationRefs: S.Array(S.String),
  cancellationState: OpenAgentsWorkroomdCancellationState,
  closeoutCaveatRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  closeoutState: OpenAgentsWorkroomdCloseoutState,
  correlationRefs: S.Array(S.String),
  createdAtIso: S.String,
  daemonRef: S.String,
  eventKinds: S.Array(OpenAgentsWorkroomdEventKind),
  failureReceiptRefs: S.Array(S.String),
  grantRefs: S.Array(S.String),
  grantResolutionRefs: S.Array(S.String),
  grantResolutionState: OpenAgentsWorkroomdGrantResolutionState,
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  lifecycleEventRefs: S.Array(S.String),
  nodeRef: S.String,
  policyRefs: S.Array(S.String),
  publicArtifactRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  runtimeRef: S.String,
  sessionRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: OpenAgentsWorkroomdSessionStatus,
  trustLevel: OpenAgentsRunnerWorkloadTrust,
  updatedAtIso: S.String,
  workroomRef: S.String,
  workspaceRef: S.String,
}) {}

export class OpenAgentsWorkroomdSessionProjection extends S.Class<OpenAgentsWorkroomdSessionProjection>(
  'OpenAgentsWorkroomdSessionProjection',
)({
  archiveState: OpenAgentsWorkroomdArchiveState,
  artifactManifestRefs: S.Array(S.String),
  artifactRefs: S.Array(S.String),
  assignmentRef: S.String,
  auditEvidencePreserved: S.Boolean,
  auditEvidenceRefs: S.Array(S.String),
  audience: OmniProjectionAudience,
  backendKind: OpenAgentsRunnerBackendKind,
  cancellationRefs: S.Array(S.String),
  cancellationState: OpenAgentsWorkroomdCancellationState,
  closeoutCaveatRefs: S.Array(S.String),
  closeoutReceiptRefs: S.Array(S.String),
  closeoutState: OpenAgentsWorkroomdCloseoutState,
  correlationRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  daemonRef: S.String,
  eventKinds: S.Array(OpenAgentsWorkroomdEventKind),
  failureReceiptRefs: S.Array(S.String),
  grantRefs: S.Array(S.String),
  grantResolutionRefs: S.Array(S.String),
  grantResolutionState: OpenAgentsWorkroomdGrantResolutionState,
  id: S.String,
  idempotencyRefs: S.Array(S.String),
  lifecycleEventRefs: S.Array(S.String),
  nodeRef: S.String,
  policyRefs: S.Array(S.String),
  publicArtifactRefs: S.Array(S.String),
  routeRefs: S.Array(S.String),
  runtimeRef: S.String,
  sessionRef: S.String,
  sourceAuthorityRefs: S.Array(S.String),
  status: OpenAgentsWorkroomdSessionStatus,
  trustLevel: OpenAgentsRunnerWorkloadTrust,
  updatedAtDisplay: S.String,
  workroomRef: S.String,
  workspaceRef: S.String,
}) {}

export class OpenAgentsWorkroomdSessionUnsafe extends S.TaggedErrorClass<OpenAgentsWorkroomdSessionUnsafe>()(
  'OpenAgentsWorkroomdSessionUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const unsafeWorkroomdRefPattern =
  /(@|\/Users\/|\/home\/|access[_-]?token|auth[_-]?content[_-]?json|auth\.json|bearer|callback[_-]?token|cookie|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|local[_-]?path|lnbc|lntb|lnbcrt|lno1|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|id|preimage|proof)|payout[_-]?(address|destination|target)|preimage|private[_-]?key|provider[_-]?(grant|payload|token)|raw[_-]?(auth|email|invoice|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet)/i
const rawTimestampPattern = /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/
const publicUnsafeRefPattern =
  /(daemon\.private|grant|node\.private|route\.private|source\.private|workspace\.private|workroom\.private)/i
const customerUnsafeRefPattern =
  /(daemon\.private|grant|node\.private|route\.private|source\.private|workspace\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(grant\.private|route\.private|source\.private|workspace\.private|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const uniqueEventKinds = (
  kinds: ReadonlyArray<OpenAgentsWorkroomdEventKind>,
): ReadonlyArray<OpenAgentsWorkroomdEventKind> =>
  [...new Set(kinds)].sort()

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    unsafeWorkroomdRefPattern.test(ref) ||
    rawTimestampPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new OpenAgentsWorkroomdSessionUnsafe({
      reason: `${label} contains raw credentials, provider/account auth material, local paths, raw logs, raw source archives, private repo refs, wallet/payment material, payout targets, or raw timestamps.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: typeof OmniProjectionAudience.Type,
): RegExp | null => {
  if (audience === 'public' || audience === 'agent') {
    return publicUnsafeRefPattern
  }

  if (audience === 'customer') {
    return customerUnsafeRefPattern
  }

  if (audience === 'team') {
    return teamUnsafeRefPattern
  }

  return null
}

const safeRefsForAudience = (
  label: string,
  refs: ReadonlyArray<string>,
  audience: typeof OmniProjectionAudience.Type,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeRefForAudience = (
  label: string,
  ref: string,
  audience: typeof OmniProjectionAudience.Type,
): string =>
  safeRefsForAudience(label, [ref], audience)[0] ??
  `${label.replaceAll(' ', '_')}.redacted`

const recordRefs = (
  record: OpenAgentsWorkroomdSessionRecord,
): ReadonlyArray<string> => [
  record.id,
  record.assignmentRef,
  record.daemonRef,
  record.nodeRef,
  record.runtimeRef,
  record.sessionRef,
  record.workroomRef,
  record.workspaceRef,
  ...record.artifactManifestRefs,
  ...record.artifactRefs,
  ...record.auditEvidenceRefs,
  ...record.cancellationRefs,
  ...record.closeoutCaveatRefs,
  ...record.closeoutReceiptRefs,
  ...record.correlationRefs,
  ...record.failureReceiptRefs,
  ...record.grantRefs,
  ...record.grantResolutionRefs,
  ...record.idempotencyRefs,
  ...record.lifecycleEventRefs,
  ...record.policyRefs,
  ...record.publicArtifactRefs,
  ...record.routeRefs,
  ...record.sourceAuthorityRefs,
]

const assertRecordSafe = (
  record: OpenAgentsWorkroomdSessionRecord,
): void => {
  assertSafeRefs('oa-workroomd session refs', recordRefs(record))
}

export const openAgentsWorkroomdSessionPreservesAuditEvidence = (
  record: OpenAgentsWorkroomdSessionRecord,
): boolean =>
  record.auditEvidenceRefs.length > 0 &&
  (record.archiveState === 'active' ||
    record.closeoutReceiptRefs.length > 0 ||
    record.failureReceiptRefs.length > 0)

export const openAgentsWorkroomdSessionHasOnlyGrantRefs = (
  record: OpenAgentsWorkroomdSessionRecord,
): boolean => {
  assertSafeRefs('oa-workroomd grant refs', record.grantRefs)
  assertSafeRefs(
    'oa-workroomd grant resolution refs',
    record.grantResolutionRefs,
  )

  return true
}

export const openAgentsWorkroomdSessionCloseoutReady = (
  record: OpenAgentsWorkroomdSessionRecord,
): boolean =>
  record.closeoutState === 'pending' ||
  record.status === 'closeout_ready' ||
  record.status === 'awaiting_review'

export const projectOpenAgentsWorkroomdSession = (
  record: OpenAgentsWorkroomdSessionRecord,
  audience: typeof OmniProjectionAudience.Type,
  nowIso: string,
): OpenAgentsWorkroomdSessionProjection => {
  assertRecordSafe(record)

  const projection: OpenAgentsWorkroomdSessionProjection = {
    archiveState: record.archiveState,
    artifactManifestRefs: safeRefsForAudience(
      'oa-workroomd artifact manifest refs',
      record.artifactManifestRefs,
      audience,
    ),
    artifactRefs: safeRefsForAudience(
      'oa-workroomd artifact refs',
      record.artifactRefs,
      audience,
    ),
    assignmentRef: safeRefForAudience(
      'oa-workroomd assignment ref',
      record.assignmentRef,
      audience,
    ),
    auditEvidencePreserved:
      openAgentsWorkroomdSessionPreservesAuditEvidence(record),
    auditEvidenceRefs: safeRefsForAudience(
      'oa-workroomd audit evidence refs',
      record.auditEvidenceRefs,
      audience,
    ),
    audience,
    backendKind: record.backendKind,
    cancellationRefs: safeRefsForAudience(
      'oa-workroomd cancellation refs',
      record.cancellationRefs,
      audience,
    ),
    cancellationState: record.cancellationState,
    closeoutCaveatRefs: safeRefsForAudience(
      'oa-workroomd closeout caveat refs',
      record.closeoutCaveatRefs,
      audience,
    ),
    closeoutReceiptRefs: safeRefsForAudience(
      'oa-workroomd closeout receipt refs',
      record.closeoutReceiptRefs,
      audience,
    ),
    closeoutState: record.closeoutState,
    correlationRefs: safeRefsForAudience(
      'oa-workroomd correlation refs',
      record.correlationRefs,
      audience,
    ),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    daemonRef: safeRefForAudience(
      'oa-workroomd daemon ref',
      record.daemonRef,
      audience,
    ),
    eventKinds: uniqueEventKinds(record.eventKinds),
    failureReceiptRefs: safeRefsForAudience(
      'oa-workroomd failure receipt refs',
      record.failureReceiptRefs,
      audience,
    ),
    grantRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience('oa-workroomd grant refs', record.grantRefs, audience)
        : [],
    grantResolutionRefs:
      audience === 'operator' || audience === 'private'
        ? safeRefsForAudience(
            'oa-workroomd grant resolution refs',
            record.grantResolutionRefs,
            audience,
          )
        : [],
    grantResolutionState: record.grantResolutionState,
    id: safeRefForAudience('oa-workroomd id', record.id, audience),
    idempotencyRefs: safeRefsForAudience(
      'oa-workroomd idempotency refs',
      record.idempotencyRefs,
      audience,
    ),
    lifecycleEventRefs: safeRefsForAudience(
      'oa-workroomd lifecycle event refs',
      record.lifecycleEventRefs,
      audience,
    ),
    nodeRef: safeRefForAudience('oa-workroomd node ref', record.nodeRef, audience),
    policyRefs: safeRefsForAudience(
      'oa-workroomd policy refs',
      record.policyRefs,
      audience,
    ),
    publicArtifactRefs: safeRefsForAudience(
      'oa-workroomd public artifact refs',
      record.publicArtifactRefs,
      audience,
    ),
    routeRefs: safeRefsForAudience(
      'oa-workroomd route refs',
      record.routeRefs,
      audience,
    ),
    runtimeRef: safeRefForAudience(
      'oa-workroomd runtime ref',
      record.runtimeRef,
      audience,
    ),
    sessionRef: safeRefForAudience(
      'oa-workroomd session ref',
      record.sessionRef,
      audience,
    ),
    sourceAuthorityRefs: safeRefsForAudience(
      'oa-workroomd source authority refs',
      record.sourceAuthorityRefs,
      audience,
    ),
    status: record.status,
    trustLevel: record.trustLevel,
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRef: safeRefForAudience(
      'oa-workroomd workroom ref',
      record.workroomRef,
      audience,
    ),
    workspaceRef: safeRefForAudience(
      'oa-workroomd workspace ref',
      record.workspaceRef,
      audience,
    ),
  }

  if (openAgentsWorkroomdSessionProjectionHasPrivateMaterial(projection)) {
    throw new OpenAgentsWorkroomdSessionUnsafe({
      reason: 'oa-workroomd session projection contains unsafe material.',
    })
  }

  return projection
}

export const openAgentsWorkroomdSessionProjectionHasPrivateMaterial = (
  projection: OpenAgentsWorkroomdSessionProjection,
): boolean => {
  const serialized = JSON.stringify(projection)

  return unsafeWorkroomdRefPattern.test(serialized) ||
    rawTimestampPattern.test(serialized)
}

export const OPENAGENTS_WORKROOMD_CONFORMANCE_FIXTURES:
  ReadonlyArray<OpenAgentsWorkroomdSessionRecord> = [
    {
      archiveState: 'active',
      artifactManifestRefs: ['artifact_manifest.otec.codex_turn_1'],
      artifactRefs: ['artifact.otec.diff_summary'],
      assignmentRef: 'assignment.otec.site_revision',
      auditEvidenceRefs: ['audit_evidence.otec.session_1'],
      backendKind: 'shc_vm',
      cancellationRefs: [],
      cancellationState: 'none',
      closeoutCaveatRefs: ['caveat.closeout.customer_review_needed'],
      closeoutReceiptRefs: [],
      closeoutState: 'pending',
      correlationRefs: ['correlation.otec.session_1'],
      createdAtIso: '2026-06-07T01:50:00.000Z',
      daemonRef: 'daemon.oa_workroomd.bertha',
      eventKinds: [
        'session_created',
        'assignment_received',
        'grant_refs_resolved',
        'turn_started',
        'artifact_manifest_recorded',
      ],
      failureReceiptRefs: [],
      grantRefs: [
        'auth_grant.codex.account_1',
        'github_write_grant.otec.repo',
      ],
      grantResolutionRefs: ['grant_resolution.refs_present'],
      grantResolutionState: 'refs_present',
      id: 'oa_workroomd_session.otec.1',
      idempotencyRefs: ['idempotency.otec.session_1'],
      lifecycleEventRefs: ['lifecycle.otec.turn_started'],
      nodeRef: 'oa_node.bertha',
      policyRefs: ['policy.oa_workroomd.no_raw_credentials'],
      publicArtifactRefs: ['artifact.public.otec.preview'],
      routeRefs: ['route.shc_vm.codex'],
      runtimeRef: 'runtime.codex_cli',
      sessionRef: 'session.otec.1',
      sourceAuthorityRefs: ['source_authority.order.customer_summary'],
      status: 'closeout_ready',
      trustLevel: 'medium',
      updatedAtIso: '2026-06-07T01:58:00.000Z',
      workroomRef: 'workroom.otec.public',
      workspaceRef: 'workspace.otec.session_1',
    },
    {
      archiveState: 'archived',
      artifactManifestRefs: ['artifact_manifest.cancelled.summary'],
      artifactRefs: [],
      assignmentRef: 'assignment.cancelled.test',
      auditEvidenceRefs: ['audit_evidence.cancelled.session'],
      backendKind: 'gcloud_vm',
      cancellationRefs: ['cancel.operator_request'],
      cancellationState: 'cancelled',
      closeoutCaveatRefs: ['caveat.closeout.cancelled_by_operator'],
      closeoutReceiptRefs: ['receipt.closeout.cancelled'],
      closeoutState: 'emitted',
      correlationRefs: ['correlation.cancelled.session'],
      createdAtIso: '2026-06-07T01:30:00.000Z',
      daemonRef: 'daemon.oa_workroomd.pylon_candidate',
      eventKinds: [
        'cancellation_requested',
        'cancellation_acknowledged',
        'closeout_emitted',
        'archive_recorded',
      ],
      failureReceiptRefs: [],
      grantRefs: ['auth_grant.codex.account_2'],
      grantResolutionRefs: ['grant_resolution.refs_present'],
      grantResolutionState: 'refs_present',
      id: 'oa_workroomd_session.cancelled.1',
      idempotencyRefs: ['idempotency.cancelled.session'],
      lifecycleEventRefs: ['lifecycle.cancelled.closeout'],
      nodeRef: 'oa_node.pylon_candidate',
      policyRefs: ['policy.oa_workroomd.cancel_safe'],
      publicArtifactRefs: ['artifact.public.cancelled.summary'],
      routeRefs: ['route.gcloud_vm.codex'],
      runtimeRef: 'runtime.codex_cli',
      sessionRef: 'session.cancelled.1',
      sourceAuthorityRefs: ['source_authority.operator.cancel_request'],
      status: 'archived',
      trustLevel: 'medium',
      updatedAtIso: '2026-06-07T01:45:00.000Z',
      workroomRef: 'workroom.cancelled.public',
      workspaceRef: 'workspace.cancelled.session',
    },
  ]
