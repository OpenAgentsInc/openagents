import { Database } from "bun:sqlite"
import { describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  BootstrapEntity,
  BootstrapResponse,
  ChangelogEntry,
  DeltaFrame,
  EntityId,
  EntityType,
  LogPage,
  MutationResult,
  PushResponse,
  SyncVersion,
  SyncVersionWatermark,
  canonicalJson,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  encodeChatMessageEntity,
  encodeChatThreadEntity,
  personalScope,
  threadScope,
  type BootstrapRequest,
  type ChatThreadEntity,
  type LiveFrame,
  type MutationEnvelope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  type KhalaSyncTransport,
  type LiveSocketHandlers,
} from "@openagentsinc/khala-sync-client"
import {
  openExpoKhalaSyncStore,
  type ExpoSqliteDatabase,
} from "@openagentsinc/khala-sync-client/expo-sqlite-store"
import { Effect } from "effect"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"

import { openDesktopSyncHost } from "../src/desktop-sync-host.ts"
import {
  openDesktopSyncStore,
  type DesktopSqliteDatabase,
} from "../src/desktop-sync-store.ts"
import { openMobileSyncHostCore } from "../../openagents-mobile/src/sync/mobile-sync-host-core.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

const OWNER = "owner.cross-device"
const THREAD = "thread.cross-device"
const FIRST_MESSAGE = "message.desktop.1"
const FOLLOW_UP = "message.mobile.1"

const waitFor = async (
  condition: () => boolean,
  label: string,
): Promise<void> => {
  for (let attempt = 0; attempt < 3_000; attempt += 1) {
    if (condition()) return
    await new Promise(resolve => setTimeout(resolve, 0))
  }
  throw new Error(`timed out waiting for ${label}`)
}

type Socket = { handlers: LiveSocketHandlers; open: boolean }

class AuthoritativeChatServer {
  private readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  private readonly sockets = new Map<SyncScope, Array<Socket>>()
  private readonly lastMutation = new Map<string, number>()
  private readonly threads = new Map<string, ChatThreadEntity>()
  private readonly postImages = new Map<SyncScope, Map<string, BootstrapEntity>>()
  private clock = 0

  private now(): string {
    this.clock += 1
    return new Date(Date.UTC(2026, 6, 10, 20, 0, this.clock)).toISOString()
  }

  private log(scope: SyncScope): Array<ChangelogEntry> {
    const existing = this.logs.get(scope)
    if (existing !== undefined) return existing
    const created: Array<ChangelogEntry> = []
    this.logs.set(scope, created)
    return created
  }

  private commit(
    scope: SyncScope,
    entityType: string,
    entityId: string,
    postImageJson: string,
    mutationRef: string,
  ): void {
    const log = this.log(scope)
    const entry = new ChangelogEntry({
      scope,
      version: SyncVersion.make(log.length + 1),
      entityType: EntityType.make(entityType),
      entityId: EntityId.make(entityId),
      op: "upsert",
      postImageJson,
      mutationRef,
      committedAt: this.now(),
    })
    log.push(entry)
    const images = this.postImages.get(scope) ?? new Map<string, BootstrapEntity>()
    images.set(`${entityType}:${entityId}`, new BootstrapEntity({
      entityType: entry.entityType,
      entityId: entry.entityId,
      postImageJson,
    }))
    this.postImages.set(scope, images)
    for (const socket of this.sockets.get(scope) ?? []) {
      if (socket.open) {
        socket.handlers.onFrame(new DeltaFrame({
          scope,
          entries: [entry],
          cursor: entry.version,
        }) as LiveFrame)
      }
    }
  }

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    const log = this.log(request.scope)
    return new BootstrapResponse({
      protocolVersion: 1,
      scope: request.scope,
      entities: [...(this.postImages.get(request.scope)?.values() ?? [])],
      cursor: SyncVersionWatermark.make(log.length),
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    const entries = this.log(scope).filter(entry => Number(entry.version) > cursor).slice(0, limit)
    const next = entries.at(-1)?.version ?? SyncVersionWatermark.make(cursor)
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries,
      nextCursor: SyncVersionWatermark.make(Number(next)),
      upToDate: Number(next) >= this.log(scope).length,
    })
  }

  push(mutations: ReadonlyArray<MutationEnvelope>, clientKey: string): PushResponse {
    let last = this.lastMutation.get(clientKey) ?? 0
    const results: Array<MutationResult> = []
    for (const mutation of mutations) {
      if (Number(mutation.mutationId) <= last) {
        results.push(new MutationResult({ mutationId: mutation.mutationId, status: "duplicate" }))
        continue
      }
      const args = JSON.parse(mutation.argsJson) as Record<string, string>
      const mutationRef = `mutation:${clientKey}:${mutation.mutationId}`
      if (mutation.name === "chat.createThread") {
        const now = this.now()
        const thread = decodeChatThreadEntity({
          threadId: args.threadId!,
          ownerUserId: OWNER,
          title: args.title!.trim(),
          status: "active",
          messageCount: 0,
          lastMessageAt: null,
          createdAt: now,
          updatedAt: now,
        })
        this.threads.set(thread.threadId, thread)
        const postImage = canonicalJson(encodeChatThreadEntity(thread))
        this.commit(personalScope(OWNER), "chat_thread", thread.threadId, postImage, mutationRef)
        this.commit(threadScope(thread.threadId), "chat_thread", thread.threadId, postImage, mutationRef)
        results.push(new MutationResult({ mutationId: mutation.mutationId, status: "applied" }))
      } else if (mutation.name === "chat.appendMessage") {
        const current = this.threads.get(args.threadId!)
        if (current === undefined) {
          results.push(new MutationResult({
            mutationId: mutation.mutationId,
            status: "rejected",
            errorCode: "thread_not_found",
            errorMessageSafe: "thread not found",
          }))
        } else {
          const now = this.now()
          const updated = decodeChatThreadEntity({
            ...current,
            messageCount: current.messageCount + 1,
            lastMessageAt: now,
            updatedAt: now,
          })
          this.threads.set(updated.threadId, updated)
          const threadPostImage = canonicalJson(encodeChatThreadEntity(updated))
          this.commit(personalScope(OWNER), "chat_thread", updated.threadId, threadPostImage, mutationRef)
          this.commit(threadScope(updated.threadId), "chat_thread", updated.threadId, threadPostImage, mutationRef)
          this.commit(
            threadScope(updated.threadId),
            "chat_message",
            args.messageId!,
            canonicalJson(encodeChatMessageEntity(decodeChatMessageEntity({
              messageId: args.messageId!,
              threadId: updated.threadId,
              authorUserId: OWNER,
              body: args.body!,
              createdAt: now,
              updatedAt: now,
              deletedAt: null,
            }))),
            mutationRef,
          )
          results.push(new MutationResult({ mutationId: mutation.mutationId, status: "applied" }))
        }
      } else {
        results.push(new MutationResult({
          mutationId: mutation.mutationId,
          status: "rejected",
          errorCode: "mutation_rejected",
          errorMessageSafe: "unknown mutator",
        }))
      }
      last = Number(mutation.mutationId)
    }
    this.lastMutation.set(clientKey, last)
    return new PushResponse({ protocolVersion: 1, results, lastMutationId: last })
  }

  transport(): KhalaSyncTransport {
    return {
      bootstrap: request => Effect.sync(() => this.bootstrap(request)),
      logPage: (scope, cursor, limit) => Effect.sync(() => this.logPage(scope, cursor, limit)),
      push: request => Effect.sync(() => this.push(
        request.mutations,
        `${request.clientGroupId}:${request.clientId}`,
      )),
      connectLive: (scope, _cursor, handlers) => Effect.sync(() => {
        const socket: Socket = { handlers, open: true }
        this.sockets.set(scope, [...(this.sockets.get(scope) ?? []), socket])
        return { close: () => { socket.open = false } }
      }),
    }
  }
}

const openDesktopDatabase = (databasePath: string): DesktopSqliteDatabase => {
  const database = new Database(databasePath, { create: true })
  return {
    exec: sql => database.exec(sql),
    prepare: sql => {
      const statement = database.query(sql)
      return {
        run: (...params) => statement.run(...params),
        all: (...params) => statement.all(...params),
      }
    },
    close: () => database.close(),
  }
}

const openExpoDatabase = (databasePath: string): ExpoSqliteDatabase => {
  const database = new Database(databasePath, { create: true })
  return {
    execSync: sql => database.exec(sql),
    runSync: (sql, ...params) => database.query(sql).run(...params),
    getAllSync: <Row>(sql: string, ...params: ReadonlyArray<string | number>) =>
      database.query(sql).all(...params) as ReadonlyArray<Row>,
    withTransactionSync: task => database.transaction(task)(),
    closeSync: () => database.close(),
  }
}

describe("contract khala_sync.client.native_conversation_continuity.v1", () => {
  test("registers the Desktop-owned authoritative conversation contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(
      contract => contract.contractId === "openagents_desktop.sync.native_conversation_continuity.v1",
    )?.state).toBe("enforced")
  })

  test("Desktop starts, mobile continues, both converge and reconstruct after restart", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "openagents-native-conversation-"))
    const desktopPath = path.join(root, "desktop", "sync.sqlite")
    const mobilePath = path.join(root, "mobile.sqlite")
    const server = new AuthoritativeChatServer()
    const openDesktop = () => openDesktopSyncHost({
      databasePath: desktopPath,
      randomId: () => "desktop",
      openStore: file => openDesktopSyncStore(file, openDesktopDatabase),
    })
    const openMobile = () => openMobileSyncHostCore({
      databaseName: mobilePath,
      randomId: () => "mobile",
      openStore: file => openExpoKhalaSyncStore(file, openExpoDatabase),
    })
    const connect = (host: ReturnType<typeof openDesktop> | ReturnType<typeof openMobile>) =>
      host.connectAuthenticated({
        verification:"server_verified",
        baseUrl: "https://openagents.test",
        ownerUserId: OWNER,
        authToken: () => "verified-test-token",
        createTransport: () => server.transport(),
        sessionOptions: {
          sleep: () => Promise.resolve(),
          random: () => 0,
        },
        now: () => "2026-07-10T20:00:00.000Z",
      })

    let desktop = openDesktop()
    let mobile = openMobile()
    try {
      connect(desktop)
      connect(mobile)
      await waitFor(
        () => desktop.status().syncPhase === "live" && mobile.status().syncPhase === "live",
        "both personal scopes live",
      )
      const desktopChat = desktop.conversation()!
      const mobileChat = mobile.conversation()!

      Effect.runSync(desktopChat.createThread({ threadId: THREAD, title: "Cross-device proof" }))
      await waitFor(
        () => Effect.runSync(mobileChat.listConfirmedThreads())[0]?.threadRef === THREAD,
        "mobile confirms Desktop thread",
      )
      await Effect.runPromise(desktopChat.openThread(THREAD))
      await Effect.runPromise(mobileChat.openThread(THREAD))
      await waitFor(
        () => desktopChat.threadStatus(THREAD).phase === "live" &&
          mobileChat.threadStatus(THREAD).phase === "live",
        "both thread scopes live",
      )

      Effect.runSync(desktopChat.appendMessage({
        threadId: THREAD,
        messageId: FIRST_MESSAGE,
        body: "Started on Desktop",
      }))
      await waitFor(
        () => Effect.runSync(mobileChat.listConfirmedMessages(THREAD)).length === 1,
        "mobile confirms Desktop message",
      )
      Effect.runSync(mobileChat.appendMessage({
        threadId: THREAD,
        messageId: FOLLOW_UP,
        body: "Continued on mobile",
      }))
      await waitFor(
        () => Effect.runSync(desktopChat.listConfirmedMessages(THREAD)).length === 2,
        "Desktop confirms mobile follow-up",
      )

      const desktopMessages = Effect.runSync(desktopChat.listConfirmedMessages(THREAD))
      const mobileMessages = Effect.runSync(mobileChat.listConfirmedMessages(THREAD))
      expect(desktopMessages).toEqual(mobileMessages)
      expect(desktopMessages.map(message => [message.messageRef, message.body])).toEqual([
        [FIRST_MESSAGE, "Started on Desktop"],
        [FOLLOW_UP, "Continued on mobile"],
      ])
      expect(desktopChat.threadStatus(THREAD)).toMatchObject({ phase: "live", cursor: 5 })
      expect(mobileChat.threadStatus(THREAD)).toMatchObject({ phase: "live", cursor: 5 })
      expect(desktopMessages.map(message => message.version)).toEqual([3, 5])
      expect(JSON.stringify(desktopMessages)).not.toContain(OWNER)

      desktop.close()
      mobile.close()
      desktop = openDesktop()
      mobile = openMobile()
      connect(desktop)
      connect(mobile)
      await waitFor(
        () => desktop.status().syncPhase === "live" && mobile.status().syncPhase === "live",
        "both personal scopes live after restart",
      )
      await Effect.runPromise(desktop.conversation()!.openThread(THREAD))
      await Effect.runPromise(mobile.conversation()!.openThread(THREAD))
      await waitFor(
        () => desktop.conversation()!.threadStatus(THREAD).phase === "live" &&
          mobile.conversation()!.threadStatus(THREAD).phase === "live",
        "both thread scopes live after restart",
      )
      expect(Effect.runSync(desktop.conversation()!.listConfirmedMessages(THREAD))).toEqual(desktopMessages)
      expect(Effect.runSync(mobile.conversation()!.listConfirmedMessages(THREAD))).toEqual(mobileMessages)
    } finally {
      desktop.close()
      mobile.close()
      rmSync(root, { recursive: true, force: true })
    }
  })
})
