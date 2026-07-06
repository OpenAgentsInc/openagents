import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  ChatThreadRepoBinding,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  EntityId,
  EntityType,
  MutationResult,
  MutatorName,
  personalScope,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"
import { ensureScopeOwner } from "./fleet-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * Owner-private chat mutators (MC-1, #8352).
 *
 * The server writes `khala_sync_chat_threads` / `khala_sync_chat_messages`
 * and the replicated changelog entries in one push-engine transaction.
 *
 * Scope layout:
 * - `scope.user.<owner>` carries `chat_thread` metadata only.
 * - `scope.thread.<threadId>` carries `chat_thread` metadata and
 *   `chat_message` bodies.
 *
 * No mutator here appends to `scope.public.*` or any shared firehose.
 */

export const CHAT_CREATE_THREAD_MUTATOR_NAME = "chat.createThread"
export const CHAT_APPEND_MESSAGE_MUTATOR_NAME = "chat.appendMessage"
export const CHAT_RENAME_THREAD_MUTATOR_NAME = "chat.renameThread"
export const CHAT_BIND_THREAD_REPO_MUTATOR_NAME = "chat.bindThreadRepo"

export const CHAT_SCOPE_REJECTION = "unauthorized_scope"
export const CHAT_THREAD_EXISTS_REJECTION = "thread_exists"
export const CHAT_THREAD_NOT_FOUND_REJECTION = "thread_not_found"
export const CHAT_MESSAGE_EXISTS_REJECTION = "message_exists"

const ChatRefField = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

const ChatTitleField = S.String.check(S.isMaxLength(160))
const ChatBodyField = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(20_000),
)

const CreateThreadArgs = S.Struct({
  threadId: ChatRefField,
  title: ChatTitleField,
})
type CreateThreadArgs = typeof CreateThreadArgs.Type

const AppendMessageArgs = S.Struct({
  threadId: ChatRefField,
  messageId: ChatRefField,
  body: ChatBodyField,
})
type AppendMessageArgs = typeof AppendMessageArgs.Type

const RenameThreadArgs = S.Struct({
  threadId: ChatRefField,
  title: ChatTitleField,
})
type RenameThreadArgs = typeof RenameThreadArgs.Type

const BindThreadRepoArgs = S.Struct({
  threadId: ChatRefField,
  repo: S.NullOr(ChatThreadRepoBinding),
})
type BindThreadRepoArgs = typeof BindThreadRepoArgs.Type

export const decodeChatCreateThreadArgs = (
  argsJson: string,
): CreateThreadArgs =>
  S.decodeUnknownSync(CreateThreadArgs)(JSON.parse(argsJson) as unknown)

export const decodeChatAppendMessageArgs = (
  argsJson: string,
): AppendMessageArgs =>
  S.decodeUnknownSync(AppendMessageArgs)(JSON.parse(argsJson) as unknown)

export const decodeChatRenameThreadArgs = (
  argsJson: string,
): RenameThreadArgs =>
  S.decodeUnknownSync(RenameThreadArgs)(JSON.parse(argsJson) as unknown)

export const decodeChatBindThreadRepoArgs = (
  argsJson: string,
): BindThreadRepoArgs =>
  S.decodeUnknownSync(BindThreadRepoArgs)(JSON.parse(argsJson) as unknown)

type ChatThreadRow = Readonly<{
  thread_id: string
  owner_user_id: string
  title: string
  status: string
  message_count: string | number
  last_message_at: string | null
  created_at: string
  updated_at: string
  repo_binding_owner: string | null
  repo_binding_name: string | null
  repo_binding_default_branch: string | null
}>

type ChatMessageRow = Readonly<{
  message_id: string
}>

const ChatThreadEntityType = EntityType.make(CHAT_THREAD_ENTITY_TYPE)
const ChatMessageEntityType = EntityType.make(CHAT_MESSAGE_ENTITY_TYPE)

const transactionNowIso = async (ctx: MutatorContext): Promise<string> => {
  const rows: Array<{ now: Date | string }> = await ctx.writer.sql`
    SELECT now() AS now
  `
  const raw = rows[0]?.now
  if (raw === undefined) throw new Error("SELECT now() returned no row")
  return raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()
}

const normalizeTitle = (title: string): string => title.trim()

/** `null` whenever any repo column is unset — covers both "never bound"
 * (all three columns NULL from row creation) and "explicitly cleared" via
 * `chat.bindThreadRepo` with `repo: null` (same result). This server
 * function always returns a decodable value (never the `undefined`/absent
 * case `ChatThreadEntity.repoBinding`'s `S.optional` also allows) since
 * every row read through here has all three columns; `undefined` only
 * matters for decoding pre-#8472 entities from elsewhere (e.g. an
 * on-device cache written before this field existed), not server reads.
 * All three columns are written together by `chat.bindThreadRepo`, so a
 * partial set (one column set, others null) is not an expected state;
 * treat it as "no binding" rather than throwing, since a decode failure
 * here would break every other read of an unrelated thread. */
const repoBindingFromRow = (row: ChatThreadRow): ChatThreadRepoBinding | null => {
  if (
    row.repo_binding_owner === null ||
    row.repo_binding_name === null ||
    row.repo_binding_default_branch === null
  ) {
    return null
  }
  return new ChatThreadRepoBinding({
    defaultBranch: row.repo_binding_default_branch,
    name: row.repo_binding_name,
    owner: row.repo_binding_owner,
  })
}

const threadEntityFromRow = (row: ChatThreadRow): ChatThreadEntity =>
  decodeChatThreadEntity({
    createdAt: row.created_at,
    lastMessageAt: row.last_message_at,
    messageCount: Number(row.message_count),
    ownerUserId: row.owner_user_id,
    repoBinding: repoBindingFromRow(row),
    status: row.status,
    threadId: row.thread_id,
    title: row.title,
    updatedAt: row.updated_at,
  })

const readThreadForUpdate = async (
  ctx: MutatorContext,
  threadId: string,
): Promise<ChatThreadRow | null> => {
  const rows: Array<ChatThreadRow> = await ctx.writer.sql`
    SELECT thread_id, owner_user_id, title, status, message_count,
           last_message_at, created_at, updated_at,
           repo_binding_owner, repo_binding_name, repo_binding_default_branch
    FROM khala_sync_chat_threads
    WHERE thread_id = ${threadId}
    FOR UPDATE
  `
  return rows[0] ?? null
}

const messageExists = async (
  ctx: MutatorContext,
  messageId: string,
): Promise<boolean> => {
  const rows: Array<ChatMessageRow> = await ctx.writer.sql`
    SELECT message_id FROM khala_sync_chat_messages
    WHERE message_id = ${messageId}
    LIMIT 1
  `
  return rows[0] !== undefined
}

const reject = (
  ctx: MutatorContext,
  errorCode: string,
  errorMessageSafe: string,
): MutationResult =>
  new MutationResult({
    errorCode,
    errorMessageSafe,
    mutationId: ctx.mutationId,
    status: "rejected",
  })

const rejectForeignScope = (ctx: MutatorContext): MutationResult =>
  reject(
    ctx,
    CHAT_SCOPE_REJECTION,
    "this chat thread scope belongs to a different user",
  )

const ensureThreadScopeOwner = async (
  ctx: MutatorContext,
  threadId: string,
): Promise<MutationResult | null> => {
  const owner = await ensureScopeOwner(ctx.writer.sql, threadScope(threadId), ctx.userId)
  return owner === ctx.userId ? null : rejectForeignScope(ctx)
}

const appendThreadEntityChanges = async (
  ctx: MutatorContext,
  entity: ChatThreadEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.threadId),
    entityType: ChatThreadEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: personalScope(entity.ownerUserId),
  })
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.threadId),
    entityType: ChatThreadEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const appendMessageEntityChange = async (
  ctx: MutatorContext,
  entity: ChatMessageEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.messageId),
    entityType: ChatMessageEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

export const chatCreateThreadMutator: MutatorDefinition =
  defineMutator<CreateThreadArgs>({
    decodeArgs: decodeChatCreateThreadArgs,
    execute: async (args, ctx) => {
      const existing = await readThreadForUpdate(ctx, args.threadId)
      if (existing !== null) {
        return existing.owner_user_id === ctx.userId
          ? reject(
              ctx,
              CHAT_THREAD_EXISTS_REJECTION,
              "this chat thread already exists",
            )
          : rejectForeignScope(ctx)
      }

      const ownerRejection = await ensureThreadScopeOwner(ctx, args.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const title = normalizeTitle(args.title)
      const inserted: Array<ChatThreadRow> = await ctx.writer.sql`
        INSERT INTO khala_sync_chat_threads
          (thread_id, owner_user_id, title, status, message_count,
           last_message_at, created_at, updated_at)
        VALUES
          (${args.threadId}, ${ctx.userId}, ${title}, 'active', 0,
           ${null}, ${nowIso}, ${nowIso})
        ON CONFLICT (thread_id) DO NOTHING
        RETURNING thread_id, owner_user_id, title, status, message_count,
                  last_message_at, created_at, updated_at,
                  repo_binding_owner, repo_binding_name, repo_binding_default_branch
      `
      const row = inserted[0]
      if (row === undefined) {
        return reject(
          ctx,
          CHAT_THREAD_EXISTS_REJECTION,
          "this chat thread already exists",
        )
      }

      await appendThreadEntityChanges(ctx, threadEntityFromRow(row))
      return new MutationResult({
        mutationId: ctx.mutationId,
        status: "applied",
      })
    },
    name: MutatorName.make(CHAT_CREATE_THREAD_MUTATOR_NAME),
  })

export const chatAppendMessageMutator: MutatorDefinition =
  defineMutator<AppendMessageArgs>({
    decodeArgs: decodeChatAppendMessageArgs,
    execute: async (args, ctx) => {
      const thread = await readThreadForUpdate(ctx, args.threadId)
      if (thread === null) {
        return reject(
          ctx,
          CHAT_THREAD_NOT_FOUND_REJECTION,
          "this chat thread does not exist",
        )
      }
      if (thread.owner_user_id !== ctx.userId) return rejectForeignScope(ctx)

      const ownerRejection = await ensureThreadScopeOwner(ctx, args.threadId)
      if (ownerRejection !== null) return ownerRejection

      if (await messageExists(ctx, args.messageId)) {
        return reject(
          ctx,
          CHAT_MESSAGE_EXISTS_REJECTION,
          "this chat message already exists",
        )
      }

      const nowIso = await transactionNowIso(ctx)
      const messageEntity = decodeChatMessageEntity({
        authorUserId: ctx.userId,
        body: args.body,
        createdAt: nowIso,
        deletedAt: null,
        messageId: args.messageId,
        threadId: args.threadId,
        updatedAt: nowIso,
      })

      await ctx.writer.sql`
        INSERT INTO khala_sync_chat_messages
          (message_id, thread_id, author_user_id, body, created_at, updated_at,
           deleted_at)
        VALUES
          (${messageEntity.messageId}, ${messageEntity.threadId},
           ${messageEntity.authorUserId}, ${messageEntity.body},
           ${messageEntity.createdAt}, ${messageEntity.updatedAt},
           ${messageEntity.deletedAt})
      `

      const updatedThreads: Array<ChatThreadRow> = await ctx.writer.sql`
        UPDATE khala_sync_chat_threads
        SET message_count = message_count + 1,
            last_message_at = ${nowIso},
            updated_at = ${nowIso}
        WHERE thread_id = ${args.threadId}
        RETURNING thread_id, owner_user_id, title, status, message_count,
                  last_message_at, created_at, updated_at,
                  repo_binding_owner, repo_binding_name, repo_binding_default_branch
      `
      const updatedThread = updatedThreads[0]
      if (updatedThread === undefined) {
        throw new Error("chat thread disappeared during append")
      }

      await appendThreadEntityChanges(ctx, threadEntityFromRow(updatedThread))
      await appendMessageEntityChange(ctx, messageEntity)

      return new MutationResult({
        mutationId: ctx.mutationId,
        status: "applied",
      })
    },
    name: MutatorName.make(CHAT_APPEND_MESSAGE_MUTATOR_NAME),
  })

export const chatRenameThreadMutator: MutatorDefinition =
  defineMutator<RenameThreadArgs>({
    decodeArgs: decodeChatRenameThreadArgs,
    execute: async (args, ctx) => {
      const thread = await readThreadForUpdate(ctx, args.threadId)
      if (thread === null) {
        return reject(
          ctx,
          CHAT_THREAD_NOT_FOUND_REJECTION,
          "this chat thread does not exist",
        )
      }
      if (thread.owner_user_id !== ctx.userId) return rejectForeignScope(ctx)

      const ownerRejection = await ensureThreadScopeOwner(ctx, args.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const updatedThreads: Array<ChatThreadRow> = await ctx.writer.sql`
        UPDATE khala_sync_chat_threads
        SET title = ${normalizeTitle(args.title)},
            updated_at = ${nowIso}
        WHERE thread_id = ${args.threadId}
        RETURNING thread_id, owner_user_id, title, status, message_count,
                  last_message_at, created_at, updated_at,
                  repo_binding_owner, repo_binding_name, repo_binding_default_branch
      `
      const updatedThread = updatedThreads[0]
      if (updatedThread === undefined) {
        throw new Error("chat thread disappeared during rename")
      }

      await appendThreadEntityChanges(ctx, threadEntityFromRow(updatedThread))
      return new MutationResult({
        mutationId: ctx.mutationId,
        status: "applied",
      })
    },
    name: MutatorName.make(CHAT_RENAME_THREAD_MUTATOR_NAME),
  })

/** Server side of MM-B2 (#8472)'s mobile repo picker. The mobile client has
 * applied this optimistically on-device since #8472 landed; this mutator is
 * what makes the binding durable server-side so it survives across
 * devices/sessions and reaches the org-cloud executor (#8473+). `repo: null`
 * explicitly clears a binding (distinct from a thread that was never bound,
 * which reads the same way — see `repoBindingFromRow`). */
export const chatBindThreadRepoMutator: MutatorDefinition =
  defineMutator<BindThreadRepoArgs>({
    decodeArgs: decodeChatBindThreadRepoArgs,
    execute: async (args, ctx) => {
      const thread = await readThreadForUpdate(ctx, args.threadId)
      if (thread === null) {
        return reject(
          ctx,
          CHAT_THREAD_NOT_FOUND_REJECTION,
          "this chat thread does not exist",
        )
      }
      if (thread.owner_user_id !== ctx.userId) return rejectForeignScope(ctx)

      const ownerRejection = await ensureThreadScopeOwner(ctx, args.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const updatedThreads: Array<ChatThreadRow> = await ctx.writer.sql`
        UPDATE khala_sync_chat_threads
        SET repo_binding_owner = ${args.repo?.owner ?? null},
            repo_binding_name = ${args.repo?.name ?? null},
            repo_binding_default_branch = ${args.repo?.defaultBranch ?? null},
            updated_at = ${nowIso}
        WHERE thread_id = ${args.threadId}
        RETURNING thread_id, owner_user_id, title, status, message_count,
                  last_message_at, created_at, updated_at,
                  repo_binding_owner, repo_binding_name, repo_binding_default_branch
      `
      const updatedThread = updatedThreads[0]
      if (updatedThread === undefined) {
        throw new Error("chat thread disappeared during bindThreadRepo")
      }

      await appendThreadEntityChanges(ctx, threadEntityFromRow(updatedThread))
      return new MutationResult({
        mutationId: ctx.mutationId,
        status: "applied",
      })
    },
    name: MutatorName.make(CHAT_BIND_THREAD_REPO_MUTATOR_NAME),
  })

export const chatMutators: ReadonlyArray<MutatorDefinition> = [
  chatCreateThreadMutator,
  chatAppendMessageMutator,
  chatRenameThreadMutator,
  chatBindThreadRepoMutator,
]
