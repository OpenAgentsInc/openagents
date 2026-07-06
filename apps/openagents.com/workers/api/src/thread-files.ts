import { readIdentityUserProfiles, type IdentityDb } from './identity-db'
import { Effect, Layer, Schema as S } from 'effect'
import * as Context from 'effect/Context'

import { OpenAgentsDatabase } from './bindings'
import { currentDate, dashedRandomId } from './runtime-primitives'

export type TeamChatKind =
  | 'message'
  | 'autopilot_intent'
  | 'adjutant_intent'
  | 'system'

export type TeamMembershipRole = 'owner' | 'admin' | 'member' | 'viewer'

export type ReadActiveTeamMembershipRole = (
  db: D1Database,
  teamId: string,
  userId: string,
) => Promise<TeamMembershipRole | undefined>

export type ThreadFileRuntime = Readonly<{
  now: () => Date
  randomId: (prefix: string) => string
}>

export const systemThreadFileRuntime: ThreadFileRuntime = {
  now: currentDate,
  randomId: dashedRandomId,
}

export type ThreadFileScope = 'personal' | 'team'

export type ThreadFileRow = Readonly<{
  checksum_sha256: string | null
  content_type: string
  created_at: string
  download_enabled: number
  filename: string
  id: string
  object_key: string
  owner_user_id: string
  scope: ThreadFileScope
  size_bytes: number
  team_id: string | null
  team_ref: string | null
  thread_id: string
}>

export type PublicThreadFile = Readonly<{
  contentType: string
  createdAt: string
  detailUrl: string
  downloadEnabled: boolean
  downloadUrl: string
  filename: string
  id: string
  ownerUserId: string
  scope: ThreadFileScope
  sizeBytes: number
  teamId: string | null
  threadId: string
}>

export type ThreadFileReferenceKind =
  | 'message_attachment'
  | 'autopilot_input'
  | 'autopilot_answer'

export type PublicThreadFileReference = Readonly<{
  author: Readonly<{
    avatarUrl: string | null
    githubUsername: string | null
    name: string
    userId: string
  }>
  body: string
  createdAt: string
  excerpt: string
  fileId: string
  href: string
  id: string
  messageId: string
  messageKind: TeamChatKind
  referenceKind: ThreadFileReferenceKind
  teamId: string | null
  threadId: string
}>

export type PublicThreadFileDetail = Readonly<{
  canManage: boolean
  file: PublicThreadFile
  references: ReadonlyArray<PublicThreadFileReference>
}>

export class ThreadFileRepositoryError extends S.TaggedErrorClass<ThreadFileRepositoryError>()(
  'ThreadFileRepositoryError',
  {
    message: S.String,
    operation: S.String,
  },
) {}

export type SetThreadFileDownloadEnabledInput = Readonly<{
  downloadEnabled: boolean
  fileId: string
}>

export type InsertThreadFileInput = Readonly<{
  checksumSha256: string
  contentType: string
  filename: string
  id: string
  objectKey: string
  ownerUserId: string
  scope: ThreadFileScope
  sizeBytes: number
  teamId: string | null
  threadId: string
}>

export type ListPersonalThreadFilesInput = Readonly<{
  ownerUserId: string
  threadId: string
}>

export type ListTeamThreadFilesInput = Readonly<{
  teamId: string
  threadId?: string
}>

export type ReadThreadFileDetailInput = Readonly<{
  readActiveTeamMembershipRole: ReadActiveTeamMembershipRole
  row: ThreadFileRow
  userId: string
}>

export type InsertThreadFileMessageReferencesInput = Readonly<{
  fileIds: ReadonlyArray<string>
  messageId: string
  referenceKind: ThreadFileReferenceKind
  teamId: string
  threadId: string
}>

const compactText = (value: string, maxLength: number): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

const safeObjectPathPart = (value: string): string =>
  value
    .trim()
    .replace(/[/\\]+/g, '-')
    .replace(/[^a-zA-Z0-9._:-]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 160) || 'file'

export const publicThreadFile = (row: ThreadFileRow): PublicThreadFile => ({
  contentType: row.content_type,
  createdAt: row.created_at,
  detailUrl:
    row.scope === 'team' && row.team_id !== null
      ? `/teams/${encodeURIComponent(row.team_ref ?? row.team_id)}/files/${encodeURIComponent(row.id)}`
      : `/files/${encodeURIComponent(row.id)}`,
  downloadEnabled: row.download_enabled === 1,
  downloadUrl: `/api/thread-files/${encodeURIComponent(row.id)}/download`,
  filename: row.filename,
  id: row.id,
  ownerUserId: row.owner_user_id,
  scope: row.scope,
  sizeBytes: row.size_bytes,
  teamId: row.team_id,
  threadId: row.thread_id,
})

export const threadFileObjectKey = (
  input: Readonly<{
    filename: string
    id: string
    ownerUserId: string
    scope: ThreadFileScope
    teamId: string | null
    threadId: string
  }>,
): string =>
  [
    'thread-files',
    input.scope,
    safeObjectPathPart(input.teamId ?? input.ownerUserId),
    safeObjectPathPart(input.threadId),
    safeObjectPathPart(input.id),
    safeObjectPathPart(input.filename),
  ].join('/')

export const sha256Hex = async (bytes: ArrayBuffer): Promise<string> => {
  const digest = await crypto.subtle.digest('SHA-256', bytes)

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const readThreadFileById = async (
  db: D1Database,
  fileId: string,
): Promise<ThreadFileRow | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         thread_files.id,
         thread_files.scope,
         thread_files.thread_id,
         thread_files.team_id,
         thread_files.owner_user_id,
         thread_files.filename,
         thread_files.content_type,
         thread_files.size_bytes,
         thread_files.object_key,
         thread_files.checksum_sha256,
         thread_files.download_enabled,
         COALESCE(teams.slug, thread_files.team_id) AS team_ref,
         thread_files.created_at
       FROM thread_files
       LEFT JOIN teams ON teams.id = thread_files.team_id
       WHERE thread_files.id = ?
         AND thread_files.deleted_at IS NULL
       LIMIT 1`,
    )
    .bind(fileId)
    .first<ThreadFileRow>()

  return row ?? undefined
}

export const listPersonalThreadFiles = async (
  db: D1Database,
  input: Readonly<{ ownerUserId: string; threadId: string }>,
): Promise<ReadonlyArray<PublicThreadFile>> => {
  const rows = await db
    .prepare(
      `SELECT
         thread_files.id,
         thread_files.scope,
         thread_files.thread_id,
         thread_files.team_id,
         thread_files.owner_user_id,
         thread_files.filename,
         thread_files.content_type,
         thread_files.size_bytes,
         thread_files.object_key,
         thread_files.checksum_sha256,
         thread_files.download_enabled,
         COALESCE(teams.slug, thread_files.team_id) AS team_ref,
         thread_files.created_at
       FROM thread_files
       LEFT JOIN teams ON teams.id = thread_files.team_id
       WHERE thread_files.scope = 'personal'
         AND thread_files.owner_user_id = ?
         AND thread_files.thread_id = ?
         AND thread_files.deleted_at IS NULL
       ORDER BY thread_files.created_at DESC
       LIMIT 200`,
    )
    .bind(input.ownerUserId, input.threadId)
    .all<ThreadFileRow>()

  return rows.results.map(publicThreadFile)
}

export const listTeamThreadFiles = async (
  db: D1Database,
  input: Readonly<{ teamId: string; threadId?: string }>,
): Promise<ReadonlyArray<PublicThreadFile>> => {
  const whereThread =
    input.threadId === undefined
      ? ''
      : '         AND thread_files.thread_id = ?\n'
  const rows = await db
    .prepare(
      `SELECT
         thread_files.id,
         thread_files.scope,
         thread_files.thread_id,
         thread_files.team_id,
         thread_files.owner_user_id,
         thread_files.filename,
         thread_files.content_type,
         thread_files.size_bytes,
         thread_files.object_key,
         thread_files.checksum_sha256,
         thread_files.download_enabled,
         COALESCE(teams.slug, thread_files.team_id) AS team_ref,
         thread_files.created_at
       FROM thread_files
       LEFT JOIN teams ON teams.id = thread_files.team_id
       WHERE thread_files.scope = 'team'
         AND thread_files.team_id = ?
${whereThread}
         AND thread_files.deleted_at IS NULL
       ORDER BY thread_files.created_at DESC
       LIMIT 500`,
    )
    .bind(
      ...(input.threadId === undefined
        ? [input.teamId]
        : [input.teamId, input.threadId]),
    )
    .all<ThreadFileRow>()

  return rows.results.map(publicThreadFile)
}

export const insertThreadFile = async (
  db: D1Database,
  input: InsertThreadFileInput,
  runtime: ThreadFileRuntime = systemThreadFileRuntime,
): Promise<PublicThreadFile> => {
  const now = runtime.now().toISOString()

  await db
    .prepare(
      `INSERT INTO thread_files
        (id, scope, thread_id, team_id, owner_user_id, filename, content_type, size_bytes, storage_provider, object_key, checksum_sha256, upload_status, scan_status, metadata_json, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'r2', ?, ?, 'uploaded', 'skipped', '{}', ?, ?)`,
    )
    .bind(
      input.id,
      input.scope,
      input.threadId,
      input.teamId,
      input.ownerUserId,
      input.filename,
      input.contentType,
      input.sizeBytes,
      input.objectKey,
      input.checksumSha256,
      now,
      now,
    )
    .run()

  const row = await readThreadFileById(db, input.id)

  if (row === undefined) {
    throw new ThreadFileRepositoryError({
      message: 'Inserted thread file could not be loaded.',
      operation: 'insert',
    })
  }

  return publicThreadFile(row)
}

export const authorizeThreadFileRead = async (
  db: D1Database,
  row: ThreadFileRow,
  userId: string,
  readActiveTeamMembershipRole: ReadActiveTeamMembershipRole,
): Promise<boolean> => {
  if (row.scope === 'personal') {
    return row.owner_user_id === userId
  }

  if (row.team_id === null) {
    return false
  }

  return (
    (await readActiveTeamMembershipRole(db, row.team_id, userId)) !== undefined
  )
}

export const authorizeThreadFileManage = async (
  db: D1Database,
  row: ThreadFileRow,
  userId: string,
  readActiveTeamMembershipRole: ReadActiveTeamMembershipRole,
): Promise<boolean> => {
  if (row.scope === 'personal') {
    return row.owner_user_id === userId
  }

  if (row.team_id === null) {
    return false
  }

  const role = await readActiveTeamMembershipRole(db, row.team_id, userId)

  return row.owner_user_id === userId || role === 'owner' || role === 'admin'
}

const threadFileMessageHref = (
  reference: Readonly<{
    message_id: string
    project_ref?: string | null
    team_ref: string | null
  }>,
): string =>
  reference.team_ref === null
    ? `/#message-${encodeURIComponent(reference.message_id)}`
    : reference.project_ref !== undefined && reference.project_ref !== null
      ? `/teams/${encodeURIComponent(reference.team_ref)}/projects/${encodeURIComponent(reference.project_ref)}/chat#message-${encodeURIComponent(reference.message_id)}`
      : `/teams/${encodeURIComponent(reference.team_ref)}/chat#message-${encodeURIComponent(reference.message_id)}`

const threadFileReferenceExcerpt = (body: string): string =>
  compactText(body, 280)

export const insertThreadFileMessageReferences = async (
  db: D1Database,
  input: InsertThreadFileMessageReferencesInput,
  runtime: ThreadFileRuntime = systemThreadFileRuntime,
): Promise<void> => {
  const fileIds = [...new Set(input.fileIds)].filter(fileId => fileId !== '')

  if (fileIds.length === 0) {
    return
  }

  const now = runtime.now().toISOString()

  await db.batch(
    fileIds.map(fileId =>
      db
        .prepare(
          `INSERT OR IGNORE INTO thread_file_message_refs
             (id, file_id, team_id, thread_id, message_id, reference_kind, created_at, updated_at)
           SELECT ?, thread_files.id, thread_files.team_id, ?, ?, ?, ?, ?
           FROM thread_files
           WHERE thread_files.id = ?
             AND thread_files.scope = 'team'
             AND thread_files.team_id = ?
             AND thread_files.deleted_at IS NULL`,
        )
        .bind(
          runtime.randomId('thread_file_message_ref'),
          input.threadId,
          input.messageId,
          input.referenceKind,
          now,
          now,
          fileId,
          input.teamId,
        ),
    ),
  )
}

export const listThreadFileReferences = async (
  db: D1Database,
  identityDb: IdentityDb,
  fileId: string,
): Promise<ReadonlyArray<PublicThreadFileReference>> => {
  // CFG-4 Domain 2 (#8519): author display fields come from the Postgres
  // identity handle now (one IN-list read for the page); the old INNER JOIN
  // users semantics are preserved by dropping references whose author row
  // no longer exists.
  const d1Rows = await db
    .prepare(
      `SELECT
         thread_file_message_refs.id,
         thread_file_message_refs.file_id,
         thread_file_message_refs.team_id,
         thread_file_message_refs.thread_id,
         thread_file_message_refs.message_id,
         thread_file_message_refs.reference_kind,
         thread_file_message_refs.created_at,
         team_chat_messages.kind AS message_kind,
         team_chat_messages.project_id,
         team_chat_messages.body,
         team_chat_messages.author_user_id,
         COALESCE(teams.slug, teams.id) AS team_ref,
         COALESCE(team_projects.slug, team_projects.id) AS project_ref
       FROM thread_file_message_refs
       INNER JOIN team_chat_messages
         ON team_chat_messages.id = thread_file_message_refs.message_id
        AND team_chat_messages.deleted_at IS NULL
        AND team_chat_messages.archived_at IS NULL
       LEFT JOIN teams ON teams.id = thread_file_message_refs.team_id
       LEFT JOIN team_projects ON team_projects.id = team_chat_messages.project_id
       WHERE thread_file_message_refs.file_id = ?
         AND thread_file_message_refs.deleted_at IS NULL
       ORDER BY thread_file_message_refs.created_at ASC`,
    )
    .bind(fileId)
    .all<
      Readonly<{
        author_user_id: string
        body: string
        created_at: string
        file_id: string
        id: string
        message_id: string
        message_kind: TeamChatKind
        project_id: string | null
        project_ref: string | null
        reference_kind: ThreadFileReferenceKind
        team_id: string | null
        team_ref: string | null
        thread_id: string
      }>
    >()

  const profiles = await readIdentityUserProfiles(
    identityDb,
    (d1Rows.results ?? []).map(row => row.author_user_id),
  )
  const rows = {
    results: (d1Rows.results ?? []).flatMap(row => {
      const profile = profiles.get(row.author_user_id)
      return profile === undefined
        ? []
        : [
            {
              ...row,
              author_avatar_url: profile.avatarUrl,
              author_github_username: profile.githubUsername,
              author_name: profile.displayName,
            },
          ]
    }),
  }

  return rows.results.map(row => ({
    author: {
      avatarUrl: row.author_avatar_url,
      githubUsername: row.author_github_username,
      name: row.author_name,
      userId: row.author_user_id,
    },
    body: row.body,
    createdAt: row.created_at,
    excerpt: threadFileReferenceExcerpt(row.body),
    fileId: row.file_id,
    href: threadFileMessageHref(row),
    id: row.id,
    messageId: row.message_id,
    messageKind: row.message_kind,
    referenceKind: row.reference_kind,
    teamId: row.team_id,
    threadId: row.thread_id,
  }))
}

export const readThreadFileDetail = async (
  db: D1Database,
  identityDb: IdentityDb,
  row: ThreadFileRow,
  userId: string,
  readActiveTeamMembershipRole: ReadActiveTeamMembershipRole,
): Promise<PublicThreadFileDetail> => ({
  canManage: await authorizeThreadFileManage(
    db,
    row,
    userId,
    readActiveTeamMembershipRole,
  ),
  file: publicThreadFile(row),
  references: await listThreadFileReferences(db, identityDb, row.id),
})

export type ThreadFileRepositoryShape = Readonly<{
  insert: (
    input: InsertThreadFileInput,
  ) => Effect.Effect<PublicThreadFile, ThreadFileRepositoryError>
  insertMessageReferences: (
    input: InsertThreadFileMessageReferencesInput,
  ) => Effect.Effect<void, ThreadFileRepositoryError>
  listPersonal: (
    input: ListPersonalThreadFilesInput,
  ) => Effect.Effect<ReadonlyArray<PublicThreadFile>, ThreadFileRepositoryError>
  listReferences: (
    fileId: string,
  ) => Effect.Effect<
    ReadonlyArray<PublicThreadFileReference>,
    ThreadFileRepositoryError
  >
  listTeam: (
    input: ListTeamThreadFilesInput,
  ) => Effect.Effect<ReadonlyArray<PublicThreadFile>, ThreadFileRepositoryError>
  readById: (
    fileId: string,
  ) => Effect.Effect<ThreadFileRow | undefined, ThreadFileRepositoryError>
  readDetail: (
    input: ReadThreadFileDetailInput,
  ) => Effect.Effect<PublicThreadFileDetail, ThreadFileRepositoryError>
  setDownloadEnabled: (
    input: SetThreadFileDownloadEnabledInput,
  ) => Effect.Effect<void, ThreadFileRepositoryError>
}>

export class ThreadFileRepository extends Context.Service<
  ThreadFileRepository,
  ThreadFileRepositoryShape
>()('@openagentsinc/ThreadFileRepository') {
  static layer = (
    db: D1Database,
    identityDb: IdentityDb,
    options: Readonly<{ runtime?: ThreadFileRuntime }> = {},
  ) =>
    Layer.succeed(
      ThreadFileRepository,
      makeD1ThreadFileRepository(db, identityDb, options),
    )

  static effectCfLayer = (
    identityDb: IdentityDb,
    options: Readonly<{ runtime?: ThreadFileRuntime }> = {},
  ) =>
    Layer.effect(
      ThreadFileRepository,
      Effect.map(OpenAgentsDatabase, db =>
        makeD1ThreadFileRepository(db, identityDb, options),
      ),
    )
}

export const makeD1ThreadFileRepository = (
  db: D1Database,
  // CFG-4 Domain 2 (#8519): Postgres identity handle for the reference
  // author enrichment.
  identityDb: IdentityDb,
  options: Readonly<{ runtime?: ThreadFileRuntime }> = {},
): ThreadFileRepositoryShape => ({
  insert: input =>
    repositoryEffect('thread_files.insert', async () =>
      insertThreadFile(db, input, options.runtime),
    ),
  insertMessageReferences: input =>
    repositoryEffect('thread_files.insert_message_references', async () =>
      insertThreadFileMessageReferences(db, input, options.runtime),
    ),
  listPersonal: input =>
    repositoryEffect('thread_files.list_personal', async () =>
      listPersonalThreadFiles(db, input),
    ),
  listReferences: fileId =>
    repositoryEffect('thread_files.list_references', async () =>
      listThreadFileReferences(db, identityDb, fileId),
    ),
  listTeam: input =>
    repositoryEffect('thread_files.list_team', async () =>
      listTeamThreadFiles(db, input),
    ),
  readById: fileId =>
    repositoryEffect('thread_files.read_by_id', async () =>
      readThreadFileById(db, fileId),
    ),
  readDetail: input =>
    repositoryEffect('thread_files.read_detail', async () =>
      readThreadFileDetail(
        db,
        identityDb,
        input.row,
        input.userId,
        input.readActiveTeamMembershipRole,
      ),
    ),
  setDownloadEnabled: input =>
    repositoryEffect('thread_files.set_download_enabled', async () => {
      await db
        .prepare(
          `UPDATE thread_files
             SET download_enabled = ?,
                 updated_at = ?
             WHERE id = ?
               AND deleted_at IS NULL`,
        )
        .bind(
          input.downloadEnabled ? 1 : 0,
          (options.runtime ?? systemThreadFileRuntime).now().toISOString(),
          input.fileId,
        )
        .run()
    }),
})

const repositoryEffect = <A>(
  operation: string,
  run: () => Promise<A>,
): Effect.Effect<A, ThreadFileRepositoryError> =>
  Effect.tryPromise({
    try: run,
    catch: error =>
      new ThreadFileRepositoryError({
        message: error instanceof Error ? error.message : String(error),
        operation,
      }),
  })

export const setThreadFileDownloadEnabled = (
  input: SetThreadFileDownloadEnabledInput,
) =>
  Effect.gen(function* () {
    const repository = yield* ThreadFileRepository

    yield* repository.setDownloadEnabled(input)
  })
