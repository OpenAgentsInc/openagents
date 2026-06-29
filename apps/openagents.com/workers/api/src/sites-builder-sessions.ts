import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Effect, Schema as S } from 'effect'

import { parseJsonRecord } from './json-boundary'
import { compactRandomId, currentIsoTimestamp } from './runtime-primitives'

export const SiteBuilderSessionStatus = S.Literals([
  'draft',
  'planning',
  'building',
  'preview_ready',
  'review_ready',
  'saved',
  'deploying',
  'deployed',
  'failed',
  'archived',
])
export type SiteBuilderSessionStatus = typeof SiteBuilderSessionStatus.Type

export const SiteBuilderVisibility = S.Literals([
  'customer',
  'operator',
  'internal',
])
export type SiteBuilderVisibility = typeof SiteBuilderVisibility.Type

export const SiteBuilderActorKind = S.Literals([
  'customer',
  'agent',
  'operator',
  'system',
])
export type SiteBuilderActorKind = typeof SiteBuilderActorKind.Type

export const SiteBuilderPhaseKind = S.Literals([
  'planning',
  'foundation',
  'core',
  'styling',
  'integration',
  'optimization',
  'preview',
  'save',
  'deploy',
])
export type SiteBuilderPhaseKind = typeof SiteBuilderPhaseKind.Type

export const SiteBuilderPhaseStatus = S.Literals([
  'queued',
  'running',
  'succeeded',
  'failed',
  'blocked',
  'skipped',
])
export type SiteBuilderPhaseStatus = typeof SiteBuilderPhaseStatus.Type

export const SiteBuilderEventKind = S.Literals([
  'session_created',
  'message_added',
  'phase_started',
  'phase_updated',
  'phase_completed',
  'file_changed',
  'preview_created',
  'artifact_created',
  'build_failed',
  'build_repaired',
  'save_requested',
  'deploy_requested',
  'error',
])
export type SiteBuilderEventKind = typeof SiteBuilderEventKind.Type

export const SiteBuilderPreviewKind = S.Literals([
  'static_r2',
  'workers_for_platforms',
  'container',
])
export type SiteBuilderPreviewKind = typeof SiteBuilderPreviewKind.Type

export const SiteBuilderPreviewStatus = S.Literals([
  'requested',
  'building',
  'ready',
  'failed',
  'expired',
])
export type SiteBuilderPreviewStatus = typeof SiteBuilderPreviewStatus.Type

export const SiteBuilderArtifactKind = S.Literals([
  'source_archive',
  'build_manifest',
  'preview_bundle',
  'deployable_worker',
  'receipt',
])
export type SiteBuilderArtifactKind = typeof SiteBuilderArtifactKind.Type

export const SiteBuilderSessionRecord = S.Struct({
  activeArtifactId: S.NullOr(S.String),
  activePreviewId: S.NullOr(S.String),
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  createdByActorRef: S.String,
  customerUserId: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  orderId: S.NullOr(S.String),
  ownerUserId: S.String,
  promptSummary: S.String,
  siteId: S.NullOr(S.String),
  sourceRevisionId: S.NullOr(S.String),
  sourceSiteVersionId: S.NullOr(S.String),
  status: SiteBuilderSessionStatus,
  updatedAt: S.String,
  workroomId: S.NullOr(S.String),
})
export type SiteBuilderSessionRecord = typeof SiteBuilderSessionRecord.Type

export const SiteBuilderMessageRecord = S.Struct({
  actorKind: SiteBuilderActorKind,
  archivedAt: S.NullOr(S.String),
  body: S.String,
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  sequence: S.Number,
  sessionId: S.String,
  visibility: SiteBuilderVisibility,
})
export type SiteBuilderMessageRecord = typeof SiteBuilderMessageRecord.Type

export const SiteBuilderEventRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  createdAt: S.String,
  eventKind: SiteBuilderEventKind,
  id: S.String,
  idempotencyKey: S.String,
  payload: S.Record(S.String, S.Unknown),
  phaseKind: S.NullOr(SiteBuilderPhaseKind),
  sequence: S.Number,
  sessionId: S.String,
  sourceRef: S.NullOr(S.String),
  status: SiteBuilderPhaseStatus,
  summary: S.String,
  title: S.String,
  visibility: SiteBuilderVisibility,
})
export type SiteBuilderEventRecord = typeof SiteBuilderEventRecord.Type

export const SiteBuilderPhaseRunRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  completedAt: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  phaseKind: SiteBuilderPhaseKind,
  sequence: S.Number,
  sessionId: S.String,
  startedAt: S.NullOr(S.String),
  status: SiteBuilderPhaseStatus,
  summary: S.String,
  title: S.String,
})
export type SiteBuilderPhaseRunRecord = typeof SiteBuilderPhaseRunRecord.Type

export const PublicSiteBuilderPhaseProjection = S.Struct({
  phaseKind: SiteBuilderPhaseKind,
  sequence: S.Number,
  status: SiteBuilderPhaseStatus,
  summary: S.String,
  title: S.String,
})
export type PublicSiteBuilderPhaseProjection =
  typeof PublicSiteBuilderPhaseProjection.Type

export const PublicSiteBuilderPreviewProjection = S.Struct({
  id: S.String,
  previewUrl: S.NullOr(S.String),
  status: SiteBuilderPreviewStatus,
  updatedAt: S.String,
})
export type PublicSiteBuilderPreviewProjection =
  typeof PublicSiteBuilderPreviewProjection.Type

export const SiteBuilderFileSnapshotRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  artifactRef: S.NullOr(S.String),
  byteSize: S.Number,
  contentHash: S.String,
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  language: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  path: S.String,
  previewText: S.NullOr(S.String),
  sequence: S.Number,
  sessionId: S.String,
  sourceRef: S.NullOr(S.String),
  updatedAt: S.String,
  visibility: SiteBuilderVisibility,
})
export type SiteBuilderFileSnapshotRecord =
  typeof SiteBuilderFileSnapshotRecord.Type

export const SiteBuilderPreviewRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  artifactRef: S.NullOr(S.String),
  createdAt: S.String,
  healthRef: S.NullOr(S.String),
  id: S.String,
  idempotencyKey: S.String,
  metadata: S.Record(S.String, S.Unknown),
  previewKind: SiteBuilderPreviewKind,
  previewUrl: S.NullOr(S.String),
  sessionId: S.String,
  status: SiteBuilderPreviewStatus,
  updatedAt: S.String,
  versionRef: S.NullOr(S.String),
})
export type SiteBuilderPreviewRecord = typeof SiteBuilderPreviewRecord.Type

export const SiteBuilderArtifactRecord = S.Struct({
  archivedAt: S.NullOr(S.String),
  artifactKind: SiteBuilderArtifactKind,
  artifactRef: S.String,
  byteSize: S.NullOr(S.Number),
  contentHash: S.NullOr(S.String),
  createdAt: S.String,
  id: S.String,
  idempotencyKey: S.String,
  manifestRef: S.NullOr(S.String),
  metadata: S.Record(S.String, S.Unknown),
  sessionId: S.String,
})
export type SiteBuilderArtifactRecord = typeof SiteBuilderArtifactRecord.Type

export const PublicSiteBuilderSessionProjection = S.Struct({
  activePreview: S.NullOr(PublicSiteBuilderPreviewProjection),
  activePreviewId: S.NullOr(S.String),
  createdAt: S.String,
  currentPhase: S.NullOr(PublicSiteBuilderPhaseProjection),
  id: S.String,
  messages: S.Array(
    S.Struct({
      actorKind: SiteBuilderActorKind,
      body: S.String,
      createdAt: S.String,
      id: S.String,
      sequence: S.Number,
    }),
  ),
  orderId: S.NullOr(S.String),
  phases: S.Array(PublicSiteBuilderPhaseProjection),
  promptSummary: S.String,
  siteId: S.NullOr(S.String),
  status: SiteBuilderSessionStatus,
  updatedAt: S.String,
})
export type PublicSiteBuilderSessionProjection =
  typeof PublicSiteBuilderSessionProjection.Type

export const OperatorSiteBuilderSessionProjection = S.Struct({
  activeArtifactId: S.NullOr(S.String),
  activePreviewId: S.NullOr(S.String),
  artifactCount: S.Number,
  createdAt: S.String,
  customerUserId: S.NullOr(S.String),
  eventCount: S.Number,
  fileCount: S.Number,
  hasMetadata: S.Boolean,
  id: S.String,
  orderId: S.NullOr(S.String),
  ownerUserId: S.String,
  phaseCount: S.Number,
  phaseCurrent: S.NullOr(PublicSiteBuilderPhaseProjection),
  previewCount: S.Number,
  promptSummary: S.String,
  siteId: S.NullOr(S.String),
  status: SiteBuilderSessionStatus,
  updatedAt: S.String,
  workroomId: S.NullOr(S.String),
})
export type OperatorSiteBuilderSessionProjection =
  typeof OperatorSiteBuilderSessionProjection.Type

export type SiteBuilderRuntime = Readonly<{
  nowIso: () => string
  randomId: (prefix: string) => string
}>

export const systemSiteBuilderRuntime: SiteBuilderRuntime = {
  nowIso: currentIsoTimestamp,
  randomId: compactRandomId,
}

export type CreateSiteBuilderSessionInput = Readonly<{
  createdByActorRef: string
  customerUserId?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  orderId?: string | undefined
  ownerUserId: string
  promptSummary: string
  siteId?: string | undefined
  sourceRevisionId?: string | undefined
  sourceSiteVersionId?: string | undefined
  status?: SiteBuilderSessionStatus | undefined
  workroomId?: string | undefined
}>

export type AppendSiteBuilderMessageInput = Readonly<{
  actorKind: SiteBuilderActorKind
  body: string
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  sequence?: number | undefined
  sessionId: string
  visibility?: SiteBuilderVisibility | undefined
}>

export type AppendSiteBuilderEventInput = Readonly<{
  eventKind: SiteBuilderEventKind
  id?: string | undefined
  idempotencyKey: string
  payload?: Readonly<Record<string, unknown>> | undefined
  phaseKind?: SiteBuilderPhaseKind | undefined
  sequence?: number | undefined
  sessionId: string
  sourceRef?: string | undefined
  status?: SiteBuilderPhaseStatus | undefined
  summary: string
  title: string
  visibility?: SiteBuilderVisibility | undefined
}>

export type RecordSiteBuilderPhaseRunInput = Readonly<{
  completedAt?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  phaseKind: SiteBuilderPhaseKind
  sequence?: number | undefined
  sessionId: string
  startedAt?: string | undefined
  status: SiteBuilderPhaseStatus
  summary: string
  title: string
  visibility?: SiteBuilderVisibility | undefined
}>

export type UpsertSiteBuilderFileSnapshotInput = Readonly<{
  artifactRef?: string | undefined
  byteSize: number
  contentHash: string
  id?: string | undefined
  idempotencyKey: string
  language?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  path: string
  previewText?: string | undefined
  sequence?: number | undefined
  sessionId: string
  sourceRef?: string | undefined
  visibility?: SiteBuilderVisibility | undefined
}>

export type RecordSiteBuilderPreviewInput = Readonly<{
  artifactRef?: string | undefined
  healthRef?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  metadata?: Readonly<Record<string, unknown>> | undefined
  previewKind: SiteBuilderPreviewKind
  previewUrl?: string | undefined
  sessionId: string
  status?: SiteBuilderPreviewStatus | undefined
  versionRef?: string | undefined
}>

export type RecordSiteBuilderArtifactInput = Readonly<{
  artifactKind: SiteBuilderArtifactKind
  artifactRef: string
  byteSize?: number | undefined
  contentHash?: string | undefined
  id?: string | undefined
  idempotencyKey: string
  manifestRef?: string | undefined
  metadata?: Readonly<Record<string, unknown>> | undefined
  sessionId: string
}>

export type ListSiteBuilderEventsAfterInput = Readonly<{
  cursor?: number | undefined
  limit?: number | undefined
  sessionId: string
}>

export type ListSiteBuilderFileSnapshotsInput = Readonly<{
  limit?: number | undefined
  sessionId: string
}>

export type ReadSiteBuilderFileSnapshotInput = Readonly<{
  path: string
  sessionId: string
}>

type SiteBuilderSessionRow = Readonly<{
  active_artifact_id: string | null
  active_preview_id: string | null
  archived_at: string | null
  created_at: string
  created_by_actor_ref: string
  customer_user_id: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  order_id: string | null
  owner_user_id: string
  prompt_summary: string
  site_id: string | null
  source_revision_id: string | null
  source_site_version_id: string | null
  status: SiteBuilderSessionStatus
  updated_at: string
  workroom_id: string | null
}>

type SiteBuilderMessageRow = Readonly<{
  actor_kind: SiteBuilderActorKind
  archived_at: string | null
  body: string
  created_at: string
  id: string
  idempotency_key: string
  metadata_json: string
  sequence: number
  session_id: string
  visibility: SiteBuilderVisibility
}>

type SiteBuilderEventRow = Readonly<{
  archived_at: string | null
  created_at: string
  event_kind: SiteBuilderEventKind
  id: string
  idempotency_key: string
  payload_json: string
  phase_kind: SiteBuilderPhaseKind | null
  sequence: number
  session_id: string
  source_ref: string | null
  status: SiteBuilderPhaseStatus
  summary: string
  title: string
  visibility: SiteBuilderVisibility
}>

type SiteBuilderPhaseRunRow = Readonly<{
  archived_at: string | null
  completed_at: string | null
  created_at: string
  id: string
  idempotency_key: string
  metadata_json: string
  phase_kind: SiteBuilderPhaseKind
  sequence: number
  session_id: string
  started_at: string | null
  status: SiteBuilderPhaseStatus
  summary: string
  title: string
}>

type SiteBuilderFileSnapshotRow = Readonly<{
  archived_at: string | null
  artifact_ref: string | null
  byte_size: number
  content_hash: string
  created_at: string
  id: string
  idempotency_key: string
  language: string | null
  metadata_json: string
  path: string
  preview_text: string | null
  sequence: number
  session_id: string
  source_ref: string | null
  updated_at: string
  visibility: SiteBuilderVisibility
}>

type SiteBuilderPreviewRow = Readonly<{
  archived_at: string | null
  artifact_ref: string | null
  created_at: string
  health_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  preview_kind: SiteBuilderPreviewKind
  preview_url: string | null
  session_id: string
  status: SiteBuilderPreviewStatus
  updated_at: string
  version_ref: string | null
}>

type SiteBuilderArtifactRow = Readonly<{
  archived_at: string | null
  artifact_kind: SiteBuilderArtifactKind
  artifact_ref: string
  byte_size: number | null
  content_hash: string | null
  created_at: string
  id: string
  idempotency_key: string
  manifest_ref: string | null
  metadata_json: string
  session_id: string
}>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,220}$/
const SAFE_PATH_PATTERN =
  /^(?:[A-Za-z0-9_.-]+\/)*[A-Za-z0-9_.-]+(?:\.[A-Za-z0-9_.-]+)?$/
const SAFE_HASH_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/=-]{6,180}$/
const PROHIBITED_TEXT_PATTERN =
  /\b(provider[_ -]?payload|provider[_ -]?account|raw[_ -]?payload|runner[_ -]?payload|browser[_ -]?log|auth[_ -]?grant|access_token|refresh_token|device_auth_id|code_verifier|token_hash|private_key|wallet_secret|mdk_access_token|payment_preimage|payment_secret|webhook_secret|gho_[a-z0-9_]+|lnbc|lntb|lnbcrt|lno1|xprv|mnemonic|bypass|captcha|cloudflare challenge|headless stealth|anti-bot)/i

const compactText = (value: string, maxLength: number): string =>
  value.trim().replace(/\s+/g, ' ').slice(0, maxLength)

const textIsSafe = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_TEXT_PATTERN.test(value)

const metadataIsSafe = (
  metadata: Readonly<Record<string, unknown>>,
): boolean => {
  const json = JSON.stringify(metadata)

  return textIsSafe(json)
}

const assertSafeRef = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_REF_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be a public-safe ref without private runner, provider, payment, wallet, or bypass material.`,
    })
  }
}

const assertSafeHash = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  if (!SAFE_HASH_PATTERN.test(value) || !textIsSafe(value)) {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be a public-safe hash or artifact digest.`,
    })
  }
}

const assertSafeText = (
  field: string,
  value: string,
  maxLength: number,
): string => {
  const compact = compactText(value, maxLength)

  if (compact === '' || !textIsSafe(compact)) {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be bounded public-safe text.`,
    })
  }

  return compact
}

const assertSafeMetadata = (
  metadata: Readonly<Record<string, unknown>> | undefined,
): Readonly<Record<string, unknown>> => {
  const safeMetadata = metadata ?? {}

  if (!metadataIsSafe(safeMetadata)) {
    throw new SiteBuilderSessionValidationError({
      reason:
        'metadata must not contain private runner, provider, payment, wallet, source, or bypass material.',
    })
  }

  return safeMetadata
}

const assertSafePath = (path: string): string => {
  const normalized = path.trim().replace(/^\/+/, '')

  if (
    normalized === '' ||
    normalized.includes('..') ||
    normalized.length > 240 ||
    !SAFE_PATH_PATTERN.test(normalized) ||
    !textIsSafe(normalized)
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: 'path must be a bounded relative generated-file path.',
    })
  }

  return normalized
}

const assertSafeUrl = (field: string, value: string | undefined): void => {
  if (value === undefined) {
    return
  }

  let url: URL
  try {
    url = new URL(value)
  } catch {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be a valid https URL.`,
    })
  }

  if (
    url.protocol !== 'https:' ||
    !url.hostname.endsWith('openagents.com') ||
    !textIsSafe(value)
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: `${field} must be a public-safe OpenAgents https URL.`,
    })
  }
}

export class SiteBuilderSessionValidationError extends S.TaggedErrorClass<SiteBuilderSessionValidationError>()(
  'SiteBuilderSessionValidationError',
  {
    reason: S.String,
  },
) {}

export class SiteBuilderSessionStorageError extends S.TaggedErrorClass<SiteBuilderSessionStorageError>()(
  'SiteBuilderSessionStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

const jsonRecord = (value: string): Readonly<Record<string, unknown>> =>
  parseJsonRecord(value) ?? {}

const sessionFromRow = (
  row: SiteBuilderSessionRow,
): SiteBuilderSessionRecord => ({
  activeArtifactId: row.active_artifact_id,
  activePreviewId: row.active_preview_id,
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  createdByActorRef: row.created_by_actor_ref,
  customerUserId: row.customer_user_id,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: jsonRecord(row.metadata_json),
  orderId: row.order_id,
  ownerUserId: row.owner_user_id,
  promptSummary: row.prompt_summary,
  siteId: row.site_id,
  sourceRevisionId: row.source_revision_id,
  sourceSiteVersionId: row.source_site_version_id,
  status: row.status,
  updatedAt: row.updated_at,
  workroomId: row.workroom_id,
})

const messageFromRow = (
  row: SiteBuilderMessageRow,
): SiteBuilderMessageRecord => ({
  actorKind: row.actor_kind,
  archivedAt: row.archived_at,
  body: row.body,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: jsonRecord(row.metadata_json),
  sequence: row.sequence,
  sessionId: row.session_id,
  visibility: row.visibility,
})

const eventFromRow = (row: SiteBuilderEventRow): SiteBuilderEventRecord => ({
  archivedAt: row.archived_at,
  createdAt: row.created_at,
  eventKind: row.event_kind,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  payload: jsonRecord(row.payload_json),
  phaseKind: row.phase_kind,
  sequence: row.sequence,
  sessionId: row.session_id,
  sourceRef: row.source_ref,
  status: row.status,
  summary: row.summary,
  title: row.title,
  visibility: row.visibility,
})

const phaseFromRow = (
  row: SiteBuilderPhaseRunRow,
): SiteBuilderPhaseRunRecord => ({
  archivedAt: row.archived_at,
  completedAt: row.completed_at,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: jsonRecord(row.metadata_json),
  phaseKind: row.phase_kind,
  sequence: row.sequence,
  sessionId: row.session_id,
  startedAt: row.started_at,
  status: row.status,
  summary: row.summary,
  title: row.title,
})

const publicPhaseProjection = (
  phase: SiteBuilderPhaseRunRecord,
): PublicSiteBuilderPhaseProjection => ({
  phaseKind: phase.phaseKind,
  sequence: phase.sequence,
  status: phase.status,
  summary: phase.summary,
  title: phase.title,
})

const publicPreviewProjection = (
  preview: SiteBuilderPreviewRecord | null,
): PublicSiteBuilderPreviewProjection | null =>
  preview === null
    ? null
    : {
        id: preview.id,
        previewUrl: preview.previewUrl,
        status: preview.status,
        updatedAt: preview.updatedAt,
      }

const currentPhaseProjection = (
  phases: ReadonlyArray<SiteBuilderPhaseRunRecord>,
): PublicSiteBuilderPhaseProjection | null => {
  const current = [...phases].sort(
    (left, right) => right.sequence - left.sequence,
  )[0]

  return current === undefined ? null : publicPhaseProjection(current)
}

const fileFromRow = (
  row: SiteBuilderFileSnapshotRow,
): SiteBuilderFileSnapshotRecord => ({
  archivedAt: row.archived_at,
  artifactRef: row.artifact_ref,
  byteSize: row.byte_size,
  contentHash: row.content_hash,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  language: row.language,
  metadata: jsonRecord(row.metadata_json),
  path: row.path,
  previewText: row.preview_text,
  sequence: row.sequence,
  sessionId: row.session_id,
  sourceRef: row.source_ref,
  updatedAt: row.updated_at,
  visibility: row.visibility,
})

const previewFromRow = (
  row: SiteBuilderPreviewRow,
): SiteBuilderPreviewRecord => ({
  archivedAt: row.archived_at,
  artifactRef: row.artifact_ref,
  createdAt: row.created_at,
  healthRef: row.health_ref,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  metadata: jsonRecord(row.metadata_json),
  previewKind: row.preview_kind,
  previewUrl: row.preview_url,
  sessionId: row.session_id,
  status: row.status,
  updatedAt: row.updated_at,
  versionRef: row.version_ref,
})

const artifactFromRow = (
  row: SiteBuilderArtifactRow,
): SiteBuilderArtifactRecord => ({
  archivedAt: row.archived_at,
  artifactKind: row.artifact_kind,
  artifactRef: row.artifact_ref,
  byteSize: row.byte_size,
  contentHash: row.content_hash,
  createdAt: row.created_at,
  id: row.id,
  idempotencyKey: row.idempotency_key,
  manifestRef: row.manifest_ref,
  metadata: jsonRecord(row.metadata_json),
  sessionId: row.session_id,
})

const storageError = (operation: string, error: unknown) =>
  new SiteBuilderSessionStorageError({
    operation,
    reason: error instanceof Error ? error.message : String(error),
  })

const readSessionById = (
  db: D1Database,
  id: string,
): Effect.Effect<
  SiteBuilderSessionRecord | null,
  SiteBuilderSessionStorageError
> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT *
             FROM site_builder_sessions
            WHERE id = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(id)
        .first<SiteBuilderSessionRow>()

      return row === null ? null : sessionFromRow(row)
    },
    catch: error => storageError('readSessionById', error),
  })

const readByIdempotencyKey = <Record, Row>(
  db: D1Database,
  table: string,
  idempotencyKey: string,
  fromRow: (row: Row) => Record,
  operation: string,
): Effect.Effect<Record | null, SiteBuilderSessionStorageError> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT *
             FROM ${table}
            WHERE idempotency_key = ?
              AND archived_at IS NULL
            LIMIT 1`,
        )
        .bind(idempotencyKey)
        .first<Row>()

      return row === null ? null : fromRow(row)
    },
    catch: error => storageError(operation, error),
  })

const nextSequence = (
  db: D1Database,
  table: string,
  sessionId: string,
): Effect.Effect<number, SiteBuilderSessionStorageError> =>
  Effect.tryPromise({
    try: async () => {
      const row = await db
        .prepare(
          `SELECT COALESCE(MAX(sequence), 0) + 1 AS next_sequence
             FROM ${table}
            WHERE session_id = ?
              AND archived_at IS NULL`,
        )
        .bind(sessionId)
        .first<{ next_sequence: number }>()

      return row?.next_sequence ?? 1
    },
    catch: error => storageError(`nextSequence.${table}`, error),
  })

const assertSessionExists = (
  db: D1Database,
  sessionId: string,
): Effect.Effect<SiteBuilderSessionRecord, SiteBuilderSessionStorageError> =>
  readSessionById(db, sessionId).pipe(
    Effect.flatMap(session =>
      session === null
        ? Effect.fail(
            new SiteBuilderSessionStorageError({
              operation: 'assertSessionExists',
              reason: 'site builder session was not found.',
            }),
          )
        : Effect.succeed(session),
    ),
  )

const validateCreateSession = (
  input: CreateSiteBuilderSessionInput,
): Readonly<{
  metadata: Readonly<Record<string, unknown>>
  promptSummary: string
  status: SiteBuilderSessionStatus
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('siteId', input.siteId)
  assertSafeRef('orderId', input.orderId)
  assertSafeRef('workroomId', input.workroomId)
  assertSafeRef('ownerUserId', input.ownerUserId)
  assertSafeRef('customerUserId', input.customerUserId)
  assertSafeRef('createdByActorRef', input.createdByActorRef)
  assertSafeRef('sourceSiteVersionId', input.sourceSiteVersionId)
  assertSafeRef('sourceRevisionId', input.sourceRevisionId)

  return {
    metadata: assertSafeMetadata(input.metadata),
    promptSummary: assertSafeText('promptSummary', input.promptSummary, 360),
    status: input.status ?? 'draft',
  }
}

const validateMessage = (
  input: AppendSiteBuilderMessageInput,
): Readonly<{
  body: string
  metadata: Readonly<Record<string, unknown>>
  visibility: SiteBuilderVisibility
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)

  return {
    body: assertSafeText('body', input.body, 2000),
    metadata: assertSafeMetadata(input.metadata),
    visibility: input.visibility ?? 'customer',
  }
}

const validateEvent = (
  input: AppendSiteBuilderEventInput,
): Readonly<{
  payload: Readonly<Record<string, unknown>>
  status: SiteBuilderPhaseStatus
  summary: string
  title: string
  visibility: SiteBuilderVisibility
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('sourceRef', input.sourceRef)

  return {
    payload: assertSafeMetadata(input.payload),
    status: input.status ?? 'running',
    summary: assertSafeText('summary', input.summary, 1000),
    title: assertSafeText('title', input.title, 160),
    visibility: input.visibility ?? 'customer',
  }
}

const validatePhaseRun = (
  input: RecordSiteBuilderPhaseRunInput,
): Readonly<{
  completedAt: string | null
  metadata: Readonly<Record<string, unknown>>
  sequence: number | undefined
  startedAt: string | null
  summary: string
  title: string
  visibility: SiteBuilderVisibility
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('startedAt', input.startedAt)
  assertSafeRef('completedAt', input.completedAt)

  if (
    input.sequence !== undefined &&
    (!Number.isSafeInteger(input.sequence) || input.sequence < 1)
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: 'sequence must be a positive safe integer.',
    })
  }

  return {
    completedAt: input.completedAt ?? null,
    metadata: assertSafeMetadata(input.metadata),
    sequence: input.sequence,
    startedAt: input.startedAt ?? null,
    summary: assertSafeText('summary', input.summary, 1000),
    title: assertSafeText('title', input.title, 160),
    visibility: input.visibility ?? 'customer',
  }
}

const validateFile = (
  input: UpsertSiteBuilderFileSnapshotInput,
): Readonly<{
  metadata: Readonly<Record<string, unknown>>
  path: string
  previewText: string | null
  visibility: SiteBuilderVisibility
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('sourceRef', input.sourceRef)
  assertSafeRef('artifactRef', input.artifactRef)
  assertSafeRef('language', input.language)
  assertSafeHash('contentHash', input.contentHash)

  if (!Number.isSafeInteger(input.byteSize) || input.byteSize < 0) {
    throw new SiteBuilderSessionValidationError({
      reason: 'byteSize must be a non-negative safe integer.',
    })
  }

  return {
    metadata: assertSafeMetadata(input.metadata),
    path: assertSafePath(input.path),
    previewText:
      input.previewText === undefined
        ? null
        : assertSafeText('previewText', input.previewText, 4000),
    visibility: input.visibility ?? 'customer',
  }
}

const validatePreview = (
  input: RecordSiteBuilderPreviewInput,
): Readonly<{
  metadata: Readonly<Record<string, unknown>>
  status: SiteBuilderPreviewStatus
}> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('versionRef', input.versionRef)
  assertSafeRef('artifactRef', input.artifactRef)
  assertSafeRef('healthRef', input.healthRef)
  assertSafeUrl('previewUrl', input.previewUrl)

  return {
    metadata: assertSafeMetadata(input.metadata),
    status: input.status ?? 'requested',
  }
}

const validateArtifact = (
  input: RecordSiteBuilderArtifactInput,
): Readonly<{ metadata: Readonly<Record<string, unknown>> }> => {
  assertSafeRef('id', input.id)
  assertSafeRef('idempotencyKey', input.idempotencyKey)
  assertSafeRef('sessionId', input.sessionId)
  assertSafeRef('artifactRef', input.artifactRef)
  assertSafeRef('manifestRef', input.manifestRef)
  assertSafeHash('contentHash', input.contentHash)

  if (
    input.byteSize !== undefined &&
    (!Number.isSafeInteger(input.byteSize) || input.byteSize < 0)
  ) {
    throw new SiteBuilderSessionValidationError({
      reason: 'byteSize must be a non-negative safe integer.',
    })
  }

  return {
    metadata: assertSafeMetadata(input.metadata),
  }
}

const validationEffect = <A>(validate: () => A) =>
  Effect.try({
    try: validate,
    catch: error =>
      error instanceof SiteBuilderSessionValidationError
        ? error
        : new SiteBuilderSessionValidationError({
            reason: error instanceof Error ? error.message : String(error),
          }),
  })

export const publicSiteBuilderSessionProjection = (
  session: SiteBuilderSessionRecord,
  messages: ReadonlyArray<SiteBuilderMessageRecord>,
  phases: ReadonlyArray<SiteBuilderPhaseRunRecord> = [],
  previews: ReadonlyArray<SiteBuilderPreviewRecord> = [],
): PublicSiteBuilderSessionProjection => ({
  activePreview: publicPreviewProjection(
    previews.find(preview => preview.id === session.activePreviewId) ??
      previews[0] ??
      null,
  ),
  activePreviewId: session.activePreviewId,
  createdAt: session.createdAt,
  currentPhase: currentPhaseProjection(phases),
  id: session.id,
  messages: messages
    .filter(message => message.visibility === 'customer')
    .map(message => ({
      actorKind: message.actorKind,
      body: message.body,
      createdAt: message.createdAt,
      id: message.id,
      sequence: message.sequence,
    })),
  orderId: session.orderId,
  phases: phases.map(publicPhaseProjection),
  promptSummary: session.promptSummary,
  siteId: session.siteId,
  status: session.status,
  updatedAt: session.updatedAt,
})

export const operatorSiteBuilderSessionProjection = (
  session: SiteBuilderSessionRecord,
  related: Readonly<{
    artifactCount: number
    eventCount: number
    fileCount: number
    phases: ReadonlyArray<SiteBuilderPhaseRunRecord>
    previewCount: number
  }>,
): OperatorSiteBuilderSessionProjection => ({
  activeArtifactId: session.activeArtifactId,
  activePreviewId: session.activePreviewId,
  artifactCount: related.artifactCount,
  createdAt: session.createdAt,
  customerUserId: session.customerUserId,
  eventCount: related.eventCount,
  fileCount: related.fileCount,
  hasMetadata: Object.keys(session.metadata).length > 0,
  id: session.id,
  orderId: session.orderId,
  ownerUserId: session.ownerUserId,
  phaseCount: related.phases.length,
  phaseCurrent: currentPhaseProjection(related.phases),
  previewCount: related.previewCount,
  promptSummary: session.promptSummary,
  siteId: session.siteId,
  status: session.status,
  updatedAt: session.updatedAt,
  workroomId: session.workroomId,
})

export const createSiteBuilderSession = (
  db: D1Database,
  input: CreateSiteBuilderSessionInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderSessionRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateCreateSession(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_sessions',
      input.idempotencyKey,
      sessionFromRow,
      'createSiteBuilderSession.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_session')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_sessions (
               id,
               idempotency_key,
               site_id,
               order_id,
               workroom_id,
               owner_user_id,
               customer_user_id,
               created_by_actor_ref,
               status,
               prompt_summary,
               source_site_version_id,
               source_revision_id,
               active_preview_id,
               active_artifact_id,
               metadata_json,
               created_at,
               updated_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.siteId ?? null,
            input.orderId ?? null,
            input.workroomId ?? null,
            input.ownerUserId,
            input.customerUserId ?? null,
            input.createdByActorRef,
            valid.status,
            valid.promptSummary,
            input.sourceSiteVersionId ?? null,
            input.sourceRevisionId ?? null,
            JSON.stringify(valid.metadata),
            now,
            now,
          )
          .run(),
      catch: error => storageError('createSiteBuilderSession.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_sessions',
      input.idempotencyKey,
      sessionFromRow,
      'createSiteBuilderSession.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'createSiteBuilderSession.readInserted',
        reason: 'site builder session was not readable after insert.',
      })
    }

    return record
  })

export const appendSiteBuilderMessage = (
  db: D1Database,
  input: AppendSiteBuilderMessageInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderMessageRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateMessage(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_messages',
      input.idempotencyKey,
      messageFromRow,
      'appendSiteBuilderMessage.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const sequence =
      input.sequence ??
      (yield* nextSequence(db, 'site_builder_messages', input.sessionId))
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_message')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_messages (
               id,
               idempotency_key,
               session_id,
               sequence,
               actor_kind,
               visibility,
               body,
               metadata_json,
               created_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            sequence,
            input.actorKind,
            valid.visibility,
            valid.body,
            JSON.stringify(valid.metadata),
            now,
          )
          .run(),
      catch: error => storageError('appendSiteBuilderMessage.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_messages',
      input.idempotencyKey,
      messageFromRow,
      'appendSiteBuilderMessage.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'appendSiteBuilderMessage.readInserted',
        reason: 'site builder message was not readable after insert.',
      })
    }

    return record
  })

export const appendSiteBuilderEvent = (
  db: D1Database,
  input: AppendSiteBuilderEventInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderEventRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateEvent(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_events',
      input.idempotencyKey,
      eventFromRow,
      'appendSiteBuilderEvent.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const sequence =
      input.sequence ??
      (yield* nextSequence(db, 'site_builder_events', input.sessionId))
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_event')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_events (
               id,
               idempotency_key,
               session_id,
               sequence,
               event_kind,
               phase_kind,
               visibility,
               status,
               title,
               summary,
               source_ref,
               payload_json,
               created_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            sequence,
            input.eventKind,
            input.phaseKind ?? null,
            valid.visibility,
            valid.status,
            valid.title,
            valid.summary,
            input.sourceRef ?? null,
            JSON.stringify(valid.payload),
            now,
          )
          .run(),
      catch: error => storageError('appendSiteBuilderEvent.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_events',
      input.idempotencyKey,
      eventFromRow,
      'appendSiteBuilderEvent.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'appendSiteBuilderEvent.readInserted',
        reason: 'site builder event was not readable after insert.',
      })
    }

    return record
  })

const phaseEventKindForStatus = (
  status: SiteBuilderPhaseStatus,
): SiteBuilderEventKind => {
  if (status === 'running') {
    return 'phase_started'
  }

  if (status === 'succeeded' || status === 'skipped') {
    return 'phase_completed'
  }

  return 'phase_updated'
}

export const recordSiteBuilderPhaseRun = (
  db: D1Database,
  input: RecordSiteBuilderPhaseRunInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderPhaseRunRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validatePhaseRun(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_phase_runs',
      input.idempotencyKey,
      phaseFromRow,
      'recordSiteBuilderPhaseRun.readByIdempotencyKey',
    )

    if (existing !== null) {
      yield* appendSiteBuilderEvent(
        db,
        {
          eventKind: phaseEventKindForStatus(existing.status),
          idempotencyKey: `${input.idempotencyKey}:event`,
          payload: {
            phaseRunId: existing.id,
            sequence: existing.sequence,
          },
          phaseKind: existing.phaseKind,
          sessionId: input.sessionId,
          status: existing.status,
          summary: existing.summary,
          title: existing.title,
          visibility: input.visibility ?? 'customer',
        },
        runtime,
      )

      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const sequence =
      valid.sequence ??
      (yield* nextSequence(db, 'site_builder_phase_runs', input.sessionId))
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_phase')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_phase_runs (
               id,
               idempotency_key,
               session_id,
               sequence,
               phase_kind,
               status,
               title,
               summary,
               started_at,
               completed_at,
               metadata_json,
               created_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            sequence,
            input.phaseKind,
            input.status,
            valid.title,
            valid.summary,
            valid.startedAt,
            valid.completedAt,
            JSON.stringify(valid.metadata),
            now,
          )
          .run(),
      catch: error => storageError('recordSiteBuilderPhaseRun.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_phase_runs',
      input.idempotencyKey,
      phaseFromRow,
      'recordSiteBuilderPhaseRun.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'recordSiteBuilderPhaseRun.readInserted',
        reason: 'site builder phase run was not readable after insert.',
      })
    }

    yield* appendSiteBuilderEvent(
      db,
      {
        eventKind: phaseEventKindForStatus(record.status),
        idempotencyKey: `${input.idempotencyKey}:event`,
        payload: {
          phaseRunId: record.id,
          sequence: record.sequence,
        },
        phaseKind: record.phaseKind,
        sessionId: record.sessionId,
        status: record.status,
        summary: record.summary,
        title: record.title,
        visibility: valid.visibility,
      },
      runtime,
    )

    return record
  })

export const listSiteBuilderEventsAfter = (
  db: D1Database,
  input: ListSiteBuilderEventsAfterInput,
): Effect.Effect<
  ReadonlyArray<SiteBuilderEventRecord>,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    yield* validationEffect(() => {
      assertSafeRef('sessionId', input.sessionId)

      if (
        input.cursor !== undefined &&
        (!Number.isSafeInteger(input.cursor) || input.cursor < 0)
      ) {
        throw new SiteBuilderSessionValidationError({
          reason: 'cursor must be a non-negative safe integer.',
        })
      }

      if (
        input.limit !== undefined &&
        (!Number.isSafeInteger(input.limit) || input.limit < 1)
      ) {
        throw new SiteBuilderSessionValidationError({
          reason: 'limit must be a positive safe integer.',
        })
      }
    })
    yield* assertSessionExists(db, input.sessionId)
    const cursor = input.cursor ?? 0
    const limit = Math.min(input.limit ?? 100, 500)

    return yield* Effect.tryPromise({
      try: async () => {
        const rows = await db
          .prepare(
            `SELECT *
               FROM site_builder_events
              WHERE session_id = ?
                AND sequence > ?
                AND archived_at IS NULL
              ORDER BY sequence ASC
              LIMIT ?`,
          )
          .bind(input.sessionId, cursor, limit)
          .all<SiteBuilderEventRow>()

        return (rows.results ?? []).map(eventFromRow)
      },
      catch: error => storageError('listSiteBuilderEventsAfter', error),
    })
  })

export const upsertSiteBuilderFileSnapshot = (
  db: D1Database,
  input: UpsertSiteBuilderFileSnapshotInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderFileSnapshotRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateFile(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_file_snapshots',
      input.idempotencyKey,
      fileFromRow,
      'upsertSiteBuilderFileSnapshot.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const sequence =
      input.sequence ??
      (yield* nextSequence(db, 'site_builder_file_snapshots', input.sessionId))
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_file')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_file_snapshots (
               id,
               idempotency_key,
               session_id,
               path,
               sequence,
               language,
               content_hash,
               byte_size,
               source_ref,
               artifact_ref,
               preview_text,
               visibility,
               metadata_json,
               created_at,
               updated_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            valid.path,
            sequence,
            input.language ?? null,
            input.contentHash,
            input.byteSize,
            input.sourceRef ?? null,
            input.artifactRef ?? null,
            valid.previewText,
            valid.visibility,
            JSON.stringify(valid.metadata),
            now,
            now,
          )
          .run(),
      catch: error =>
        storageError('upsertSiteBuilderFileSnapshot.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_file_snapshots',
      input.idempotencyKey,
      fileFromRow,
      'upsertSiteBuilderFileSnapshot.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'upsertSiteBuilderFileSnapshot.readInserted',
        reason: 'site builder file snapshot was not readable after insert.',
      })
    }

    return record
  })

export const listSiteBuilderFileSnapshots = (
  db: D1Database,
  input: ListSiteBuilderFileSnapshotsInput,
): Effect.Effect<
  ReadonlyArray<SiteBuilderFileSnapshotRecord>,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    yield* validationEffect(() => {
      assertSafeRef('sessionId', input.sessionId)

      if (
        input.limit !== undefined &&
        (!Number.isSafeInteger(input.limit) || input.limit < 1)
      ) {
        throw new SiteBuilderSessionValidationError({
          reason: 'limit must be a positive safe integer.',
        })
      }
    })
    yield* assertSessionExists(db, input.sessionId)
    const limit = Math.min(input.limit ?? 500, 1000)

    return yield* Effect.tryPromise({
      try: async () => {
        const rows = await db
          .prepare(
            `SELECT *
               FROM site_builder_file_snapshots
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY path ASC, sequence DESC
              LIMIT ?`,
          )
          .bind(input.sessionId, limit)
          .all<SiteBuilderFileSnapshotRow>()

        return (rows.results ?? []).map(fileFromRow)
      },
      catch: error => storageError('listSiteBuilderFileSnapshots', error),
    })
  })

export const readLatestSiteBuilderFileSnapshot = (
  db: D1Database,
  input: ReadSiteBuilderFileSnapshotInput,
): Effect.Effect<
  SiteBuilderFileSnapshotRecord | null,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const path = yield* validationEffect(() => {
      assertSafeRef('sessionId', input.sessionId)

      return assertSafePath(input.path)
    })
    yield* assertSessionExists(db, input.sessionId)

    return yield* Effect.tryPromise({
      try: async () => {
        const row = await db
          .prepare(
            `SELECT *
               FROM site_builder_file_snapshots
              WHERE session_id = ?
                AND path = ?
                AND archived_at IS NULL
              ORDER BY sequence DESC
              LIMIT 1`,
          )
          .bind(input.sessionId, path)
          .first<SiteBuilderFileSnapshotRow>()

        return row === null ? null : fileFromRow(row)
      },
      catch: error => storageError('readLatestSiteBuilderFileSnapshot', error),
    })
  })

export const recordSiteBuilderPreview = (
  db: D1Database,
  input: RecordSiteBuilderPreviewInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderPreviewRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validatePreview(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_previews',
      input.idempotencyKey,
      previewFromRow,
      'recordSiteBuilderPreview.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_preview')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_previews (
               id,
               idempotency_key,
               session_id,
               preview_kind,
               status,
               preview_url,
               version_ref,
               artifact_ref,
               health_ref,
               metadata_json,
               created_at,
               updated_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            input.previewKind,
            valid.status,
            input.previewUrl ?? null,
            input.versionRef ?? null,
            input.artifactRef ?? null,
            input.healthRef ?? null,
            JSON.stringify(valid.metadata),
            now,
            now,
          )
          .run(),
      catch: error => storageError('recordSiteBuilderPreview.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_previews',
      input.idempotencyKey,
      previewFromRow,
      'recordSiteBuilderPreview.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'recordSiteBuilderPreview.readInserted',
        reason: 'site builder preview was not readable after insert.',
      })
    }

    return record
  })

export const recordSiteBuilderArtifact = (
  db: D1Database,
  input: RecordSiteBuilderArtifactInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderArtifactRecord,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const valid = yield* validationEffect(() => validateArtifact(input))
    const existing = yield* readByIdempotencyKey(
      db,
      'site_builder_artifacts',
      input.idempotencyKey,
      artifactFromRow,
      'recordSiteBuilderArtifact.readByIdempotencyKey',
    )

    if (existing !== null) {
      return existing
    }

    yield* assertSessionExists(db, input.sessionId)
    const now = runtime.nowIso()
    const id = input.id ?? runtime.randomId('site_builder_artifact')

    yield* Effect.tryPromise({
      try: () =>
        db
          .prepare(
            `INSERT OR IGNORE INTO site_builder_artifacts (
               id,
               idempotency_key,
               session_id,
               artifact_kind,
               artifact_ref,
               content_hash,
               byte_size,
               manifest_ref,
               metadata_json,
               created_at,
               archived_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL)`,
          )
          .bind(
            id,
            input.idempotencyKey,
            input.sessionId,
            input.artifactKind,
            input.artifactRef,
            input.contentHash ?? null,
            input.byteSize ?? null,
            input.manifestRef ?? null,
            JSON.stringify(valid.metadata),
            now,
          )
          .run(),
      catch: error => storageError('recordSiteBuilderArtifact.insert', error),
    })

    const record = yield* readByIdempotencyKey(
      db,
      'site_builder_artifacts',
      input.idempotencyKey,
      artifactFromRow,
      'recordSiteBuilderArtifact.readInserted',
    )

    if (record === null) {
      return yield* new SiteBuilderSessionStorageError({
        operation: 'recordSiteBuilderArtifact.readInserted',
        reason: 'site builder artifact was not readable after insert.',
      })
    }

    return record
  })

export const readSiteBuilderSessionProjection = (
  db: D1Database,
  sessionId: string,
): Effect.Effect<
  Readonly<{
    operator: OperatorSiteBuilderSessionProjection
    public: PublicSiteBuilderSessionProjection
  }>,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    yield* validationEffect(() => assertSafeRef('sessionId', sessionId))
    const session = yield* assertSessionExists(db, sessionId)
    const related = yield* Effect.tryPromise({
      try: async () => {
        const messageRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_messages
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY sequence ASC
              LIMIT 200`,
          )
          .bind(sessionId)
          .all<SiteBuilderMessageRow>()
        const eventRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_events
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY sequence ASC
              LIMIT 500`,
          )
          .bind(sessionId)
          .all<SiteBuilderEventRow>()
        const phaseRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_phase_runs
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY sequence ASC
              LIMIT 100`,
          )
          .bind(sessionId)
          .all<SiteBuilderPhaseRunRow>()
        const fileRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_file_snapshots
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY path ASC, sequence DESC
              LIMIT 500`,
          )
          .bind(sessionId)
          .all<SiteBuilderFileSnapshotRow>()
        const previewRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_previews
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 200`,
          )
          .bind(sessionId)
          .all<SiteBuilderPreviewRow>()
        const artifactRows = await db
          .prepare(
            `SELECT *
               FROM site_builder_artifacts
              WHERE session_id = ?
                AND archived_at IS NULL
              ORDER BY created_at DESC
              LIMIT 200`,
          )
          .bind(sessionId)
          .all<SiteBuilderArtifactRow>()

        return {
          artifactCount: artifactRows.results?.length ?? 0,
          eventCount: eventRows.results?.length ?? 0,
          fileCount: fileRows.results?.length ?? 0,
          messages: (messageRows.results ?? []).map(messageFromRow),
          phases: (phaseRows.results ?? []).map(phaseFromRow),
          previews: (previewRows.results ?? []).map(previewFromRow),
          previewCount: previewRows.results?.length ?? 0,
        }
      },
      catch: error =>
        storageError('readSiteBuilderSessionProjection.related', error),
    })

    return {
      operator: operatorSiteBuilderSessionProjection(session, related),
      public: publicSiteBuilderSessionProjection(
        session,
        related.messages,
        related.phases,
        related.previews,
      ),
    }
  })
