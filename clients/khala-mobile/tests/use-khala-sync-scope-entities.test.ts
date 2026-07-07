import { Database, type SQLQueryBindings } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import * as React from "react"
import { act, create as createTestRenderer } from "react-test-renderer"
import {
  BootstrapEntity,
  BootstrapResponse,
  CHAT_MESSAGE_ENTITY_TYPE,
  canonicalJson,
  ChangelogEntry,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  EntityId,
  EntityType,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  LogPage,
  MutationResult,
  personalScope,
  PushResponse,
  SyncVersion,
  SyncVersionWatermark,
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
import { Effect } from "effect"

import {
  openKhalaMobileSyncRuntime,
  type KhalaMobileSyncRuntime
} from "../src/sync/khala-mobile-sync-runtime"
import type {
  ExpoSqliteDatabase,
  ExpoSqliteModule
} from "../src/sync/expo-db-sqlite-persistence"
import {
  useKhalaSyncScopeEntities,
  type KhalaSyncScopeEntitiesState
} from "../src/sync/use-khala-sync-scope-entities"

/**
 * Proves the fix for the owner report "every time i open a new thread
 * session in the app it loads the messages from scratch": a REOPENED thread
 * scope (same on-device Expo SQLite store as a prior app session) renders
 * its cached messages immediately, via `useKhalaSyncScopeEntities`, without
 * a second full-history bootstrap — and a message committed on the server
 * after the reopen still arrives as a delta.
 *
 * Shares the same fake Expo SQLite + fake Khala Sync transport shape as
 * `khala-mobile-sync-runtime.test.ts` (bun:sqlite standing in for the real
 * native module, same wire fixtures), trimmed to just chat_thread/
 * chat_message so this file stays a focused, self-contained fixture.
 */

const OWNER_ID = "user.mobile.scope-entities"
const THREAD_ID = "thread.mobile.scope-entities"
const FIXED_TIME = "2026-07-05T12:00:00.000Z"

const flush = async (ticks = 20): Promise<void> => {
  for (let i = 0; i < ticks; i++) await Promise.resolve()
}

const expoSqliteFromBun = (): { module: ExpoSqliteModule; databases: Map<string, Database> } => {
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
    databases,
    module: { openDatabaseAsync: async name => open(name) }
  }
}

type FakeEntry = Readonly<{ entityId: string; entityType: string; postImageJson: string }>

class ScopeEntitiesFakeServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<SyncScope, { handlers: LiveSocketHandlers; open: boolean }>()
  bootstrapCallsByScope = new Map<SyncScope, number>()
  logPageCallsByScope = new Map<SyncScope, number>()
  clientLastMutationId = 0

  private logOf(scope: SyncScope): Array<ChangelogEntry> {
    const existing = this.logs.get(scope)
    if (existing !== undefined) return existing
    const created: Array<ChangelogEntry> = []
    this.logs.set(scope, created)
    return created
  }

  private lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : log[log.length - 1]!.version
  }

  private fold(scope: SyncScope): Array<FakeEntry> {
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

  commit(scope: SyncScope, entries: ReadonlyArray<FakeEntry>): void {
    const version = SyncVersion.make(this.lastVersion(scope) + 1)
    const rows = entries.map(
      entry =>
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
      socket.handlers.onFrame({ _tag: "DeltaFrame", cursor: version, entries: rows, scope } as never)
    }
  }

  commitThread(thread: ChatThreadEntity): void {
    const entry = {
      entityId: thread.threadId,
      entityType: "chat_thread",
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
    this.bootstrapCallsByScope.set(request.scope, (this.bootstrapCallsByScope.get(request.scope) ?? 0) + 1)
    return new BootstrapResponse({
      cursor: SyncVersionWatermark.make(this.lastVersion(request.scope)),
      entities: this.fold(request.scope).map(
        entity =>
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
    this.logPageCallsByScope.set(scope, (this.logPageCallsByScope.get(scope) ?? 0) + 1)
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
      if (mutation.mutationId <= last) {
        results.push(new MutationResult({ mutationId: mutation.mutationId, status: "duplicate" }))
        continue
      }
      last = mutation.mutationId
      if (mutation.name === "chat.createThread") {
        const args = JSON.parse(mutation.argsJson) as { threadId: string; title: string }
        this.commitThread(
          decodeChatThreadEntity({
            createdAt: FIXED_TIME,
            lastMessageAt: null,
            messageCount: 0,
            ownerUserId: OWNER_ID,
            status: "active",
            threadId: args.threadId,
            title: args.title,
            updatedAt: FIXED_TIME
          })
        )
      }
      if (mutation.name === "chat.appendMessage") {
        const args = JSON.parse(mutation.argsJson) as { body: string; messageId: string; threadId: string }
        this.commitMessage(
          decodeChatMessageEntity({
            authorUserId: OWNER_ID,
            body: args.body,
            createdAt: FIXED_TIME,
            deletedAt: null,
            messageId: args.messageId,
            threadId: args.threadId,
            updatedAt: FIXED_TIME
          })
        )
      }
      results.push(new MutationResult({ mutationId: mutation.mutationId, status: "applied" }))
    }
    this.clientLastMutationId = last
    return new PushResponse({ lastMutationId: last, protocolVersion: 1, results })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    const record = { handlers, open: true }
    this.sockets.set(scope, record)
    return { close: () => { record.open = false } }
  }
}

const fakeTransport = (server: ScopeEntitiesFakeServer): KhalaSyncTransport => {
  const attempt = <A>(run: () => A): Effect.Effect<A, KhalaSyncTransportError> =>
    Effect.suspend(() => {
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
    connectLive: (scope, cursor, handlers) => attempt(() => server.connect(scope, cursor, handlers)),
    logPage: (scope, cursor) => attempt(() => server.logPage(scope, cursor)),
    push: request => attempt(() => server.push(request))
  }
}

const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

/** Test-only "renderHook": mounts a host component that stashes every
 * render's hook output onto `states`, so assertions can inspect the FULL
 * history (not just the latest value) without a dedicated hook-testing
 * library — this repo only has `react-test-renderer` as a devDependency. */
const mountScopeEntities = async (
  scope: string,
  entityType: string,
  runtime: Pick<KhalaMobileSyncRuntime, "overlay" | "session" | "store">
): Promise<{ states: Array<KhalaSyncScopeEntitiesState<ChatMessageEntity>> }> => {
  const states: Array<KhalaSyncScopeEntitiesState<ChatMessageEntity>> = []
  const Harness = () => {
    const state = useKhalaSyncScopeEntities<ChatMessageEntity>({
      decode: value => decodeChatMessageEntity(value),
      entityType,
      overlay: runtime.overlay,
      scope,
      session: runtime.session,
      store: runtime.store
    })
    states.push(state)
    return null
  }
  await act(async () => {
    createTestRenderer(React.createElement(Harness))
  })
  return { states }
}

describe("useKhalaSyncScopeEntities (thread revisit local-first cache)", () => {
  test("a revisited thread renders cached messages instantly, without a second bootstrap, and still receives new deltas", async () => {
    const sqlite = expoSqliteFromBun()
    const server = new ScopeEntitiesFakeServer()
    const scope = String(threadScope(THREAD_ID))

    // --- "First app session": create the thread, send one message. This is
    // the only bootstrap this thread scope should ever need.
    const first = await openKhalaMobileSyncRuntime({
      databaseName: "scope-entities-sync",
      now: () => new Date(FIXED_TIME),
      ownerUserId: OWNER_ID,
      randomId: () => "client-a",
      secureTokenLoader: async () => "oa_agent_token",
      sleep: () => tick(),
      sqliteLoader: async () => sqlite.module,
      syncBaseUrl: "https://openagents.test",
      transport: () => fakeTransport(server)
    })
    expect(first.ok).toBe(true)
    if (!first.ok) return

    await first.runtime.createThread({ threadId: THREAD_ID, title: "Revisit test" })
    await first.runtime.appendMessage({
      body: "message from before you closed the app",
      messageId: "chat-message.before-reopen",
      threadId: THREAD_ID
    })
    expect(server.bootstrapCallsByScope.get(scope as never)).toBe(1)

    await act(async () => {
      await first.runtime.close()
    })

    // --- "Reopen the app": brand-new runtime instance, but the SAME
    // on-device SQLite database (Expo SQLite persists across app restarts)
    // — this is exactly what "every time i open a new thread session"
    // means: reopening a thread that was already synced once before.
    const reopened = await openKhalaMobileSyncRuntime({
      databaseName: "scope-entities-sync",
      now: () => new Date(FIXED_TIME),
      ownerUserId: OWNER_ID,
      randomId: () => "client-b",
      secureTokenLoader: async () => "oa_agent_token",
      sleep: () => tick(),
      sqliteLoader: async () => sqlite.module,
      syncBaseUrl: "https://openagents.test",
      transport: () => fakeTransport(server)
    })
    expect(reopened.ok).toBe(true)
    if (!reopened.ok) return

    const { states } = await mountScopeEntities(scope, CHAT_MESSAGE_ENTITY_TYPE, reopened.runtime)

    // (a) CACHE-FIRST: the very first flush already shows the message that
    // was synced in the prior "session" — read straight from the durable
    // local store, well before any new network bootstrap could resolve.
    await act(async () => {
      await flush(1)
    })
    const firstRender = states[states.length - 1]!
    expect(firstRender.status).toBe("ready")
    expect(firstRender.items.map(message => message.messageId)).toEqual(["chat-message.before-reopen"])

    await act(async () => {
      await flush()
    })

    // (b) NO SECOND BOOTSTRAP: reopening a previously-visited thread must
    // resume from the durable cursor (catch-up / live-tail only), never
    // re-fetch the whole history — this is the literal bug being fixed.
    expect(server.bootstrapCallsByScope.get(scope as never)).toBe(1)
    expect(server.logPageCallsByScope.get(scope as never) ?? 0).toBeGreaterThanOrEqual(1)

    // (c) DELTA SYNC STILL WORKS: a message committed on the server AFTER
    // the reopen (e.g. sent from another device) still arrives live.
    server.commitMessage(
      decodeChatMessageEntity({
        authorUserId: OWNER_ID,
        body: "message sent after you reopened the app",
        createdAt: FIXED_TIME,
        deletedAt: null,
        messageId: "chat-message.after-reopen",
        threadId: THREAD_ID,
        updatedAt: FIXED_TIME
      })
    )
    await act(async () => {
      await flush()
    })
    const latest = states[states.length - 1]!
    expect(latest.status).toBe("ready")
    expect(latest.error).toBeNull()
    // (d) NO CORRECTNESS REGRESSION: the old cached message is not
    // clobbered/lost by the new delta — both are present.
    expect(new Set(latest.items.map(message => message.messageId))).toEqual(
      new Set(["chat-message.before-reopen", "chat-message.after-reopen"])
    )

    await act(async () => {
      await reopened.runtime.close()
    })
  })
})

/**
 * Regression for "sending a message does nothing" (2026-07-07): the user's
 * own just-sent message must show IMMEDIATELY, before the server confirms it.
 * This is the optimistic-overlay path — a store-only read (confirmed rows
 * only) would show nothing here, which was the exact bug. The hook must read
 * through the overlay (confirmed base + pending optimistic effects).
 *
 * Oracle for khala_mobile.chat.optimistic_message_renders_on_send.v1
 */
describe("useKhalaSyncScopeEntities (optimistic append visible before server confirmation)", () => {
  test("an optimistic overlay row absent from the confirmed store is still surfaced by the hook", async () => {
    const scope = String(threadScope(THREAD_ID))
    const optimisticMessage = {
      authorUserId: OWNER_ID,
      body: "optimistic hello",
      createdAt: FIXED_TIME,
      deletedAt: null,
      messageId: "chat-message.optimistic",
      threadId: THREAD_ID,
      updatedAt: FIXED_TIME,
    }
    const optimisticEntity = {
      entityId: optimisticMessage.messageId,
      entityType: CHAT_MESSAGE_ENTITY_TYPE,
      postImageJson: JSON.stringify(optimisticMessage),
    }

    // The confirmed store is EMPTY (the message hasn't been confirmed by the
    // server yet); the pending optimistic effect lives ONLY in the overlay.
    // A store-only read would show an empty thread — the exact "sending does
    // nothing" bug. Reading the overlay, the hook must surface the row.
    const session = {
      state: () => ({ phase: "live" as const }),
      subscribe: () => Effect.succeed(undefined),
      subscribeState: () => () => undefined,
      unsubscribe: () => Effect.succeed(undefined),
    }
    const store = {
      readEntities: () => Effect.succeed([]),
    }
    const overlay = {
      read: () =>
        Effect.succeed({
          get: () => undefined,
          list: (entityType: string) =>
            entityType === CHAT_MESSAGE_ENTITY_TYPE ? [optimisticEntity] : [],
        }),
      subscribe: () => () => undefined,
    }

    const { states } = await mountScopeEntities(scope, CHAT_MESSAGE_ENTITY_TYPE, {
      overlay: overlay as never,
      session: session as never,
      store: store as never,
    })

    await act(async () => {
      await flush()
    })

    const latest = states[states.length - 1]!
    expect(latest.items.map(message => message.messageId)).toEqual(["chat-message.optimistic"])
    expect(latest.status).toBe("ready")
  })
})
