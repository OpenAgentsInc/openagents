import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const CodingAutopilotRepoMemoryKind = S.Literals([
  'accepted_fix',
  'build_command',
  'denied_path',
  'dependency_note',
  'flaky_test',
  'pr_style',
  'rejected_fix',
  'repo_convention',
  'reviewer_preference',
  'test_command',
])
export type CodingAutopilotRepoMemoryKind =
  typeof CodingAutopilotRepoMemoryKind.Type

export const CodingAutopilotRepoMemorySourceState = S.Literals([
  'accepted',
  'agent_inferred',
  'customer_supplied',
  'observed',
  'operator_reviewed',
  'rejected',
])
export type CodingAutopilotRepoMemorySourceState =
  typeof CodingAutopilotRepoMemorySourceState.Type

export const CodingAutopilotRepoMemoryStatus = S.Literals([
  'active',
  'archived',
  'blocked',
  'expired',
  'needs_review',
])
export type CodingAutopilotRepoMemoryStatus =
  typeof CodingAutopilotRepoMemoryStatus.Type

export const CodingAutopilotRepoMemoryRetrievalMode = S.Literals([
  'manual_review',
  'semantic_embedding',
  'typed_selector',
])
export type CodingAutopilotRepoMemoryRetrievalMode =
  typeof CodingAutopilotRepoMemoryRetrievalMode.Type

export const CodingAutopilotRepoMemoryRepoVisibility = S.Literals([
  'private',
  'public',
])
export type CodingAutopilotRepoMemoryRepoVisibility =
  typeof CodingAutopilotRepoMemoryRepoVisibility.Type

export class CodingAutopilotRepoMemoryRecord extends S.Class<CodingAutopilotRepoMemoryRecord>(
  'CodingAutopilotRepoMemoryRecord',
)({
  caveatRefs: S.Array(S.String),
  confidence: S.Number,
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  expiresAtIso: S.NullOr(S.String),
  id: S.String,
  keywordRoutingAllowed: S.Literal(false),
  memoryKind: CodingAutopilotRepoMemoryKind,
  memoryRef: S.String,
  missionRefs: S.Array(S.String),
  repoRef: S.String,
  repoVisibility: CodingAutopilotRepoMemoryRepoVisibility,
  retrievalMode: CodingAutopilotRepoMemoryRetrievalMode,
  reviewAfterIso: S.NullOr(S.String),
  selectorRefs: S.Array(S.String),
  semanticIndexRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  sourceState: CodingAutopilotRepoMemorySourceState,
  status: CodingAutopilotRepoMemoryStatus,
  summaryRef: S.String,
  updatedAtIso: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotRepoMemoryProjection extends S.Class<CodingAutopilotRepoMemoryProjection>(
  'CodingAutopilotRepoMemoryProjection',
)({
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  caveatRefs: S.Array(S.String),
  confidence: S.Number,
  confidenceBucket: S.String,
  createdAtDisplay: S.String,
  effectiveStatus: CodingAutopilotRepoMemoryStatus,
  evidenceRefs: S.Array(S.String),
  expiresAtDisplay: S.NullOr(S.String),
  id: S.String,
  keywordRoutingAllowed: S.Literal(false),
  memoryKind: CodingAutopilotRepoMemoryKind,
  memoryRef: S.String,
  missionRefs: S.Array(S.String),
  repoRef: S.String,
  repoVisibility: CodingAutopilotRepoMemoryRepoVisibility,
  retrievalMode: CodingAutopilotRepoMemoryRetrievalMode,
  reviewAfterDisplay: S.NullOr(S.String),
  selectorRefs: S.Array(S.String),
  semanticIndexRefs: S.Array(S.String),
  sourceAuthorityRefs: S.Array(S.String),
  sourceState: CodingAutopilotRepoMemorySourceState,
  status: CodingAutopilotRepoMemoryStatus,
  summaryRef: S.String,
  updatedAtDisplay: S.String,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotRepoMemoryUnsafe extends S.TaggedErrorClass<CodingAutopilotRepoMemoryUnsafe>()(
  'CodingAutopilotRepoMemoryUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(email|invoice|payment|payload|patch|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(repo\.private|source[_-]?authority|workroom\.)/i
const customerUnsafeRefPattern =
  /(source[_-]?authority|workroom\.private)/i
const teamUnsafeRefPattern =
  /(source[_-]?authority|workroom\.private)/i

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const confidenceBucket = (confidence: number): string => {
  if (confidence >= 0.85) {
    return 'high'
  }

  if (confidence >= 0.55) {
    return 'medium'
  }

  return 'low'
}

const assertSafeRefs = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotRepoMemoryUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, or raw artifact material.`,
    })
  }
}

const audienceUnsafePattern = (
  audience: BlueprintMissionBriefingAudience,
): RegExp | null => {
  if (audience === 'public') {
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
  audience: BlueprintMissionBriefingAudience,
): ReadonlyArray<string> => {
  assertSafeRefs(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const displayTime = (
  iso: string | null,
  nowIso: string,
): string | null =>
  iso === null ? null : friendlyBlueprintMissionBriefingTime(iso, nowIso)

const effectiveStatus = (
  record: CodingAutopilotRepoMemoryRecord,
  nowIso: string,
): CodingAutopilotRepoMemoryStatus => {
  if (
    record.status === 'archived' ||
    record.status === 'blocked' ||
    record.status === 'expired'
  ) {
    return record.status
  }

  if (
    record.expiresAtIso !== null &&
    Date.parse(record.expiresAtIso) <= Date.parse(nowIso)
  ) {
    return 'expired'
  }

  if (
    record.reviewAfterIso !== null &&
    Date.parse(record.reviewAfterIso) <= Date.parse(nowIso)
  ) {
    return 'needs_review'
  }

  return record.status
}

const assertRecordSafe = (
  record: CodingAutopilotRepoMemoryRecord,
): void => {
  assertSafeRefs('repo memory identity refs', [
    record.id,
    record.memoryRef,
    record.repoRef,
    record.summaryRef,
  ])
  assertSafeRefs('repo memory mission refs', record.missionRefs)
  assertSafeRefs('repo memory workroom refs', record.workroomRefs)
  assertSafeRefs('repo memory source authority refs', record.sourceAuthorityRefs)
  assertSafeRefs('repo memory evidence refs', record.evidenceRefs)
  assertSafeRefs('repo memory selector refs', record.selectorRefs)
  assertSafeRefs('repo memory semantic index refs', record.semanticIndexRefs)
  assertSafeRefs('repo memory caveat refs', record.caveatRefs)

  if (record.retrievalMode === 'typed_selector' && record.selectorRefs.length === 0) {
    throw new CodingAutopilotRepoMemoryUnsafe({
      reason: 'typed_selector repo memory requires selector refs.',
    })
  }

  if (
    record.retrievalMode === 'semantic_embedding' &&
    record.semanticIndexRefs.length === 0
  ) {
    throw new CodingAutopilotRepoMemoryUnsafe({
      reason: 'semantic_embedding repo memory requires semantic index refs.',
    })
  }
}

export const codingAutopilotRepoMemoryProjectionHasPrivateMaterial = (
  projection: CodingAutopilotRepoMemoryProjection,
): boolean =>
  universallyUnsafeRefPattern.test(JSON.stringify(projection)) ||
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(JSON.stringify(projection))

export const projectCodingAutopilotRepoMemory = (
  record: CodingAutopilotRepoMemoryRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotRepoMemoryProjection => {
  assertRecordSafe(record)

  const projection: CodingAutopilotRepoMemoryProjection = {
    audience,
    caveatRefs: safeRefsForAudience('repo memory caveat refs', record.caveatRefs, audience),
    confidence: record.confidence,
    confidenceBucket: confidenceBucket(record.confidence),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    effectiveStatus: effectiveStatus(record, nowIso),
    evidenceRefs: safeRefsForAudience('repo memory evidence refs', record.evidenceRefs, audience),
    expiresAtDisplay: displayTime(record.expiresAtIso, nowIso),
    id: record.id,
    keywordRoutingAllowed: false,
    memoryKind: record.memoryKind,
    memoryRef: record.memoryRef,
    missionRefs: safeRefsForAudience('repo memory mission refs', record.missionRefs, audience),
    repoRef: record.repoVisibility === 'public' || audience !== 'public'
      ? safeRefsForAudience('repo memory repo ref', [record.repoRef], audience)[0] ??
        'repo.redacted'
      : 'repo.redacted',
    repoVisibility: record.repoVisibility,
    retrievalMode: record.retrievalMode,
    reviewAfterDisplay: displayTime(record.reviewAfterIso, nowIso),
    selectorRefs: safeRefsForAudience('repo memory selector refs', record.selectorRefs, audience),
    semanticIndexRefs: safeRefsForAudience(
      'repo memory semantic index refs',
      record.semanticIndexRefs,
      audience,
    ),
    sourceAuthorityRefs: audience === 'operator'
      ? safeRefsForAudience('repo memory source authority refs', record.sourceAuthorityRefs, audience)
      : [],
    sourceState: record.sourceState,
    status: record.status,
    summaryRef: safeRefsForAudience('repo memory summary ref', [record.summaryRef], audience)[0] ??
      'summary.redacted',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('repo memory workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotRepoMemoryProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotRepoMemoryUnsafe({
      reason: 'Repo memory projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const exampleCodingAutopilotRepoMemoryRecords =
  (): ReadonlyArray<CodingAutopilotRepoMemoryRecord> => [
    {
      caveatRefs: ['caveat.repo_memory.evidence_only'],
      confidence: 0.91,
      createdAtIso: '2026-06-06T20:00:00.000Z',
      evidenceRefs: ['evidence.accepted_fix.otec_revision_4'],
      expiresAtIso: null,
      id: 'repo_memory_accepted_fix_otec_1',
      keywordRoutingAllowed: false,
      memoryKind: 'accepted_fix',
      memoryRef: 'memory.repo.accepted_fix.otec_1',
      missionRefs: ['mission.otec_revision_4'],
      repoRef: 'repo.OpenAgentsInc.otec_public',
      repoVisibility: 'public',
      retrievalMode: 'typed_selector',
      reviewAfterIso: null,
      selectorRefs: ['selector.repo_memory.accepted_fix'],
      semanticIndexRefs: [],
      sourceAuthorityRefs: ['source_authority.operator_reviewed_fix'],
      sourceState: 'accepted',
      status: 'active',
      summaryRef: 'summary.repo_memory.accepted_fix.otec_1',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
    {
      caveatRefs: ['caveat.repo_memory.customer_only'],
      confidence: 0.72,
      createdAtIso: '2026-06-06T20:20:00.000Z',
      evidenceRefs: ['evidence.flaky_test.customer_app'],
      expiresAtIso: '2026-06-07T21:00:00.000Z',
      id: 'repo_memory_flaky_test_customer_app',
      keywordRoutingAllowed: false,
      memoryKind: 'flaky_test',
      memoryRef: 'memory.repo.flaky_test.customer_app',
      missionRefs: ['mission.customer_app_fix'],
      repoRef: 'repo.customer_app',
      repoVisibility: 'private',
      retrievalMode: 'semantic_embedding',
      reviewAfterIso: '2026-06-06T20:30:00.000Z',
      selectorRefs: [],
      semanticIndexRefs: ['semantic_index.repo_memory.customer_app'],
      sourceAuthorityRefs: ['source_authority.test_result_review'],
      sourceState: 'observed',
      status: 'active',
      summaryRef: 'summary.repo_memory.flaky_test.customer_app',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      workroomRefs: ['workroom.customer_app_fix'],
    },
  ]
