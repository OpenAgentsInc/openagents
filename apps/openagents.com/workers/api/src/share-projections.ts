import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import {
  SHARE_AGENT_PROMPT_LIMIT,
  SHARE_ARG_KEY_LIMIT,
  SHARE_ARG_LIMIT,
  SHARE_ARG_VALUE_LIMIT,
  SHARE_CHANGE_LIMIT,
  SHARE_COMMAND_LIMIT,
  SHARE_DIFF_LIMIT,
  SHARE_ERROR_MESSAGE_LIMIT,
  SHARE_NOTICE_TEXT_LIMIT,
  SHARE_OUTPUT_TAIL_LIMIT,
  SHARE_PATH_LIMIT,
  SHARE_PLAN_ENTRY_LIMIT,
  SHARE_PLAN_PROSE_LIMIT,
  SHARE_PLAN_STEP_LIMIT,
  SHARE_REASONING_SUMMARY_LIMIT,
  SHARE_RESULT_SNIPPET_LIMIT,
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
  type WorkroomTimelineAgentChildStatus,
  type WorkroomTimelineMessage,
  type WorkroomTimelinePart,
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
  recordFromUnknown,
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
    error: S.Defect(),
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

// ---------------------------------------------------------------------------
// T14 (#8871, epic #8857 Wave 3): classify a raw agent/Codex event into the
// widened `WorkroomTimelinePart` union instead of always collapsing to the
// generic `tool` part.
//
// Ground truth for these raw event shapes is `OmniEventRecord.payloadJson`
// (a free-form runner-callback envelope — `rawEventPayloadRecord` above
// already unwraps the `dataJson`/`rawPayloadJson` nesting), which carries
// EITHER Codex-style `ThreadItem`-shaped fields (`commandExecution`,
// `fileChange`, `mcpToolCall`, ...; see
// `apps/openagents-desktop/src/workbench-item-contract.ts`'s
// `workbenchItemFromThreadItem`) OR Claude Agent SDK content-block shapes
// (`tool_use`/`tool_result` with a tool `name` from the same vocabulary
// `apps/openagents-desktop/src/renderer/tool-cards.ts`'s
// `humanizeToolInvocation` names — `Bash`, `Write`, `Edit`, `Read`, `Grep`,
// `Glob`, `Agent`, `mcp__*`), depending on which backend executed the run.
// Reusing `workbenchItemFromThreadItem`/`humanizeToolInvocation` directly
// isn't possible: both live in `apps/openagents-desktop`, an Electron
// application package, and this Cloud Run API must not depend on another
// app's package. This classifier is a deliberate, tolerant SERVER-SIDE
// equivalent covering the same two vocabularies, with the same redaction/
// bounding discipline `safePublicText` already applies here, so the
// fallback below (unchanged from the pre-#8871 behavior) still runs for any
// event shape neither vocabulary recognizes.
// ---------------------------------------------------------------------------

type BoundedTextResult = Readonly<{ text: string; capped: boolean }>

/** Bounds text at `max` characters, preserving newlines (unlike
 * `safePublicText`/`compactLine`, which flatten to one line) so multi-line
 * command output, diffs, and args stay readable in the typed cards. Always
 * runs the same secret-material check `safePublicText` uses; a match
 * redacts the WHOLE value rather than partially masking it. */
const safeBoundedText = (
  value: string | undefined,
  max: number,
  mode: 'head' | 'tail' = 'head',
): BoundedTextResult | undefined => {
  if (value === undefined) {
    return undefined
  }

  const trimmed = value.trim()

  if (trimmed === '') {
    return undefined
  }

  if (containsProviderSecretMaterial(trimmed)) {
    return { text: '[redacted]', capped: false }
  }

  if (trimmed.length <= max) {
    return { text: trimmed, capped: false }
  }

  return mode === 'tail'
    ? { text: trimmed.slice(-max), capped: true }
    : { text: trimmed.slice(0, max), capped: true }
}

const diffLineCounts = (
  diff: string,
): Readonly<{ adds: number; dels: number }> => {
  let adds = 0
  let dels = 0

  for (const line of diff.split('\n')) {
    if (line.startsWith('+') && !line.startsWith('+++')) {
      adds++
    } else if (line.startsWith('-') && !line.startsWith('---')) {
      dels++
    }
  }

  return { adds, dels }
}

/** Bounded k/v projection of a JSON arguments payload (objects only),
 * mirroring `workbenchArgEntries` in the desktop contract. */
const workroomArgEntries = (
  value: unknown,
): ReadonlyArray<Readonly<{ key: string; value: string }>> => {
  const record = recordFromUnknown(value)

  if (record === undefined) {
    return []
  }

  const entries: Array<Readonly<{ key: string; value: string }>> = []

  for (const [key, raw] of Object.entries(record)) {
    if (entries.length >= SHARE_ARG_LIMIT) {
      break
    }

    const rendered =
      typeof raw === 'string'
        ? raw
        : typeof raw === 'number' || typeof raw === 'boolean'
          ? String(raw)
          : raw === null || raw === undefined
            ? ''
            : (() => {
                try {
                  return JSON.stringify(raw) ?? ''
                } catch {
                  return ''
                }
              })()
    const boundedKey = safeBoundedText(key, SHARE_ARG_KEY_LIMIT)
    const boundedValue = safeBoundedText(rendered, SHARE_ARG_VALUE_LIMIT)

    entries.push({
      key: boundedKey?.text ?? key.slice(0, SHARE_ARG_KEY_LIMIT),
      value: boundedValue?.text ?? '',
    })
  }

  return entries
}

/** Bounded text extraction from an MCP-style result payload
 * (`{content:[{text}...]}`), mirroring `mcpResultSnippet` in the desktop
 * contract. */
const mcpResultSnippetFromRaw = (value: unknown): string | undefined => {
  if (value === null || value === undefined) {
    return undefined
  }

  const record = recordFromUnknown(value)
  const content = record?.content

  if (Array.isArray(content)) {
    const text = content
      .map(part => optionalString(recordFromUnknown(part)?.text) ?? '')
      .filter(part => part !== '')
      .join('\n')

    if (text !== '') {
      return safeBoundedText(text, SHARE_RESULT_SNIPPET_LIMIT)?.text
    }
  }

  if (typeof value === 'string') {
    return safeBoundedText(value, SHARE_RESULT_SNIPPET_LIMIT)?.text
  }

  try {
    const rendered = JSON.stringify(value)

    return rendered === undefined
      ? undefined
      : safeBoundedText(rendered, SHARE_RESULT_SNIPPET_LIMIT)?.text
  } catch {
    return undefined
  }
}

/** Prefers a nested `raw.part` content-block record when present (the
 * shape `eventLooksLikeToolCall`/`eventDetail` above already special-case),
 * falling back to the event's own raw payload record. */
const rawItemRecord = (
  raw: Record<string, unknown> | undefined,
): Record<string, unknown> | undefined => {
  if (raw === undefined) {
    return undefined
  }

  const part = recordFromUnknown(raw.part)

  return part ?? raw
}

const rawItemType = (
  raw: Record<string, unknown> | undefined,
  target: Record<string, unknown> | undefined,
): string => optionalString(target?.type) ?? optionalString(raw?.type) ?? ''

const normalizedApprovalDecision = (
  raw: string | undefined,
): 'approved' | 'denied' | undefined => {
  if (raw === undefined) {
    return undefined
  }

  const lowered = raw.toLowerCase()

  if (
    ['accept', 'acceptforsession', 'approve', 'approved', 'accepted', 'allow', 'allowed'].includes(
      lowered,
    )
  ) {
    return 'approved'
  }

  if (
    ['decline', 'declined', 'deny', 'denied', 'reject', 'rejected'].includes(
      lowered,
    )
  ) {
    return 'denied'
  }

  return undefined
}

const reasoningPartFromRaw = (
  target: Record<string, unknown>,
): WorkroomTimelinePart | undefined => {
  const summaryParts = arrayFromUnknown(target.summary)
  const joinedSummary = summaryParts
    ?.map(part => optionalString(part) ?? '')
    .filter(part => part !== '')
    .join('\n')
  const summary =
    joinedSummary !== undefined && joinedSummary !== ''
      ? joinedSummary
      : (optionalString(target.summary) ??
        optionalString(target.thinking) ??
        optionalString(target.text))
  const bounded = safeBoundedText(summary, SHARE_REASONING_SUMMARY_LIMIT)

  return bounded === undefined
    ? undefined
    : { kind: 'reasoning', summary: bounded.text }
}

const planPartFromRaw = (
  target: Record<string, unknown>,
): WorkroomTimelinePart | undefined => {
  const prose = safeBoundedText(optionalString(target.text), SHARE_PLAN_PROSE_LIMIT)
  const rawEntries = arrayFromUnknown(target.entries ?? target.steps) ?? []
  const entries = rawEntries.slice(0, SHARE_PLAN_ENTRY_LIMIT).flatMap(rawEntry => {
    const record = recordFromUnknown(rawEntry)
    const step = safeBoundedText(
      optionalString(record?.step) ?? optionalString(record?.text),
      SHARE_PLAN_STEP_LIMIT,
    )

    if (step === undefined) {
      return []
    }

    const rawStatus = optionalString(record?.status)
    const status: 'pending' | 'in_progress' | 'completed' =
      rawStatus === 'completed'
        ? 'completed'
        : rawStatus === 'in_progress' || rawStatus === 'running'
          ? 'in_progress'
          : 'pending'

    return [{ step: step.text, status }]
  })

  if (entries.length === 0 && prose === undefined) {
    return undefined
  }

  return {
    kind: 'plan',
    entries,
    ...(prose === undefined ? {} : { prose: prose.text }),
  }
}

const approvalPartFromRaw = (
  target: Record<string, unknown>,
): WorkroomTimelinePart => {
  const decision = normalizedApprovalDecision(optionalString(target.decision))
  const detail = safeBoundedText(
    optionalString(target.reason) ?? optionalString(target.message),
    SHARE_ERROR_MESSAGE_LIMIT,
  )

  return {
    kind: 'approval',
    ...(decision === undefined ? {} : { decision }),
    ...(detail === undefined ? {} : { detail: detail.text }),
  }
}

const commandPartFromRaw = (
  target: Record<string, unknown>,
  itemType: string,
  status: WorkroomTimelineToolPart['status'],
): WorkroomTimelinePart | undefined => {
  const isCommandExecution =
    itemType === 'commandExecution' || itemType === 'command_execution'
  const isBashToolUse =
    itemType === 'tool_use' && optionalString(target.name) === 'Bash'

  if (!isCommandExecution && !isBashToolUse) {
    return undefined
  }

  const commandText = isCommandExecution
    ? optionalString(target.command)
    : optionalNestedString(target, [['input', 'command']])
  const bounded = safeBoundedText(commandText, SHARE_COMMAND_LIMIT)

  if (bounded === undefined) {
    return undefined
  }

  const cwd = safeBoundedText(
    optionalString(target.cwd) ?? optionalNestedString(target, [['input', 'cwd']]),
    SHARE_PATH_LIMIT,
  )
  const exitCode = optionalInteger(target.exitCode ?? target.exit_code)
  const durationMs = optionalInteger(target.durationMs ?? target.duration_ms)
  const outputRaw =
    optionalString(
      target.aggregatedOutput ?? target.aggregated_output ?? target.output,
    ) ?? optionalNestedString(target, [['detail'], ['message']])
  const outputBound = safeBoundedText(outputRaw, SHARE_OUTPUT_TAIL_LIMIT, 'tail')

  return {
    kind: 'command',
    command: bounded.text,
    status,
    ...(cwd === undefined ? {} : { cwd: cwd.text }),
    ...(exitCode === undefined ? {} : { exitCode }),
    ...(durationMs === undefined ? {} : { durationMs }),
    ...(outputBound === undefined ? {} : { outputTail: outputBound.text }),
    ...(outputBound?.capped === true ? { outputCapReached: true } : {}),
  }
}

const fileChangeEntriesFromRawChanges = (
  target: Record<string, unknown>,
): ReadonlyArray<
  Readonly<{
    path: string
    kind: 'add' | 'delete' | 'update'
    adds?: number
    dels?: number
    diff?: string
    diffCapReached?: boolean
  }>
> => {
  const rawChanges = arrayFromUnknown(target.changes)

  if (rawChanges === undefined) {
    return []
  }

  return rawChanges.slice(0, SHARE_CHANGE_LIMIT).flatMap(rawChange => {
    const record = recordFromUnknown(rawChange)
    const path = safeBoundedText(optionalString(record?.path), SHARE_PATH_LIMIT)

    if (path === undefined) {
      return []
    }

    const rawKind =
      optionalString(record?.kind) ??
      optionalString(recordFromUnknown(record?.type)?.type)
    const kind: 'add' | 'delete' | 'update' =
      rawKind === 'add' || rawKind === 'delete' ? rawKind : 'update'
    const diffBound = safeBoundedText(optionalString(record?.diff), SHARE_DIFF_LIMIT)
    const counts = diffBound === undefined ? undefined : diffLineCounts(diffBound.text)

    return [
      {
        path: path.text,
        kind,
        ...(counts === undefined ? {} : { adds: counts.adds, dels: counts.dels }),
        ...(diffBound === undefined ? {} : { diff: diffBound.text }),
        ...(diffBound?.capped === true ? { diffCapReached: true } : {}),
      },
    ]
  })
}

const fileChangePartFromRaw = (
  target: Record<string, unknown>,
  itemType: string,
  status: WorkroomTimelineToolPart['status'],
): WorkroomTimelinePart | undefined => {
  const isFileChange =
    itemType === 'fileChange' || itemType === 'file_change'
  const isApplyPatch = itemType === 'apply_patch' || itemType === 'applyPatch'
  const isWriteOrEditToolUse =
    itemType === 'tool_use' &&
    (optionalString(target.name) === 'Write' || optionalString(target.name) === 'Edit')

  if (isFileChange) {
    const changes = fileChangeEntriesFromRawChanges(target)

    return changes.length === 0 ? undefined : { kind: 'fileChange', status, changes }
  }

  if (isApplyPatch) {
    const rawPatch = target.patch ?? target.input ?? target.arguments ?? target.content
    const patchRecord =
      recordFromUnknown(rawPatch) ??
      (typeof rawPatch === 'string' ? safeJsonRecord(rawPatch) : undefined)
    const patchText =
      optionalString(patchRecord?.patch) ??
      optionalString(patchRecord?.input) ??
      optionalString(rawPatch)
    const bounded = safeBoundedText(patchText, SHARE_DIFF_LIMIT)

    if (bounded === undefined) {
      return undefined
    }

    const counts = diffLineCounts(bounded.text)

    return {
      kind: 'fileChange',
      status,
      changes: [
        {
          path: 'Turn diff',
          kind: 'update',
          adds: counts.adds,
          dels: counts.dels,
          diff: bounded.text,
          ...(bounded.capped ? { diffCapReached: true } : {}),
        },
      ],
    }
  }

  if (isWriteOrEditToolUse) {
    const path = safeBoundedText(
      optionalNestedString(target, [['input', 'file_path']]),
      SHARE_PATH_LIMIT,
    )

    if (path === undefined) {
      return undefined
    }

    const oldString = optionalNestedString(target, [['input', 'old_string']])
    const newString = optionalNestedString(target, [['input', 'new_string']])
    const content = optionalNestedString(target, [['input', 'content']])
    const diffText =
      oldString !== undefined || newString !== undefined
        ? `-${oldString ?? ''}\n+${newString ?? ''}`
        : content
    const diffBound = safeBoundedText(diffText, SHARE_DIFF_LIMIT)
    const counts = diffBound === undefined ? undefined : diffLineCounts(diffBound.text)
    const changeKind: 'add' | 'update' =
      oldString === undefined && content !== undefined ? 'add' : 'update'

    return {
      kind: 'fileChange',
      status,
      changes: [
        {
          path: path.text,
          kind: changeKind,
          ...(counts === undefined ? {} : { adds: counts.adds, dels: counts.dels }),
          ...(diffBound === undefined ? {} : { diff: diffBound.text }),
          ...(diffBound?.capped === true ? { diffCapReached: true } : {}),
        },
      ],
    }
  }

  return undefined
}

const toolCallPartFromRaw = (
  target: Record<string, unknown>,
  itemType: string,
  status: WorkroomTimelineToolPart['status'],
): WorkroomTimelinePart | undefined => {
  if (itemType === 'mcpToolCall' || itemType === 'mcp_tool_call') {
    const tool = safeBoundedText(
      optionalString(target.tool ?? target.tool_name ?? target.name) ?? 'tool',
      120,
    )
    const server = safeBoundedText(
      optionalString(target.server ?? target.server_name),
      120,
    )
    const errorMessage = safeBoundedText(
      optionalString(recordFromUnknown(target.error)?.message),
      SHARE_ERROR_MESSAGE_LIMIT,
    )
    const durationMs = optionalInteger(target.durationMs ?? target.duration_ms)
    const resultSnippet = mcpResultSnippetFromRaw(target.result)

    return {
      kind: 'toolCall',
      callKind: 'mcp',
      tool: tool?.text ?? 'tool',
      args: workroomArgEntries(target.arguments ?? target.args),
      status,
      ...(server === undefined ? {} : { server: server.text }),
      ...(resultSnippet === undefined ? {} : { resultSnippet }),
      ...(errorMessage === undefined ? {} : { errorMessage: errorMessage.text }),
      ...(durationMs === undefined ? {} : { durationMs }),
    }
  }

  if (itemType === 'dynamicToolCall' || itemType === 'dynamic_tool_call' || itemType === 'custom_tool_call') {
    const tool = safeBoundedText(optionalString(target.tool ?? target.name) ?? 'tool', 120)
    const namespace = safeBoundedText(optionalString(target.namespace), 120)
    const durationMs = optionalInteger(target.durationMs ?? target.duration_ms)

    return {
      kind: 'toolCall',
      callKind: 'dynamic',
      tool: tool?.text ?? 'tool',
      args: workroomArgEntries(target.arguments ?? target.args ?? target.input),
      status,
      ...(namespace === undefined ? {} : { namespace: namespace.text }),
      ...(durationMs === undefined ? {} : { durationMs }),
    }
  }

  if (itemType === 'webSearch' || itemType === 'web_search') {
    const query = safeBoundedText(optionalString(target.query), SHARE_ERROR_MESSAGE_LIMIT)
    const results = arrayFromUnknown(target.results)

    return {
      kind: 'toolCall',
      callKind: 'web',
      tool: 'webSearch',
      args: [],
      status,
      ...(query === undefined ? {} : { query: query.text }),
      ...(results === undefined ? {} : { resultCount: results.length }),
    }
  }

  if (
    itemType === 'imageGeneration' ||
    itemType === 'image_generation' ||
    itemType === 'imageView' ||
    itemType === 'image_view'
  ) {
    const path = safeBoundedText(
      optionalString(target.savedPath ?? target.saved_path ?? target.path),
      SHARE_PATH_LIMIT,
    )
    const revised = safeBoundedText(
      optionalString(target.revisedPrompt ?? target.revised_prompt),
      SHARE_RESULT_SNIPPET_LIMIT,
    )

    return {
      kind: 'toolCall',
      callKind: 'image',
      tool: itemType.toLowerCase().includes('view') ? 'imageView' : 'imageGeneration',
      args: [],
      status,
      ...(revised === undefined ? {} : { resultSnippet: revised.text }),
      ...(path === undefined ? {} : { path: path.text }),
    }
  }

  if (itemType === 'tool_result') {
    const isError = target.is_error === true
    const snippet = mcpResultSnippetFromRaw(target.content ?? target.result)

    return {
      kind: 'toolCall',
      callKind: 'dynamic',
      tool: safeBoundedText(optionalString(target.tool), 120)?.text ?? 'tool_result',
      args: [],
      status: isError ? 'failed' : status,
      ...(snippet === undefined ? {} : { resultSnippet: snippet }),
      ...(isError ? { errorMessage: 'Tool call failed.' } : {}),
    }
  }

  if (itemType === 'tool_use') {
    const name = optionalString(target.name) ?? 'tool'

    // Bash/Write/Edit are classified as command/fileChange parts above.
    if (name === 'Bash' || name === 'Write' || name === 'Edit') {
      return undefined
    }

    // spawnAgent-style delegation reads as an `agent` part below.
    if (name === 'Agent' || name === 'Task') {
      return undefined
    }

    const isMcp = name.startsWith('mcp__')
    const segments = isMcp ? name.split('__').filter(part => part !== '') : []
    const server = isMcp && segments.length >= 2 ? segments[1] : undefined
    const tool = isMcp && segments.length >= 3 ? segments.slice(2).join('__') : name

    return {
      kind: 'toolCall',
      callKind: server === undefined ? 'dynamic' : 'mcp',
      tool: safeBoundedText(tool, 120)?.text ?? tool,
      args: workroomArgEntries(target.input),
      status,
      ...(server === undefined ? {} : { server: safeBoundedText(server, 120)?.text ?? server }),
    }
  }

  return undefined
}

const agentChildStatusFromRaw = (
  value: unknown,
): WorkroomTimelineAgentChildStatus | undefined => {
  const raw = optionalString(value)

  if (raw === undefined) {
    return undefined
  }

  const normalized = raw === 'pending_init' ? 'pendingInit' : raw === 'not_found' ? 'notFound' : raw

  return normalized === 'pendingInit' ||
    normalized === 'running' ||
    normalized === 'interrupted' ||
    normalized === 'completed' ||
    normalized === 'errored' ||
    normalized === 'shutdown' ||
    normalized === 'notFound'
    ? normalized
    : undefined
}

const agentPartFromRaw = (
  target: Record<string, unknown>,
  itemType: string,
  status: WorkroomTimelineToolPart['status'],
): WorkroomTimelinePart | undefined => {
  const isCollabAgent =
    itemType === 'collabAgentToolCall' || itemType === 'collab_agent_tool_call'
  const isAgentToolUse =
    itemType === 'tool_use' &&
    (optionalString(target.name) === 'Agent' || optionalString(target.name) === 'Task')

  if (!isCollabAgent && !isAgentToolUse) {
    return undefined
  }

  const tool = optionalString(target.tool)
  const promptText = isAgentToolUse
    ? optionalNestedString(target, [['input', 'prompt'], ['input', 'description']])
    : optionalString(target.prompt)
  const prompt = safeBoundedText(promptText, SHARE_AGENT_PROMPT_LIMIT)
  const statesRecord = recordFromUnknown(target.agentsStates ?? target.agents_states)
  const children = statesRecord === undefined
    ? []
    : Object.entries(statesRecord).flatMap(([threadRef, raw]) => {
        const childStatus = agentChildStatusFromRaw(recordFromUnknown(raw)?.status)

        return childStatus === undefined
          ? []
          : [{ threadRef: threadRef.slice(0, 120), status: childStatus }]
      })

  return {
    kind: 'agent',
    status,
    ...(tool === undefined ? {} : { tool: tool.slice(0, 40) }),
    ...(prompt === undefined ? {} : { prompt: prompt.text }),
    ...(children.length === 0 ? {} : { children: children.slice(0, 16) }),
  }
}

const noticePartFromEvent = (
  event: OmniEventRecord,
  target: Record<string, unknown>,
): WorkroomTimelinePart | undefined => {
  const looksLikeNotice =
    event.type.includes('warning') ||
    event.type.includes('notice') ||
    event.type.includes('deprecation')

  if (!looksLikeNotice) {
    return undefined
  }

  const text = safeBoundedText(
    optionalString(target.message) ?? event.summary,
    SHARE_NOTICE_TEXT_LIMIT,
  )

  if (text === undefined) {
    return undefined
  }

  return {
    kind: 'notice',
    severity: event.type.includes('error') ? 'error' : 'warning',
    text: text.text,
  }
}

const meterPartFromEvent = (
  event: OmniEventRecord,
  target: Record<string, unknown>,
): WorkroomTimelinePart | undefined => {
  const looksLikeUsage =
    event.type.includes('tokenUsage') ||
    event.type.includes('token_usage') ||
    event.type.includes('rateLimit')

  if (!looksLikeUsage) {
    return undefined
  }

  const usage = recordFromUnknown(target.usage ?? target.tokenUsage) ?? target
  const inputTokens = optionalInteger(
    nestedUnknown(usage, ['inputTokens']) ?? nestedUnknown(usage, ['input_tokens']),
  )
  const cachedInputTokens = optionalInteger(
    nestedUnknown(usage, ['cachedInputTokens']) ?? nestedUnknown(usage, ['cached_input_tokens']),
  )
  const outputTokens = optionalInteger(
    nestedUnknown(usage, ['outputTokens']) ?? nestedUnknown(usage, ['output_tokens']),
  )
  const reasoningTokens = optionalInteger(
    nestedUnknown(usage, ['reasoningTokens']) ?? nestedUnknown(usage, ['reasoning_tokens']),
  )
  const totalTokens = optionalInteger(
    nestedUnknown(usage, ['totalTokens']) ?? nestedUnknown(usage, ['total_tokens']),
  )

  if (
    inputTokens === undefined &&
    outputTokens === undefined &&
    totalTokens === undefined
  ) {
    return undefined
  }

  return {
    kind: 'meter',
    ...(inputTokens === undefined ? {} : { inputTokens }),
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(outputTokens === undefined ? {} : { outputTokens }),
    ...(reasoningTokens === undefined ? {} : { reasoningTokens }),
    ...(totalTokens === undefined ? {} : { totalTokens }),
  }
}

/**
 * Classifies one raw agent/Codex `OmniEventRecord` into the widened
 * `WorkroomTimelinePart` union. Falls back to the pre-#8871 generic `tool`
 * part — unchanged — for any event shape neither the Codex nor the Claude
 * Agent SDK vocabulary above recognizes, so nothing regresses for event
 * shapes this classifier does not (yet) know about.
 */
export const workroomPartFromEvent = (event: OmniEventRecord): WorkroomTimelinePart => {
  const raw = rawEventPayloadRecord(event)
  const target = rawItemRecord(raw) ?? {}
  const itemType = rawItemType(raw, target)
  const status = eventStatus(event)

  const reasoning =
    itemType === 'reasoning' || itemType === 'thinking'
      ? reasoningPartFromRaw(target)
      : undefined
  if (reasoning !== undefined) return reasoning

  const plan = itemType === 'plan' ? planPartFromRaw(target) : undefined
  if (plan !== undefined) return plan

  if (itemType.includes('approval')) return approvalPartFromRaw(target)

  if (itemType === 'contextCompaction' || itemType === 'context_compaction') {
    return { kind: 'compaction' }
  }

  const notice = noticePartFromEvent(event, target)
  if (notice !== undefined) return notice

  const meter = meterPartFromEvent(event, target)
  if (meter !== undefined) return meter

  const command = commandPartFromRaw(target, itemType, status)
  if (command !== undefined) return command

  const fileChange = fileChangePartFromRaw(target, itemType, status)
  if (fileChange !== undefined) return fileChange

  const agent = agentPartFromRaw(target, itemType, status)
  if (agent !== undefined) return agent

  const toolCall = toolCallPartFromRaw(target, itemType, status)
  if (toolCall !== undefined) return toolCall

  return {
    kind: 'tool',
    title: safePublicText(event.summary, event.type, 120),
    subtitle: event.source,
    status,
    detail: eventDetail(event),
  }
}

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
      parts: [workroomPartFromEvent(event)],
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
