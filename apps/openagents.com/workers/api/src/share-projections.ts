import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import {
  ShareAudience,
  type ShareAudienceRecipient as ShareAudienceRecipientType,
  type ShareAudience as ShareAudienceType,
  ShareCreateRequest,
  type ShareCreateRequest as ShareCreateRequestType,
  ShareProjectionV1,
  type ShareProjectionV1 as ShareProjectionV1Type,
  type ShareSource as ShareSourceType,
  ShareUpdateRequest,
  type ShareUpdateRequest as ShareUpdateRequestType,
  type WorkroomFileItem,
  type WorkroomTimelineMessage,
  type WorkroomTimelineToolPart,
} from '@openagentsinc/sync-schema'
import { Context, Effect, Layer, Match as M, Schema as S } from 'effect'

import {
  arrayFromUnknown,
  nestedUnknown,
  optionalInteger,
  optionalNestedString,
  optionalString,
  parseJsonStringArray,
  parseJsonWithSchema,
  safeJsonRecord,
} from './json-boundary'
import {
  type AgentRunBundle,
  type AgentRunRecord,
  type OmniEventRecord,
} from './omni-runs'
import {
  currentEpochMillis,
  currentIsoTimestamp,
  randomUuid,
} from './runtime-primitives'
import type { TeamChatMessage } from './team-chat'
import { readActiveTeamMembershipRole } from './team-repository'

export const SHARE_PROJECTION_VERSION = 1
export const DEFAULT_SHARE_REDACTION_POLICY_ID = 'default'

type ShareRuntime = Readonly<{
  makeUuid: () => string
  nowIso: () => string
}>

const defaultShareRuntime: ShareRuntime = {
  makeUuid: randomUuid,
  nowIso: currentIsoTimestamp,
}

export class ShareProjectionNotFound extends S.TaggedErrorClass<ShareProjectionNotFound>()(
  'ShareProjectionNotFound',
  {
    shareId: S.String,
  },
) {}

export class ShareProjectionAuthenticationRequired extends S.TaggedErrorClass<ShareProjectionAuthenticationRequired>()(
  'ShareProjectionAuthenticationRequired',
  {
    shareId: S.String,
  },
) {}

export class ShareProjectionForbidden extends S.TaggedErrorClass<ShareProjectionForbidden>()(
  'ShareProjectionForbidden',
  {
    shareId: S.String,
  },
) {}

export class ShareProjectionMalformed extends S.TaggedErrorClass<ShareProjectionMalformed>()(
  'ShareProjectionMalformed',
  {
    reason: S.String,
  },
) {}

export class ShareProjectionStorageError extends S.TaggedErrorClass<ShareProjectionStorageError>()(
  'ShareProjectionStorageError',
  {
    error: S.Defect,
    operation: S.String,
  },
) {}

export class ShareProjectionUnsafe extends S.TaggedErrorClass<ShareProjectionUnsafe>()(
  'ShareProjectionUnsafe',
  {
    shareId: S.String,
  },
) {}

export const ShareProjectionError = S.Union([
  ShareProjectionNotFound,
  ShareProjectionAuthenticationRequired,
  ShareProjectionForbidden,
  ShareProjectionMalformed,
  ShareProjectionStorageError,
  ShareProjectionUnsafe,
])
export type ShareProjectionError = typeof ShareProjectionError.Type

export type ShareViewer = Readonly<{
  email: string
  isAdmin?: boolean
  name: string
  userId: string
}>

export type ShareProjectionRecord = Readonly<{
  audience: ShareAudienceType
  canonicalUrl: string
  createdAt: string
  expiresAt: string | null
  id: string
  ownerUserId: string
  projectId: string | null
  projection: ShareProjectionV1Type
  redactionPolicyId: string
  revokedAt: string | null
  source: ShareSourceType
  status: 'active' | 'revoked'
  summary: string | null
  teamId: string | null
  title: string
  updatedAt: string
}>

type ShareProjectionRow = Readonly<{
  audience_json: string
  canonical_url: string
  created_at: string
  expires_at: string | null
  id: string
  owner_user_id: string
  project_id: string | null
  projection_json: string
  redaction_policy_id: string
  revoked_at: string | null
  source_id: string
  source_kind: ShareSourceType['kind']
  status: 'active' | 'revoked'
  summary: string | null
  team_id: string | null
  title: string
  updated_at: string
}>

export type ShareProjectionCreateInput = Readonly<{
  audience: ShareAudienceType
  canonicalUrl: string
  ownerUserId: string
  projectId: string | null
  projection: ShareProjectionV1Type
  redactionPolicyId: string
  shareId: string
  source: ShareSourceType
  teamId: string | null
  title: string
  expiresAt?: string | null
}>

export type ShareProjectionRepositoryShape = Readonly<{
  create: (
    input: ShareProjectionCreateInput,
  ) => Effect.Effect<ShareProjectionRecord, ShareProjectionStorageError>
  readById: (
    shareId: string,
  ) => Effect.Effect<
    ShareProjectionRecord | undefined,
    ShareProjectionStorageError
  >
  revoke: (
    shareId: string,
    nowIso: string,
  ) => Effect.Effect<
    ShareProjectionRecord | undefined,
    ShareProjectionStorageError
  >
  update: (
    shareId: string,
    input: Readonly<{
      audience?: ShareAudienceType
      expiresAt?: string | null
      projection: ShareProjectionV1Type
      title?: string
      updatedAt: string
    }>,
  ) => Effect.Effect<
    ShareProjectionRecord | undefined,
    ShareProjectionStorageError
  >
}>

export class ShareProjectionRepository extends Context.Service<
  ShareProjectionRepository,
  ShareProjectionRepositoryShape
>()('@openagentsinc/autopilot-omega/ShareProjectionRepository') {
  static readonly layer = (db: D1Database) =>
    Layer.succeed(
      ShareProjectionRepository,
      makeD1ShareProjectionRepository(db),
    )
}

export type ShareUrlServiceShape = Readonly<{
  canonicalUrlForShareId: (shareId: string) => string
}>

export class ShareUrlService extends Context.Service<
  ShareUrlService,
  ShareUrlServiceShape
>()('@openagentsinc/autopilot-omega/ShareUrlService') {
  static readonly layer = (origin: string) =>
    Layer.succeed(ShareUrlService, {
      canonicalUrlForShareId: (shareId: string) => `${origin}/share/${shareId}`,
    })
}

export type ShareReceiptServiceShape = Readonly<{
  createdRef: (shareId: string) => string
  audienceChangedRef: (shareId: string) => string
  revokedRef: (shareId: string) => string
}>

export class ShareReceiptService extends Context.Service<
  ShareReceiptService,
  ShareReceiptServiceShape
>()('@openagentsinc/autopilot-omega/ShareReceiptService') {
  static readonly layer = Layer.succeed(ShareReceiptService, {
    audienceChangedRef: shareId => `receipt_share_audience_changed_${shareId}`,
    createdRef: shareId => `receipt_share_created_${shareId}`,
    revokedRef: shareId => `receipt_share_revoked_${shareId}`,
  })
}

export type ShareAccessServiceShape = Readonly<{
  authorizeCreate: (
    input: Readonly<{
      audience: ShareAudienceType
      db: D1Database
      sourceTeamId: string | null
      viewer: ShareViewer
    }>,
  ) => Effect.Effect<void, ShareProjectionForbidden>
  authorizeManage: (
    input: Readonly<{
      db: D1Database
      record: ShareProjectionRecord
      viewer: ShareViewer
    }>,
  ) => Effect.Effect<void, ShareProjectionForbidden>
  authorizeView: (
    input: Readonly<{
      db: D1Database
      record: ShareProjectionRecord
      viewer?: ShareViewer
    }>,
  ) => Effect.Effect<
    ShareProjectionV1Type,
    ShareProjectionAuthenticationRequired | ShareProjectionForbidden
  >
}>

export class ShareAccessService extends Context.Service<
  ShareAccessService,
  ShareAccessServiceShape
>()('@openagentsinc/autopilot-omega/ShareAccessService') {
  static readonly layer = Layer.succeed(ShareAccessService, {
    authorizeCreate: input => authorizeCreate(input),
    authorizeManage: input => authorizeManage(input),
    authorizeView: input => authorizeView(input),
  })
}

type ShareProjectionBuilderInput =
  | Readonly<{
      _tag: 'AgentRun'
      audience: ShareAudienceType
      bundle: AgentRunBundle
      canonicalUrl: string
      createdAt: string
      expiresAt?: string | null
      receiptRefs: ReadonlyArray<string>
      shareId: string
      title?: string
    }>
  | Readonly<{
      _tag: 'TeamThread'
      audience: ShareAudienceType
      canonicalUrl: string
      createdAt: string
      messages: ReadonlyArray<TeamChatMessage>
      projectId: string | null
      receiptRefs: ReadonlyArray<string>
      shareId: string
      teamId: string
      title?: string
    }>

export type ShareProjectionBuilderShape = Readonly<{
  build: (
    input: ShareProjectionBuilderInput,
  ) => Effect.Effect<ShareProjectionV1Type, ShareProjectionUnsafe>
}>

export class ShareProjectionBuilder extends Context.Service<
  ShareProjectionBuilder,
  ShareProjectionBuilderShape
>()('@openagentsinc/autopilot-omega/ShareProjectionBuilder') {
  static readonly layer = Layer.succeed(ShareProjectionBuilder, {
    build: input =>
      Effect.succeed(
        input._tag === 'AgentRun'
          ? projectionFromAgentRun(input)
          : projectionFromTeamThread(input),
      ).pipe(
        Effect.flatMap(projection =>
          containsProviderSecretMaterial(JSON.stringify(projection))
            ? Effect.fail(new ShareProjectionUnsafe({ shareId: input.shareId }))
            : Effect.succeed(projection),
        ),
      ),
  })
}

export const decodeShareCreateRequest = (
  value: unknown,
): Effect.Effect<ShareCreateRequestType, ShareProjectionMalformed> =>
  S.decodeUnknownEffect(ShareCreateRequest)(value).pipe(
    Effect.mapError(
      error =>
        new ShareProjectionMalformed({
          reason: String(error),
        }),
    ),
  )

export const decodeShareUpdateRequest = (
  value: unknown,
): Effect.Effect<ShareUpdateRequestType, ShareProjectionMalformed> =>
  S.decodeUnknownEffect(ShareUpdateRequest)(value).pipe(
    Effect.mapError(
      error =>
        new ShareProjectionMalformed({
          reason: String(error),
        }),
    ),
  )

const shareRowToSource = (row: ShareProjectionRow): ShareSourceType => {
  if (row.source_kind === 'team-thread') {
    return {
      kind: 'team-thread',
      id: row.source_id,
      ...(row.team_id === null ? {} : { teamId: row.team_id }),
    }
  }

  if (row.source_kind === 'team-project-thread') {
    return {
      kind: 'team-project-thread',
      id: row.source_id,
      teamId: row.team_id ?? '',
      ...(row.project_id === null ? {} : { projectId: row.project_id }),
    }
  }

  return { kind: 'agent-run', id: row.source_id }
}

const shareProjectionRecordFromRow = (
  row: ShareProjectionRow,
): ShareProjectionRecord => {
  const projection = parseJsonWithSchema(ShareProjectionV1, row.projection_json)
  const audience = parseJsonWithSchema(ShareAudience, row.audience_json)

  return {
    audience,
    canonicalUrl: row.canonical_url,
    createdAt: row.created_at,
    expiresAt: row.expires_at,
    id: row.id,
    ownerUserId: row.owner_user_id,
    projectId: row.project_id,
    projection,
    redactionPolicyId: row.redaction_policy_id,
    revokedAt: row.revoked_at,
    source: shareRowToSource(row),
    status: row.status,
    summary: row.summary,
    teamId: row.team_id,
    title: row.title,
    updatedAt: row.updated_at,
  }
}

const sourceStorage = (
  source: ShareSourceType,
): Readonly<{
  projectId: string | null
  sourceId: string
  sourceKind: ShareSourceType['kind']
  teamId: string | null
}> =>
  source.kind === 'agent-run'
    ? {
        projectId: null,
        sourceId: source.id,
        sourceKind: source.kind,
        teamId: null,
      }
    : source.kind === 'team-thread'
      ? {
          projectId: null,
          sourceId: source.id,
          sourceKind: source.kind,
          teamId: source.teamId ?? source.id,
        }
      : {
          projectId: source.projectId ?? source.id,
          sourceId: source.projectId ?? source.id,
          sourceKind: source.kind,
          teamId: source.teamId,
        }

const recipientStorageRows = (
  shareId: string,
  audience: ShareAudienceType,
  createdAt: string,
): ReadonlyArray<
  Readonly<{
    displayName: string
    subjectId: string
    subjectKind: 'email' | 'team' | 'user'
  }>
> => {
  if (shareId.trim() === '' || createdAt.trim() === '') {
    return []
  }

  if (audience._tag === 'TeamMembers') {
    return [
      {
        displayName: audience.teamName,
        subjectId: audience.teamId,
        subjectKind: 'team',
      },
    ]
  }

  if (audience._tag !== 'Users') {
    return []
  }

  return audience.recipients.flatMap(recipient => {
    const userRow =
      recipient.userId === null
        ? []
        : [
            {
              displayName: recipient.displayName,
              subjectId: recipient.userId,
              subjectKind: 'user' as const,
            },
          ]
    const emailRow =
      recipient.email === null
        ? []
        : [
            {
              displayName: recipient.displayName,
              subjectId: recipient.email.toLowerCase(),
              subjectKind: 'email' as const,
            },
          ]

    return [...userRow, ...emailRow]
  })
}

const replaceRecipients = async (
  db: D1Database,
  shareId: string,
  audience: ShareAudienceType,
  createdAt: string,
): Promise<void> => {
  const rows = recipientStorageRows(shareId, audience, createdAt)

  await db
    .prepare('DELETE FROM share_projection_recipients WHERE share_id = ?')
    .bind(shareId)
    .run()

  if (rows.length === 0) {
    return
  }

  await db.batch(
    rows.map(row =>
      db
        .prepare(
          `INSERT INTO share_projection_recipients
            (share_id, subject_kind, subject_id, display_name, created_at)
           VALUES (?, ?, ?, ?, ?)`,
        )
        .bind(
          shareId,
          row.subjectKind,
          row.subjectId,
          row.displayName,
          createdAt,
        ),
    ),
  )
}

const readShareProjectionRecordById = async (
  db: D1Database,
  shareId: string,
): Promise<ShareProjectionRecord | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         id,
         canonical_url,
         source_kind,
         source_id,
         owner_user_id,
         team_id,
         project_id,
         audience_json,
         title,
         summary,
         status,
         projection_json,
         redaction_policy_id,
         created_at,
         updated_at,
         revoked_at,
         expires_at
       FROM share_projections
       WHERE id = ?
       LIMIT 1`,
    )
    .bind(shareId)
    .first<ShareProjectionRow>()

  return row === null ? undefined : shareProjectionRecordFromRow(row)
}

export const makeD1ShareProjectionRepository = (
  db: D1Database,
): ShareProjectionRepositoryShape => {
  const readById = Effect.fn('ShareProjectionRepository.readById')(
    (shareId: string) =>
      Effect.tryPromise({
        try: () => readShareProjectionRecordById(db, shareId),
        catch: error =>
          new ShareProjectionStorageError({
            error,
            operation: 'share_projection.read',
          }),
      }),
  )

  return {
    create: Effect.fn('ShareProjectionRepository.create')(input =>
      Effect.tryPromise({
        try: async () => {
          const storage = sourceStorage(input.source)
          const now = input.projection.createdAt

          await db
            .prepare(
              `INSERT INTO share_projections
                (
                  id,
                  canonical_url,
                  source_kind,
                  source_id,
                  owner_user_id,
                  team_id,
                  project_id,
                  audience_json,
                  title,
                  summary,
                  status,
                  projection_version,
                  projection_json,
                  projection_object_key,
                  redaction_policy_id,
                  created_at,
                  updated_at,
                  revoked_at,
                  expires_at
                )
               VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, ?, NULL, ?, ?, ?, NULL, ?)`,
            )
            .bind(
              input.shareId,
              input.canonicalUrl,
              storage.sourceKind,
              storage.sourceId,
              input.ownerUserId,
              input.teamId,
              input.projectId,
              JSON.stringify(input.audience),
              input.title,
              input.projection.subtitle,
              SHARE_PROJECTION_VERSION,
              JSON.stringify(input.projection),
              input.redactionPolicyId,
              now,
              now,
              input.expiresAt ?? null,
            )
            .run()
          await replaceRecipients(db, input.shareId, input.audience, now)

          return await readShareProjectionRecordById(db, input.shareId)
        },
        catch: error =>
          new ShareProjectionStorageError({
            error,
            operation: 'share_projection.create',
          }),
      }).pipe(
        Effect.flatMap(record =>
          record === undefined
            ? Effect.fail(
                new ShareProjectionStorageError({
                  error: 'Created share projection could not be loaded.',
                  operation: 'share_projection.create_load',
                }),
              )
            : Effect.succeed(record),
        ),
      ),
    ),
    readById,
    revoke: Effect.fn('ShareProjectionRepository.revoke')((shareId, nowIso) =>
      Effect.tryPromise({
        try: async () => {
          const current = await readShareProjectionRecordById(db, shareId)

          if (current === undefined) {
            return undefined
          }

          const projection = {
            ...current.projection,
            status: 'revoked' as const,
            updatedAt: nowIso,
            receipts: [
              ...current.projection.receipts,
              `receipt_share_revoked_${shareId}`,
            ],
          }

          await db
            .prepare(
              `UPDATE share_projections
               SET status = 'revoked',
                   projection_json = ?,
                   updated_at = ?,
                   revoked_at = ?
               WHERE id = ?`,
            )
            .bind(JSON.stringify(projection), nowIso, nowIso, shareId)
            .run()

          return await readShareProjectionRecordById(db, shareId)
        },
        catch: error =>
          new ShareProjectionStorageError({
            error,
            operation: 'share_projection.revoke',
          }),
      }),
    ),
    update: Effect.fn('ShareProjectionRepository.update')((shareId, input) =>
      Effect.tryPromise({
        try: async () => {
          const current = await readShareProjectionRecordById(db, shareId)

          if (current === undefined) {
            return undefined
          }

          await db
            .prepare(
              `UPDATE share_projections
               SET audience_json = ?,
                   title = ?,
                   projection_json = ?,
                   updated_at = ?,
                   expires_at = ?
               WHERE id = ?`,
            )
            .bind(
              JSON.stringify(input.audience ?? current.audience),
              input.title ?? current.title,
              JSON.stringify(input.projection),
              input.updatedAt,
              Object.hasOwn(input, 'expiresAt')
                ? input.expiresAt
                : current.expiresAt,
              shareId,
            )
            .run()
          await replaceRecipients(
            db,
            shareId,
            input.audience ?? current.audience,
            input.updatedAt,
          )

          return await readShareProjectionRecordById(db, shareId)
        },
        catch: error =>
          new ShareProjectionStorageError({
            error,
            operation: 'share_projection.update',
          }),
      }),
    ),
  }
}

const compactLine = (value: string, max = 260): string => {
  const line = value.replace(/\s+/g, ' ').trim()

  return line.length <= max ? line : `${line.slice(0, max - 1)}...`
}

const safePublicText = (
  value: string | null | undefined,
  fallback = 'Activity recorded.',
  max = 320,
): string => {
  const text = value?.trim()

  if (text === undefined || text === '') {
    return fallback
  }

  return containsProviderSecretMaterial(text)
    ? '[redacted]'
    : compactLine(text, max)
}

const rawEventPayloadRecord = (
  event: Pick<OmniEventRecord, 'payloadJson'>,
): Record<string, unknown> | undefined => {
  const payload = safeJsonRecord(event.payloadJson)
  const dataJson = optionalString(payload?.dataJson)
  const rawPayloadJson =
    optionalString(payload?.rawPayloadJson) ??
    optionalString(payload?.raw_payload_json)

  return safeJsonRecord(dataJson) ?? safeJsonRecord(rawPayloadJson) ?? payload
}

const eventLooksLikeToolCall = (
  event: Pick<OmniEventRecord, 'payloadJson' | 'type'>,
): boolean => {
  const raw = rawEventPayloadRecord(event)
  const rawType = optionalString(raw?.type)
  const part = raw?.part

  return (
    event.type.includes('tool') ||
    rawType === 'tool_use' ||
    rawType === 'tool_result' ||
    optionalString(raw?.tool) !== undefined ||
    (part !== undefined && JSON.stringify(part).includes('tool'))
  )
}

const eventTokenTotal = (
  event: Pick<OmniEventRecord, 'payloadJson'>,
): number => {
  const raw = rawEventPayloadRecord(event)
  const total =
    optionalInteger(nestedUnknown(raw, ['usage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['tokenUsage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['token_usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['total_tokens']))

  if (total !== undefined) {
    return total
  }

  const input =
    optionalInteger(nestedUnknown(raw, ['usage', 'inputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'input_tokens'])) ??
    0
  const output =
    optionalInteger(nestedUnknown(raw, ['usage', 'outputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'output_tokens'])) ??
    0
  const reasoning =
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoningTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoning_tokens'])) ??
    0

  return input + output + reasoning
}

const eventStatus = (
  event: Pick<OmniEventRecord, 'status' | 'type'>,
): WorkroomTimelineToolPart['status'] => {
  if (event.status === 'failed' || event.type.includes('failed')) {
    return 'failed'
  }

  if (
    event.status === 'completed' ||
    event.status === 'complete' ||
    event.type.includes('completed')
  ) {
    return 'completed'
  }

  if (event.status === 'running' || event.status === 'waiting_for_input') {
    return 'running'
  }

  return 'queued'
}

const eventDetail = (event: OmniEventRecord): ReadonlyArray<string> => {
  const raw = rawEventPayloadRecord(event)
  const detail =
    optionalNestedString(raw, [
      ['detail'],
      ['message'],
      ['text'],
      ['output'],
      ['error', 'message'],
      ['properties', 'part', 'text'],
      ['part', 'text'],
    ]) ?? event.summary
  const safe = safePublicText(detail, event.type, 1_000)

  return safe === '' ? [] : [safe]
}

const artifactsFromEvents = (
  events: ReadonlyArray<OmniEventRecord>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      events.flatMap(event => [
        ...event.artifactRefs,
        ...parseJsonStringArray(event.payloadJson).filter(ref =>
          ref.startsWith('artifact_'),
        ),
      ]),
    ),
  ].filter(ref => !containsProviderSecretMaterial(ref))

const filesFromEvents = (
  events: ReadonlyArray<OmniEventRecord>,
): ReadonlyArray<WorkroomFileItem> => {
  const fileNames = events.flatMap(event => {
    const raw = rawEventPayloadRecord(event)
    const direct =
      optionalString(raw?.filename) ??
      optionalString(raw?.path) ??
      optionalString(raw?.file)
    const files = arrayFromUnknown(raw?.files)?.flatMap(item => {
      const text = optionalString(item)

      if (text !== undefined) {
        return [text]
      }

      const record = safeJsonRecord(JSON.stringify(item))

      return (optionalString(record?.path) ?? optionalString(record?.filename))
        ? [optionalString(record?.path) ?? optionalString(record?.filename)!]
        : []
    })

    return direct === undefined ? (files ?? []) : [direct, ...(files ?? [])]
  })

  return [...new Set(fileNames)]
    .filter(path => !containsProviderSecretMaterial(path))
    .slice(0, 32)
    .map(path => ({
      label: path,
      meta: 'file',
    }))
}

const repositorySubtitle = (run: AgentRunRecord): string =>
  `${run.repository.owner}/${run.repository.repo}@${run.repository.ref} · ${run.status}`

const eventMessages = (
  events: ReadonlyArray<OmniEventRecord>,
): ReadonlyArray<WorkroomTimelineMessage> =>
  [...events]
    .sort((left, right) =>
      left.sequence === right.sequence
        ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
        : left.sequence - right.sequence,
    )
    .slice(0, 120)
    .map(event => ({
      id: event.id,
      author: 'system' as const,
      label: 'Autopilot',
      time: event.createdAt,
      status: 'complete' as const,
      parts: [
        {
          kind: 'tool' as const,
          title: safePublicText(event.summary, event.type, 120),
          subtitle: event.source,
          status: eventStatus(event),
          detail: eventDetail(event),
        },
      ],
    }))

const projectionFromAgentRun = (
  input: Extract<ShareProjectionBuilderInput, { _tag: 'AgentRun' }>,
): ShareProjectionV1Type => {
  const run = input.bundle.run
  const artifacts = artifactsFromEvents(input.bundle.events)
  const files = filesFromEvents(input.bundle.events)

  return {
    schemaVersion: 'openagents.share_projection.v1',
    id: input.shareId,
    url: input.canonicalUrl,
    audience: input.audience,
    audienceLabel: audienceLabel(input.audience),
    title: safePublicText(input.title ?? run.goal, 'Autopilot run', 160),
    subtitle: repositorySubtitle(run),
    source: { kind: 'agent-run', id: run.id },
    status: 'active',
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    messages: [
      {
        id: `${input.shareId}-goal`,
        author: 'user',
        label: 'Goal',
        time: run.createdAt,
        status: 'complete',
        parts: [
          {
            kind: 'text',
            body: [safePublicText(run.goal, 'Autopilot run goal', 2_000)],
          },
        ],
      },
      ...eventMessages(input.bundle.events),
    ],
    files,
    artifacts,
    approvals: [],
    receipts: input.receiptRefs,
    metrics: {
      eventCount: input.bundle.events.length,
      tokenTotal: input.bundle.events.reduce(
        (total, event) => total + eventTokenTotal(event),
        0,
      ),
      toolCallCount: input.bundle.events.filter(eventLooksLikeToolCall).length,
    },
  }
}

const teamMessageToTimelineMessage = (
  message: TeamChatMessage,
): WorkroomTimelineMessage => ({
  id: message.id,
  author: message.kind === 'system' ? 'system' : 'user',
  label: message.kind === 'system' ? 'Autopilot' : message.author.name,
  time: message.createdAt,
  status: 'complete',
  parts: [
    {
      kind: 'text',
      body: [safePublicText(message.body, 'Team message', 2_000)],
    },
    ...(message.runSummary === undefined
      ? []
      : [
          {
            kind: 'tool' as const,
            title: 'Autopilot run',
            subtitle: message.runSummary.repository,
            status:
              message.runSummary.status === 'completed'
                ? ('completed' as const)
                : message.runSummary.status === 'failed' ||
                    message.runSummary.status === 'canceled'
                  ? ('failed' as const)
                  : ('running' as const),
            detail: [
              `${message.runSummary.status} · ${message.runSummary.eventCount} events · ${message.runSummary.tokenTotal} tokens`,
            ],
            actionHref: `/t/${message.runSummary.runId}`,
            actionLabel: 'Open run',
          },
        ]),
  ],
  ...(message.author.avatarUrl === null
    ? {}
    : { avatarUrl: message.author.avatarUrl }),
})

const projectionFromTeamThread = (
  input: Extract<ShareProjectionBuilderInput, { _tag: 'TeamThread' }>,
): ShareProjectionV1Type => {
  const title = input.title ?? 'Team workroom thread'

  return {
    schemaVersion: 'openagents.share_projection.v1',
    id: input.shareId,
    url: input.canonicalUrl,
    audience: input.audience,
    audienceLabel: audienceLabel(input.audience),
    title: safePublicText(title, 'Team workroom thread', 160),
    subtitle:
      input.projectId === null
        ? `Team thread · ${input.teamId}`
        : `Project thread · ${input.projectId}`,
    source:
      input.projectId === null
        ? { kind: 'team-thread', id: input.teamId, teamId: input.teamId }
        : {
            kind: 'team-project-thread',
            id: input.projectId,
            teamId: input.teamId,
            projectId: input.projectId,
          },
    status: 'active',
    createdAt: input.createdAt,
    updatedAt: input.createdAt,
    messages: input.messages.map(teamMessageToTimelineMessage),
    files: [],
    artifacts: input.messages.flatMap(message =>
      message.runSummary === undefined
        ? []
        : [`run:${message.runSummary.runId}`],
    ),
    approvals: [],
    receipts: input.receiptRefs,
    metrics: {
      eventCount: input.messages.reduce(
        (total, message) => total + (message.runSummary?.eventCount ?? 0),
        0,
      ),
      tokenTotal: input.messages.reduce(
        (total, message) => total + (message.runSummary?.tokenTotal ?? 0),
        0,
      ),
      toolCallCount: input.messages.reduce(
        (total, message) => total + (message.runSummary?.toolCallCount ?? 0),
        0,
      ),
    },
  }
}

const recipientMatchesViewer = (
  recipient: ShareAudienceRecipientType,
  viewer: ShareViewer,
): boolean =>
  (recipient.userId !== null && recipient.userId === viewer.userId) ||
  (recipient.email !== null &&
    recipient.email.toLowerCase() === viewer.email.toLowerCase())

export const audienceLabel = (
  audience: ShareAudienceType,
  viewer?: ShareViewer,
): string =>
  M.value(audience).pipe(
    M.tagsExhaustive({
      Public: () => 'Shared publicly',
      TeamMembers: audience =>
        audience.teamName.trim() === ''
          ? 'Shared with this team'
          : `Shared with members of ${audience.teamName}`,
      Users: audience => {
        if (
          viewer !== undefined &&
          audience.recipients.some(recipient =>
            recipientMatchesViewer(recipient, viewer),
          )
        ) {
          return 'Shared with you'
        }

        const [recipient] = audience.recipients

        return audience.recipients.length === 1 && recipient !== undefined
          ? `Shared with ${recipient.displayName}`
          : `Shared with ${audience.recipients.length} people`
      },
    }),
  )

const projectionForViewer = (
  record: ShareProjectionRecord,
  viewer?: ShareViewer,
): ShareProjectionV1Type => ({
  ...record.projection,
  audienceLabel: audienceLabel(record.audience, viewer),
  status:
    record.revokedAt !== null
      ? 'revoked'
      : record.expiresAt !== null &&
          Date.parse(record.expiresAt) <= currentEpochMillis()
        ? 'expired'
        : record.projection.status,
})

const requireActiveTeamMembership = (
  db: D1Database,
  teamId: string,
  userId: string,
): Effect.Effect<void, ShareProjectionForbidden> =>
  Effect.tryPromise({
    try: () => readActiveTeamMembershipRole(db, teamId, userId),
    catch: () => new ShareProjectionForbidden({ shareId: teamId }),
  }).pipe(
    Effect.flatMap(role =>
      role === undefined
        ? Effect.fail(new ShareProjectionForbidden({ shareId: teamId }))
        : Effect.void,
    ),
  )

const authorizeCreate = (
  input: Readonly<{
    audience: ShareAudienceType
    db: D1Database
    sourceTeamId: string | null
    viewer: ShareViewer
  }>,
): Effect.Effect<void, ShareProjectionForbidden> =>
  Effect.gen(function* () {
    if (input.sourceTeamId !== null) {
      yield* requireActiveTeamMembership(
        input.db,
        input.sourceTeamId,
        input.viewer.userId,
      )
    }

    if (input.audience._tag === 'TeamMembers') {
      yield* requireActiveTeamMembership(
        input.db,
        input.audience.teamId,
        input.viewer.userId,
      )
    }
  })

const authorizeManage = (
  input: Readonly<{
    db: D1Database
    record: ShareProjectionRecord
    viewer: ShareViewer
  }>,
): Effect.Effect<void, ShareProjectionForbidden> =>
  input.viewer.isAdmin === true ||
  input.record.ownerUserId === input.viewer.userId
    ? Effect.void
    : input.record.teamId === null
      ? Effect.fail(new ShareProjectionForbidden({ shareId: input.record.id }))
      : requireActiveTeamMembership(
          input.db,
          input.record.teamId,
          input.viewer.userId,
        ).pipe(
          Effect.mapError(
            () => new ShareProjectionForbidden({ shareId: input.record.id }),
          ),
        )

const authorizeView = (
  input: Readonly<{
    db: D1Database
    record: ShareProjectionRecord
    viewer?: ShareViewer
  }>,
): Effect.Effect<
  ShareProjectionV1Type,
  ShareProjectionAuthenticationRequired | ShareProjectionForbidden
> => {
  if (input.record.status === 'revoked' || input.record.revokedAt !== null) {
    return Effect.succeed(projectionForViewer(input.record, input.viewer))
  }

  if (
    input.record.expiresAt !== null &&
    Date.parse(input.record.expiresAt) <= currentEpochMillis()
  ) {
    return Effect.succeed(projectionForViewer(input.record, input.viewer))
  }

  if (input.record.audience._tag === 'Public') {
    return Effect.succeed(projectionForViewer(input.record, input.viewer))
  }

  if (input.viewer === undefined) {
    return Effect.fail(
      new ShareProjectionAuthenticationRequired({ shareId: input.record.id }),
    )
  }

  if (input.record.audience._tag === 'TeamMembers') {
    return requireActiveTeamMembership(
      input.db,
      input.record.audience.teamId,
      input.viewer.userId,
    ).pipe(
      Effect.as(projectionForViewer(input.record, input.viewer)),
      Effect.mapError(
        () => new ShareProjectionForbidden({ shareId: input.record.id }),
      ),
    )
  }

  const allowed =
    input.record.ownerUserId === input.viewer.userId ||
    input.viewer.isAdmin === true ||
    input.record.audience.recipients.some(recipient =>
      recipientMatchesViewer(recipient, input.viewer!),
    )

  return allowed
    ? Effect.succeed(projectionForViewer(input.record, input.viewer))
    : Effect.fail(new ShareProjectionForbidden({ shareId: input.record.id }))
}

export const makeShareId = (
  runtime: ShareRuntime = defaultShareRuntime,
): string => runtime.makeUuid()

export const sourceTeamId = (source: ShareSourceType): string | null =>
  source.kind === 'team-thread'
    ? (source.teamId ?? source.id)
    : source.kind === 'team-project-thread'
      ? source.teamId
      : null

export const sourceProjectId = (source: ShareSourceType): string | null =>
  source.kind === 'team-project-thread' ? (source.projectId ?? source.id) : null

export const sourceId = (source: ShareSourceType): string =>
  source.kind === 'team-project-thread'
    ? (source.projectId ?? source.id)
    : source.id

export const addReceiptRef = (
  projection: ShareProjectionV1Type,
  receiptRef: string,
  updatedAt: string,
): ShareProjectionV1Type => ({
  ...projection,
  updatedAt,
  receipts: [...projection.receipts, receiptRef],
})
