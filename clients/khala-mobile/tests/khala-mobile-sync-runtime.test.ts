import { Database, type SQLQueryBindings } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import {
  BootstrapEntity,
  BootstrapResponse,
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  canonicalJson,
  ChangelogEntry,
  EntityId,
  EntityType,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  LogPage,
  MutationResult,
  PushResponse,
  SyncVersion,
  SyncVersionWatermark,
  personalScope,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
  type MutationEnvelope,
  type SyncScope
} from "@openagentsinc/khala-sync"
import {
  KhalaSyncTransportError,
  type KhalaSyncTransport,
  type LiveSocketHandlers
} from "@openagentsinc/khala-sync-client"
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME
} from "@openagentsinc/khala-sync-db-collection"
import { Effect } from "effect"

import {
  makeKhalaMobileMessageId,
  openKhalaMobileSyncRuntime
} from "../src/sync/khala-mobile-sync-runtime"
import type {
  ExpoSqliteDatabase,
  ExpoSqliteModule
} from "../src/sync/expo-db-sqlite-persistence"

const OWNER_ID = "user.mobile.owner"
const THREAD_ID = "thread.mobile.sync"
const FIXED_TIME = "2026-07-04T20:00:00.000Z"

const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

const expoSqliteFromBun = (): ExpoSqliteModule => {
  const databases = new Map<string, Database>()
  const open = (name: string): ExpoSqliteDatabase => {
    const db = databases.get(name) ?? new Database(":memory:")
    databases.set(name, db)
    return {
      execAsync: async statement => {
        db.exec(statement)
      },
      getAllAsync: async <T>(statement: string, ...params: ReadonlyArray<unknown>) =>
        db.query(statement).all(...(params as ReadonlyArray<SQLQueryBindings>)) as ReadonlyArray<T>,
      getFirstAsync: async <T>(statement: string, ...params: ReadonlyArray<unknown>) =>
        (db.query(statement).get(...(params as ReadonlyArray<SQLQueryBindings>)) as T | null) ?? null,
      runAsync: async (statement, ...params) => {
        db.query(statement).run(...(params as ReadonlyArray<SQLQueryBindings>))
      },
      withTransactionAsync: async task => task()
    }
  }

  return {
    openDatabaseAsync: async name => open(name)
  }
}

type FakeEntry = Readonly<{
  entityId: string
  entityType: string
  postImageJson: string
}>

class MobileChatSyncServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<
    SyncScope,
    { handlers: LiveSocketHandlers; open: boolean }
  >()
  readonly seenAuthTokens: Array<string> = []
  readonly pushedMutations: Array<MutationEnvelope> = []
  clientLastMutationId = 0
  rejectPushes = false

  logOf(scope: SyncScope): Array<ChangelogEntry> {
    const existing = this.logs.get(scope)
    if (existing !== undefined) return existing
    const created: Array<ChangelogEntry> = []
    this.logs.set(scope, created)
    return created
  }

  lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : log[log.length - 1]!.version
  }

  fold(scope: SyncScope): Array<FakeEntry> {
    const state = new Map<string, FakeEntry>()
    for (const entry of this.logOf(scope)) {
      const key = `${entry.entityType}/${entry.entityId}`
      if (entry.op === "upsert" && entry.postImageJson !== undefined) {
        state.set(key, {
          entityId: String(entry.entityId),
          entityType: String(entry.entityType),
          postImageJson: entry.postImageJson
        })
      } else {
        state.delete(key)
      }
    }
    return [...state.values()]
  }

  currentThread(threadId: string): ChatThreadEntity | null {
    const row = this
      .fold(personalScope(OWNER_ID))
      .find(entry =>
        entry.entityType === CHAT_THREAD_ENTITY_TYPE &&
        entry.entityId === threadId
      )
    return row === undefined
      ? null
      : decodeChatThreadEntity(JSON.parse(row.postImageJson) as unknown)
  }

  commit(scope: SyncScope, entries: ReadonlyArray<FakeEntry>): void {
    const version = SyncVersion.make(this.lastVersion(scope) + 1)
    const rows = entries.map(entry =>
      new ChangelogEntry({
        committedAt: FIXED_TIME,
        entityId: EntityId.make(entry.entityId),
        entityType: EntityType.make(entry.entityType),
        op: "upsert",
        postImageJson: entry.postImageJson,
        scope,
        version
      })
    )
    this.logOf(scope).push(...rows)
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open) {
      socket.handlers.onFrame({
        _tag: "DeltaFrame",
        cursor: version,
        entries: rows,
        scope
      } as never)
    }
  }

  commitThread(thread: ChatThreadEntity): void {
    const entry = {
      entityId: thread.threadId,
      entityType: CHAT_THREAD_ENTITY_TYPE,
      postImageJson: canonicalJson(encodeChatThreadEntity(thread))
    }
    this.commit(personalScope(thread.ownerUserId), [entry])
    this.commit(threadScope(thread.threadId), [entry])
  }

  commitMessage(message: ChatMessageEntity): void {
    this.commit(threadScope(message.threadId), [
      {
        entityId: message.messageId,
        entityType: CHAT_MESSAGE_ENTITY_TYPE,
        postImageJson: canonicalJson(encodeChatMessageEntity(message))
      }
    ])
  }

  bootstrap(request: { readonly scope: SyncScope }) {
    return new BootstrapResponse({
      cursor: SyncVersionWatermark.make(this.lastVersion(request.scope)),
      entities: this.fold(request.scope).map(entity =>
        new BootstrapEntity({
          entityId: EntityId.make(entity.entityId),
          entityType: EntityType.make(entity.entityType),
          postImageJson: entity.postImageJson
        })
      ),
      protocolVersion: 1,
      scope: request.scope
    })
  }

  logPage(scope: SyncScope, cursor: number) {
    const entries = this.logOf(scope).filter(entry => entry.version > cursor)
    const last = entries[entries.length - 1]
    return new LogPage({
      entries,
      nextCursor: SyncVersionWatermark.make(last === undefined ? cursor : last.version),
      protocolVersion: 1,
      scope,
      upToDate: true
    })
  }

  push(request: { readonly mutations: ReadonlyArray<MutationEnvelope> }) {
    const results: Array<MutationResult> = []
    let last = this.clientLastMutationId
    for (const mutation of request.mutations) {
      this.pushedMutations.push(mutation)
      if (mutation.mutationId <= last) {
        results.push(new MutationResult({ mutationId: mutation.mutationId, status: "duplicate" }))
        continue
      }
      last = mutation.mutationId
      if (this.rejectPushes) {
        results.push(new MutationResult({
          errorCode: "unauthorized_scope",
          errorMessageSafe: "this chat thread scope belongs to a different user",
          mutationId: mutation.mutationId,
          status: "rejected"
        }))
        continue
      }
      if (mutation.name === CHAT_CREATE_THREAD_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as { threadId: string; title: string }
        this.commitThread(decodeChatThreadEntity({
          createdAt: FIXED_TIME,
          lastMessageAt: null,
          messageCount: 0,
          ownerUserId: OWNER_ID,
          status: "active",
          threadId: args.threadId,
          title: args.title,
          updatedAt: FIXED_TIME
        }))
      }
      if (mutation.name === CHAT_APPEND_MESSAGE_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as {
          body: string
          messageId: string
          threadId: string
        }
        const thread = this.currentThread(args.threadId)
        if (thread === null) {
          results.push(new MutationResult({
            errorCode: "thread_not_found",
            errorMessageSafe: "this chat thread does not exist",
            mutationId: mutation.mutationId,
            status: "rejected"
          }))
          continue
        }
        this.commitThread(decodeChatThreadEntity({
          ...thread,
          lastMessageAt: FIXED_TIME,
          messageCount: thread.messageCount + 1,
          updatedAt: FIXED_TIME
        }))
        this.commitMessage(decodeChatMessageEntity({
          authorUserId: OWNER_ID,
          body: args.body,
          createdAt: FIXED_TIME,
          deletedAt: null,
          messageId: args.messageId,
          threadId: args.threadId,
          updatedAt: FIXED_TIME
        }))
      }
      results.push(new MutationResult({ mutationId: mutation.mutationId, status: "applied" }))
    }
    this.clientLastMutationId = last
    return new PushResponse({ lastMutationId: last, protocolVersion: 1, results })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    const record = { handlers, open: true }
    this.sockets.set(scope, record)
    return {
      close: () => {
        record.open = false
      }
    }
  }
}

const fakeTransport = (
  server: MobileChatSyncServer,
  authToken: () => string,
): KhalaSyncTransport => {
  const attempt = <A>(run: () => A): Effect.Effect<A, KhalaSyncTransportError> =>
    Effect.suspend(() => {
      server.seenAuthTokens.push(authToken())
      try {
        return Effect.succeed(run())
      } catch (error) {
        return Effect.fail(
          error instanceof KhalaSyncTransportError
            ? error
            : new KhalaSyncTransportError("network", true, String(error), { cause: error })
        )
      }
    })
  return {
    bootstrap: request => attempt(() => server.bootstrap(request)),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
    logPage: (scope, cursor) => attempt(() => server.logPage(scope, cursor)),
    push: request => attempt(() => server.push(request))
  }
}

describe("Khala mobile Sync runtime", () => {
  test("generates public-safe client-side message ids", () => {
    expect(
      makeKhalaMobileMessageId({
        now: () => new Date(FIXED_TIME),
        randomId: () => "abc123"
      })
    ).toBe("chat-message.mobile.1783195200000.abc123")
  })

  test("creates/appends chat through a real session and resumes from Expo SQLite cursor", async () => {
    const sqlite = expoSqliteFromBun()
    const server = new MobileChatSyncServer()
    const first = await openKhalaMobileSyncRuntime({
      databaseName: "mobile-sync",
      now: () => new Date(FIXED_TIME),
      ownerUserId: OWNER_ID,
      randomId: () => "client-a",
      secureTokenLoader: async () => "oa_agent_mobile_token",
      sleep: () => tick(),
      sqliteLoader: async () => sqlite,
      syncBaseUrl: "https://openagents.test",
      transport: config => fakeTransport(server, config.authToken)
    })
    expect(first.ok).toBe(true)
    if (!first.ok) return

    await expect(first.runtime.createThread({
      threadId: THREAD_ID,
      title: "Mobile sync"
    })).resolves.toEqual({ ok: true, threadId: THREAD_ID })
    await expect(first.runtime.appendMessage({
      body: "hello from mobile",
      messageId: "chat-message.mobile.test.1",
      threadId: THREAD_ID
    })).resolves.toEqual({
      ok: true,
      messageId: "chat-message.mobile.test.1",
      threadId: THREAD_ID
    })

    const messages = await first.runtime.chatMessages({ threadId: THREAD_ID })
    expect(messages.messages.map(message => [message.messageId, message.body])).toEqual([
      ["chat-message.mobile.test.1", "hello from mobile"]
    ])
    const threads = await first.runtime.chatThreads()
    expect(threads.threads[0]).toMatchObject({
      messageCount: 1,
      threadId: THREAD_ID,
      title: "Mobile sync"
    })
    await first.runtime.close()

    const reopened = await openKhalaMobileSyncRuntime({
      databaseName: "mobile-sync",
      now: () => new Date(FIXED_TIME),
      ownerUserId: OWNER_ID,
      randomId: () => "client-b",
      secureTokenLoader: async () => "oa_agent_mobile_token",
      sleep: () => tick(),
      sqliteLoader: async () => sqlite,
      syncBaseUrl: "https://openagents.test",
      transport: config => fakeTransport(server, config.authToken)
    })
    expect(reopened.ok).toBe(true)
    if (!reopened.ok) return

    const resumed = await reopened.runtime.chatMessages({ threadId: THREAD_ID })
    expect(resumed.cursor).not.toBeNull()
    expect(resumed.messages.map(message => message.messageId)).toEqual([
      "chat-message.mobile.test.1"
    ])
    expect(server.seenAuthTokens).not.toContain("")
    await reopened.runtime.close()
  })

  test("returns public-safe rejection state without retaining rejected bodies", async () => {
    const sqlite = expoSqliteFromBun()
    const server = new MobileChatSyncServer()
    const opened = await openKhalaMobileSyncRuntime({
      databaseName: "mobile-sync-rejection",
      now: () => new Date(FIXED_TIME),
      ownerUserId: OWNER_ID,
      randomId: () => "client-reject",
      secureTokenLoader: async () => "oa_agent_mobile_token",
      sleep: () => tick(),
      sqliteLoader: async () => sqlite,
      syncBaseUrl: "https://openagents.test",
      transport: config => fakeTransport(server, config.authToken)
    })
    expect(opened.ok).toBe(true)
    if (!opened.ok) return

    await opened.runtime.createThread({ threadId: THREAD_ID, title: "Rejected" })
    server.rejectPushes = true
    const rejected = await opened.runtime.appendMessage({
      body: "private rejected body",
      messageId: "chat-message.mobile.rejected",
      threadId: THREAD_ID
    })
    expect(rejected).toMatchObject({
      messageId: "chat-message.mobile.rejected",
      ok: false,
      threadId: THREAD_ID
    })
    expect(rejected.error).toContain("different user")

    const state = await opened.runtime.chatMessages({ threadId: THREAD_ID })
    expect(state.rejections[0]).toMatchObject({
      errorCode: "unauthorized_scope",
      threadId: THREAD_ID
    })
    expect(state.messages.some(message => message.body === "private rejected body")).toBe(false)
    await opened.runtime.close()
  })
})
