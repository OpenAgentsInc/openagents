import { Schema as S } from 'effect'

import type { BlueprintMissionBriefingAudience } from './blueprint/schemas/continuation-mission-briefing'
import { friendlyBlueprintMissionBriefingTime } from './blueprint/services/continuation-mission-briefing'

export const CodingAutopilotArtifactKind = S.Literals([
  'build_log_summary',
  'customer_note',
  'diff_summary',
  'fulfillment_receipt',
  'patch_ref',
  'pr_draft',
  'pr_url',
  'preview_url',
  'redaction_report',
  'rollback_note',
  'screenshot_ref',
  'test_run',
])
export type CodingAutopilotArtifactKind =
  typeof CodingAutopilotArtifactKind.Type

export const CodingAutopilotArtifactStatus = S.Literals([
  'archived',
  'blocked',
  'draft',
  'failed',
  'ready',
  'superseded',
])
export type CodingAutopilotArtifactStatus =
  typeof CodingAutopilotArtifactStatus.Type

export const CodingAutopilotArtifactVisibility = S.Literals([
  'customer',
  'private',
  'public',
  'team',
])
export type CodingAutopilotArtifactVisibility =
  typeof CodingAutopilotArtifactVisibility.Type

export class CodingAutopilotArtifactRecord extends S.Class<CodingAutopilotArtifactRecord>(
  'CodingAutopilotArtifactRecord',
)({
  archivedAtIso: S.NullOr(S.String),
  artifactKind: CodingAutopilotArtifactKind,
  artifactRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  createdAtIso: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  publicSafe: S.Boolean,
  retentionCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: CodingAutopilotArtifactStatus,
  summaryRef: S.String,
  updatedAtIso: S.String,
  visibility: CodingAutopilotArtifactVisibility,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotArtifactProjection extends S.Class<CodingAutopilotArtifactProjection>(
  'CodingAutopilotArtifactProjection',
)({
  archivedAtDisplay: S.NullOr(S.String),
  artifactKind: CodingAutopilotArtifactKind,
  artifactRef: S.String,
  authorityReceiptRefs: S.Array(S.String),
  audience: S.Literals(['public', 'customer', 'team', 'operator']),
  caveatRefs: S.Array(S.String),
  createdAtDisplay: S.String,
  evidenceRefs: S.Array(S.String),
  id: S.String,
  missionRef: S.String,
  publicSafe: S.Boolean,
  retentionCaveatRefs: S.Array(S.String),
  sourceRefs: S.Array(S.String),
  status: CodingAutopilotArtifactStatus,
  summaryRef: S.String,
  updatedAtDisplay: S.String,
  visibility: CodingAutopilotArtifactVisibility,
  workroomRefs: S.Array(S.String),
}) {}

export class CodingAutopilotArtifactUnsafe extends S.TaggedErrorClass<CodingAutopilotArtifactUnsafe>()(
  'CodingAutopilotArtifactUnsafe',
  {
    reason: S.String,
  },
) {}

const safeRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const universallyUnsafeRefPattern =
  /(@|access[_-]?token|auth\.json|bearer|checkout_id=|cookie|customer[_-]?(email|name|value)|email[_-]?body|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|github\.com\/[^:/]+\/private|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|opencode_auth_content|payment[_-]?(hash|preimage)|preimage|private[_-]?key|private[_-]?repo|provider[_-]?(account|grant|payload|token)|raw[_-]?(build[_-]?log|email|invoice|patch|payment|payload|prompt|runner|run[_-]?log|source[_-]?archive|webhook)|runner[_-]?log|secret|sk-[a-z0-9]|source[_-]?archive|token|wallet|webhook[_-]?secret|workroom[_-]?private)/i
const publicUnsafeRefPattern =
  /(workroom\.|source\.private|artifact\.private)/i
const customerUnsafeRefPattern =
  /(source\.private|artifact\.private|workroom\.private)/i
const teamUnsafeRefPattern =
  /(source\.private|artifact\.private|workroom\.private)/i

const prWritebackKinds = new Set<CodingAutopilotArtifactKind>([
  'pr_draft',
  'pr_url',
])

const visibleToAudience = (
  visibility: CodingAutopilotArtifactVisibility,
  audience: BlueprintMissionBriefingAudience,
): boolean => {
  if (audience === 'operator') {
    return true
  }

  if (audience === 'team') {
    return visibility !== 'private'
  }

  if (audience === 'customer') {
    return visibility === 'customer' || visibility === 'public'
  }

  return visibility === 'public'
}

const uniqueRefs = (
  refs: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(refs.map(ref => ref.trim()).filter(ref => ref !== ''))].sort()

const assertNoUniversalPrivateMaterial = (
  label: string,
  refs: ReadonlyArray<string>,
): void => {
  const unsafe = uniqueRefs(refs).find(ref =>
    !safeRefPattern.test(ref) ||
    universallyUnsafeRefPattern.test(ref)
  )

  if (unsafe !== undefined) {
    throw new CodingAutopilotArtifactUnsafe({
      reason: `${label} contains private, secret, provider, runner, wallet, payment, customer, private repo, raw log, raw patch, or source archive material.`,
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
  assertNoUniversalPrivateMaterial(label, refs)

  const pattern = audienceUnsafePattern(audience)

  return pattern === null
    ? uniqueRefs(refs)
    : uniqueRefs(refs).filter(ref => !pattern.test(ref))
}

const safeNullableDisplay = (
  iso: string | null,
  nowIso: string,
): string | null =>
  iso === null ? null : friendlyBlueprintMissionBriefingTime(iso, nowIso)

const assertRecordSafe = (record: CodingAutopilotArtifactRecord): void => {
  assertNoUniversalPrivateMaterial('artifact identity refs', [
    record.id,
    record.artifactRef,
    record.missionRef,
    record.summaryRef,
  ])
  assertNoUniversalPrivateMaterial('artifact workroom refs', record.workroomRefs)
  assertNoUniversalPrivateMaterial('artifact source refs', record.sourceRefs)
  assertNoUniversalPrivateMaterial('artifact evidence refs', record.evidenceRefs)
  assertNoUniversalPrivateMaterial('artifact authority receipt refs', record.authorityReceiptRefs)
  assertNoUniversalPrivateMaterial('artifact caveat refs', record.caveatRefs)
  assertNoUniversalPrivateMaterial('artifact retention caveat refs', record.retentionCaveatRefs)

  if (record.visibility === 'public' && !record.publicSafe) {
    throw new CodingAutopilotArtifactUnsafe({
      reason: 'Public artifact visibility requires publicSafe true.',
    })
  }

  if (
    record.status === 'ready' &&
    record.evidenceRefs.length === 0 &&
    record.authorityReceiptRefs.length === 0
  ) {
    throw new CodingAutopilotArtifactUnsafe({
      reason: 'Ready artifacts require evidence refs or authority receipt refs.',
    })
  }

  if (
    prWritebackKinds.has(record.artifactKind) &&
    record.authorityReceiptRefs.length === 0
  ) {
    throw new CodingAutopilotArtifactUnsafe({
      reason: 'PR artifacts require authority receipt refs.',
    })
  }
}

export const codingAutopilotArtifactProjectionHasPrivateMaterial = (
  projection: CodingAutopilotArtifactProjection,
): boolean =>
  universallyUnsafeRefPattern.test(JSON.stringify(projection)) ||
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}/.test(JSON.stringify(projection))

export const projectCodingAutopilotArtifactRecord = (
  record: CodingAutopilotArtifactRecord,
  audience: BlueprintMissionBriefingAudience,
  nowIso: string,
): CodingAutopilotArtifactProjection | null => {
  assertRecordSafe(record)

  if (!visibleToAudience(record.visibility, audience)) {
    return null
  }

  const projection: CodingAutopilotArtifactProjection = {
    archivedAtDisplay: safeNullableDisplay(record.archivedAtIso, nowIso),
    artifactKind: record.artifactKind,
    artifactRef: safeRefsForAudience('artifact ref', [record.artifactRef], audience)[0] ??
      'artifact.redacted',
    authorityReceiptRefs: safeRefsForAudience(
      'artifact authority receipt refs',
      record.authorityReceiptRefs,
      audience,
    ),
    audience,
    caveatRefs: safeRefsForAudience('artifact caveat refs', record.caveatRefs, audience),
    createdAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.createdAtIso,
      nowIso,
    ),
    evidenceRefs: safeRefsForAudience('artifact evidence refs', record.evidenceRefs, audience),
    id: record.id,
    missionRef: record.missionRef,
    publicSafe: record.publicSafe,
    retentionCaveatRefs: safeRefsForAudience(
      'artifact retention caveat refs',
      record.retentionCaveatRefs,
      audience,
    ),
    sourceRefs: safeRefsForAudience('artifact source refs', record.sourceRefs, audience),
    status: record.status,
    summaryRef: safeRefsForAudience('artifact summary ref', [record.summaryRef], audience)[0] ??
      'summary.redacted',
    updatedAtDisplay: friendlyBlueprintMissionBriefingTime(
      record.updatedAtIso,
      nowIso,
    ),
    visibility: record.visibility,
    workroomRefs: audience === 'public'
      ? []
      : safeRefsForAudience('artifact workroom refs', record.workroomRefs, audience),
  }

  if (codingAutopilotArtifactProjectionHasPrivateMaterial(projection)) {
    throw new CodingAutopilotArtifactUnsafe({
      reason: 'Artifact projection contains private material or raw timestamps.',
    })
  }

  return projection
}

export const exampleCodingAutopilotArtifacts =
  (): ReadonlyArray<CodingAutopilotArtifactRecord> => [
    {
      archivedAtIso: null,
      artifactKind: 'diff_summary',
      artifactRef: 'artifact.diff_summary.otec_revision_4',
      authorityReceiptRefs: [],
      caveatRefs: ['caveat.diff_summary.summary_only'],
      createdAtIso: '2026-06-06T20:30:00.000Z',
      evidenceRefs: ['evidence.diff_summary.otec_revision_4'],
      id: 'artifact_otec_revision_4_diff_summary',
      missionRef: 'mission.otec_revision_4',
      publicSafe: true,
      retentionCaveatRefs: ['retention.summary_only'],
      sourceRefs: ['source.public_repo.commit_ref'],
      status: 'ready',
      summaryRef: 'summary.diff_summary.otec_revision_4',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      visibility: 'public',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
    {
      archivedAtIso: null,
      artifactKind: 'test_run',
      artifactRef: 'artifact.test_run.otec_revision_4',
      authorityReceiptRefs: [],
      caveatRefs: ['caveat.test_summary.summary_only'],
      createdAtIso: '2026-06-06T20:40:00.000Z',
      evidenceRefs: ['evidence.test_run.otec_revision_4'],
      id: 'artifact_otec_revision_4_test_run',
      missionRef: 'mission.otec_revision_4',
      publicSafe: true,
      retentionCaveatRefs: ['retention.summary_only'],
      sourceRefs: ['source.public_repo.test_command'],
      status: 'ready',
      summaryRef: 'summary.test_run.otec_revision_4',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      visibility: 'customer',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
    {
      archivedAtIso: null,
      artifactKind: 'pr_url',
      artifactRef: 'artifact.pr_url.otec_revision_4',
      authorityReceiptRefs: ['authority_receipt.github_writeback.otec_revision_4'],
      caveatRefs: ['caveat.pr_customer_review_required'],
      createdAtIso: '2026-06-06T20:50:00.000Z',
      evidenceRefs: ['evidence.pr_url.otec_revision_4'],
      id: 'artifact_otec_revision_4_pr_url',
      missionRef: 'mission.otec_revision_4',
      publicSafe: true,
      retentionCaveatRefs: ['retention.pr_metadata_only'],
      sourceRefs: ['source.github_pr.public_fork'],
      status: 'ready',
      summaryRef: 'summary.pr_url.otec_revision_4',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      visibility: 'customer',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
    {
      archivedAtIso: null,
      artifactKind: 'build_log_summary',
      artifactRef: 'artifact.build_log_summary.otec_revision_4',
      authorityReceiptRefs: [],
      caveatRefs: ['caveat.build_summary.summary_only'],
      createdAtIso: '2026-06-06T20:55:00.000Z',
      evidenceRefs: ['evidence.build_log_summary.otec_revision_4'],
      id: 'artifact_otec_revision_4_build_log_summary',
      missionRef: 'mission.otec_revision_4',
      publicSafe: false,
      retentionCaveatRefs: ['retention.summary_only'],
      sourceRefs: ['source.runner_summary'],
      status: 'ready',
      summaryRef: 'summary.build_log_summary.otec_revision_4',
      updatedAtIso: '2026-06-06T21:00:00.000Z',
      visibility: 'team',
      workroomRefs: ['workroom.otec_site_revision_4'],
    },
  ]
