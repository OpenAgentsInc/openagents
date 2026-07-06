import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutatorName,
  personalScope,
  PushRequest,
  SyncSchemaVersion,
  threadScope,
} from "@openagentsinc/khala-sync"
import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  CHAT_MESSAGE_EXISTS_REJECTION,
  CHAT_RENAME_THREAD_MUTATOR_NAME,
  CHAT_SCOPE_REJECTION,
  CHAT_THREAD_NOT_FOUND_REJECTION,
  chatMutators,
} from "./chat-mutators.js"
import { logPage } from "./read-service.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import { readScopeOwner } from "./fleet-projection.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-chat-${clientCounter}`),
    clientId: ClientId.make(`c-chat-${clientCounter}`),
    userId: `user-chat-${clientCounter}`,
  }
}

const envelope = (id: number, name: string, args: unknown): MutationEnvelope =>
  new MutationEnvelope({
    argsJson: canonicalJson(args),
    mutationId: MutationId.make(id),
    name: MutatorName.make(name),
  })

const pushRequest = (
  client: { clientGroupId: ClientGroupId; clientId: ClientId },
  mutations: ReadonlyArray<MutationEnvelope>,
): PushRequest =>
  new PushRequest({
    clientGroupId: client.clientGroupId,
    clientId: client.clientId,
    mutations,
    protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
    schemaVersion,
  })

const registry = makeMutatorRegistry([...chatMutators])

describe.skipIf(!hasLocalPostgres())(
  "owner-private chat mutators against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_chat")
      await admin.end()
      const result = await runMigrations({
        databaseUrl: pg.urlFor("khala_sync_chat"),
      })
      expect(result.applied).toContain("0018_owner_private_chat.sql")
      sql = new SQL({ url: pg.urlFor("khala_sync_chat"), max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("create/append/rename writes owner-private scopes and appears via catch-up", async () => {
      const client = freshClient()
      const threadId = "chat-thread.flow.1"
      const messageId = "chat-message.flow.1"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId,
            title: "  Initial title  ",
          }),
          envelope(2, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "private hello from the owner",
            messageId,
            threadId,
          }),
          envelope(3, CHAT_RENAME_THREAD_MUTATOR_NAME, {
            threadId,
            title: "Renamed thread",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
      ])
      expect(Number(response.lastMutationId)).toBe(3)

      const ownerScope = personalScope(client.userId)
      const chatScope = threadScope(threadId)
      expect(await readScopeOwner(sql as unknown as SyncSql, chatScope)).toBe(
        client.userId,
      )

      const ownerLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 10,
        scope: ownerScope,
      })
      const threadLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 10,
        scope: chatScope,
      })

      expect(ownerLog.entries.map((entry) => String(entry.entityType))).toEqual([
        CHAT_THREAD_ENTITY_TYPE,
        CHAT_THREAD_ENTITY_TYPE,
        CHAT_THREAD_ENTITY_TYPE,
      ])
      expect(JSON.stringify(ownerLog.entries)).not.toContain(
        "private hello from the owner",
      )
      expect(threadLog.entries.map((entry) => String(entry.entityType))).toEqual([
        CHAT_THREAD_ENTITY_TYPE,
        CHAT_MESSAGE_ENTITY_TYPE,
        CHAT_THREAD_ENTITY_TYPE,
        CHAT_THREAD_ENTITY_TYPE,
      ])
      expect(JSON.stringify(threadLog.entries)).toContain(
        "private hello from the owner",
      )
      expect(
        threadLog.entries.every(
          (entry) => entry.mutationRef?.startsWith("mutation:") === true,
        ),
      ).toBe(true)

      const publicRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope LIKE 'scope.public.%'
      `
      expect(Number(publicRows[0]!.count)).toBe(0)

      const threadRows: Array<{
        title: string
        message_count: string | number
        last_message_at: string | null
      }> = await sql`
        SELECT title, message_count, last_message_at
        FROM khala_sync_chat_threads
        WHERE thread_id = ${threadId}
      `
      expect(threadRows[0]!.title).toBe("Renamed thread")
      expect(Number(threadRows[0]!.message_count)).toBe(1)
      expect(threadRows[0]!.last_message_at).not.toBeNull()
    })

    test("bindThreadRepo durably binds/clears a repo and rejects for a foreign owner or missing thread (#8472 follow-up)", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const threadId = "chat-thread.repo-binding.1"

      await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId,
            title: "Repo-bound thread",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })

      const bound = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(2, CHAT_BIND_THREAD_REPO_MUTATOR_NAME, {
            repo: { defaultBranch: "main", name: "openagents", owner: "OpenAgentsInc" },
            threadId,
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(bound.results[0]!.status).toBe("applied")

      const boundRows: Array<{
        repo_binding_owner: string | null
        repo_binding_name: string | null
        repo_binding_default_branch: string | null
      }> = await sql`
        SELECT repo_binding_owner, repo_binding_name, repo_binding_default_branch
        FROM khala_sync_chat_threads WHERE thread_id = ${threadId}
      `
      expect(boundRows[0]).toEqual({
        repo_binding_default_branch: "main",
        repo_binding_name: "openagents",
        repo_binding_owner: "OpenAgentsInc",
      })

      const chatScope = threadScope(threadId)
      const threadLogAfterBind = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 10,
        scope: chatScope,
      })
      expect(JSON.stringify(threadLogAfterBind.entries)).toContain("OpenAgentsInc")

      // A foreign user may not bind a repo onto someone else's thread.
      const foreignAttempt = await executePush({
        registry,
        request: pushRequest(intruder, [
          envelope(1, CHAT_BIND_THREAD_REPO_MUTATOR_NAME, {
            repo: { defaultBranch: "main", name: "other-repo", owner: "someone-else" },
            threadId,
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })
      expect(foreignAttempt.results[0]!.status).toBe("rejected")
      expect(foreignAttempt.results[0]!.errorCode).toBe(CHAT_SCOPE_REJECTION)

      // Binding onto a thread that does not exist rejects in-band.
      const missingThreadAttempt = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(3, CHAT_BIND_THREAD_REPO_MUTATOR_NAME, {
            repo: { defaultBranch: "main", name: "n", owner: "o" },
            threadId: "chat-thread.repo-binding.does-not-exist",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(missingThreadAttempt.results[0]!.status).toBe("rejected")
      expect(missingThreadAttempt.results[0]!.errorCode).toBe(
        CHAT_THREAD_NOT_FOUND_REJECTION,
      )

      // repo: null clears an existing binding back to "no repo".
      const cleared = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(4, CHAT_BIND_THREAD_REPO_MUTATOR_NAME, {
            repo: null,
            threadId,
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(cleared.results[0]!.status).toBe("applied")

      const clearedRows: Array<{
        repo_binding_owner: string | null
        repo_binding_name: string | null
        repo_binding_default_branch: string | null
      }> = await sql`
        SELECT repo_binding_owner, repo_binding_name, repo_binding_default_branch
        FROM khala_sync_chat_threads WHERE thread_id = ${threadId}
      `
      expect(clearedRows[0]).toEqual({
        repo_binding_default_branch: null,
        repo_binding_name: null,
        repo_binding_owner: null,
      })
    })

    test("foreign append rejects in-band and the following owner-private mutation still applies", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const ownerThread = "chat-thread.foreign.owner"
      const ownThread = "chat-thread.foreign.intruder"

      await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId: ownerThread,
            title: "Owner thread",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })

      const response = await executePush({
        registry,
        request: pushRequest(intruder, [
          envelope(1, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "should not land",
            messageId: "chat-message.foreign.1",
            threadId: ownerThread,
          }),
          envelope(2, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId: ownThread,
            title: "Intruder own thread",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })

      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe(CHAT_SCOPE_REJECTION)
      expect(response.results[1]!.status).toBe("applied")
      expect(Number(response.lastMutationId)).toBe(2)
      expect(
        await readScopeOwner(sql as unknown as SyncSql, threadScope(ownerThread)),
      ).toBe(owner.userId)
      expect(
        await readScopeOwner(sql as unknown as SyncSql, threadScope(ownThread)),
      ).toBe(intruder.userId)

      const leaked: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_chat_messages
        WHERE thread_id = ${ownerThread} AND author_user_id = ${intruder.userId}
      `
      expect(Number(leaked[0]!.count)).toBe(0)
    })

    test("business validation rejects are acked without changelog residue", async () => {
      const client = freshClient()
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "no thread yet",
            messageId: "chat-message.missing.1",
            threadId: "chat-thread.missing.1",
          }),
          envelope(2, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId: "chat-thread.after-rejection.1",
            title: "After rejection",
          }),
          envelope(3, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "first real message",
            messageId: "chat-message.after-rejection.1",
            threadId: "chat-thread.after-rejection.1",
          }),
          envelope(4, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "duplicate body",
            messageId: "chat-message.after-rejection.1",
            threadId: "chat-thread.after-rejection.1",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "rejected",
        "applied",
        "applied",
        "rejected",
      ])
      expect(response.results[0]!.errorCode).toBe(
        CHAT_THREAD_NOT_FOUND_REJECTION,
      )
      expect(response.results[3]!.errorCode).toBe(
        CHAT_MESSAGE_EXISTS_REJECTION,
      )
      expect(Number(response.lastMutationId)).toBe(4)

      const missingRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope = ${threadScope("chat-thread.missing.1")}
      `
      expect(Number(missingRows[0]!.count)).toBe(0)
      const messages: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_chat_messages
        WHERE thread_id = ${"chat-thread.after-rejection.1"}
      `
      expect(Number(messages[0]!.count)).toBe(1)
    })

    test("duplicate replay answers from the mutation ledger without re-executing", async () => {
      const client = freshClient()
      const request = pushRequest(client, [
        envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
          threadId: "chat-thread.duplicate.1",
          title: "Duplicate proof",
        }),
      ])

      const first = await executePush({
        registry,
        request,
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(first.results[0]!.status).toBe("applied")

      const replay = await executePush({
        registry,
        request,
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(replay.results[0]!.status).toBe("duplicate")

      const threads: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_chat_threads
        WHERE thread_id = ${"chat-thread.duplicate.1"}
      `
      expect(Number(threads[0]!.count)).toBe(1)
    })

    test("bad args reject in-band without echoing raw private values", async () => {
      const client = freshClient()
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "",
            messageId: "chat-message.bad.1",
            threadId: "chat-thread.bad.1",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe("invalid_args")
      expect(response.results[0]!.errorMessageSafe ?? "").not.toContain(
        "chat-message.bad.1",
      )
    })
  },
)
