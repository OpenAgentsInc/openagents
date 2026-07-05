import { describe, expect, test } from "bun:test"
import {
  BootstrapEntity,
  BootstrapResponse,
  type BootstrapRequest,
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
  fleetRunScope,
  LogPage,
  MustRefetchFrame,
  MutationResult,
  PushResponse,
  SyncVersion,
  SyncVersionWatermark,
  type MutationEnvelope,
  type SyncScope,
  personalScope,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import {
  KhalaSyncTransportError,
  type KhalaSyncTransport,
  type LiveSocketHandlers,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  CHAT_RENAME_THREAD_MUTATOR_NAME,
} from "@openagentsinc/khala-sync-db-collection"
import {
  createKhalaCodeDesktopKhalaSyncService,
  FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME,
  FLEET_PAUSE_RUN_MUTATOR_NAME,
  FLEET_PAUSE_WORKER_MUTATOR_NAME,
  FLEET_RESUME_WORKER_MUTATOR_NAME,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  FLEET_STOP_RUN_MUTATOR_NAME,
  khalaCodeDesktopKhalaSyncFleetEnabled,
  khalaSyncFleetDisabledState,
} from "../src/bun/khala-sync-service"

/**
 * KS-6.2 (#8303) service tests: the desktop Khala Sync fleet consumer over
 * a deterministic in-memory fake transport (the khala-sync-client test
 * fakes are not exported, so this file carries a scoped local fake that
 * implements the SPEC §3 semantics the service exercises). No network, no
 * real sockets; timing is injected so backoff paths run instantly.
 */

const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

const waitFor = async (
  condition: () => boolean | Promise<boolean>,
  label: string,
  ticks = 3000,
): Promise<void> => {
  for (let index = 0; index < ticks; index++) {
    if (await condition()) return
    await tick()
  }
  throw new Error(`timed out waiting for: ${label}`)
}

const RUN_ID = "fleet_run.test.1"
const SCOPE = fleetRunScope(RUN_ID)
const FIXED_TIME = "2026-07-04T00:00:00.000Z"

const fleetRunImage = (patch: Record<string, unknown> = {}): string =>
  canonicalJson({
    counters: {
      activeAssignments: 1,
      blockedAssignments: 0,
      completedAssignments: 2,
      failedAssignments: 0,
      workUnitsTotal: 5,
    },
    desiredSlots: 3,
    runId: RUN_ID,
    startedAt: FIXED_TIME,
    status: "running",
    updatedAt: FIXED_TIME,
    workerKind: "codex",
    ...patch,
  })

const workerImage = canonicalJson({
  accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
  phase: "dispatched",
  updatedAt: FIXED_TIME,
  workerId: "worker.slot.1",
})

const assignmentImage = canonicalJson({
  assignmentRef: "assignment.test.1",
  issueRef: "#8303",
  status: "running",
  updatedAt: FIXED_TIME,
})

const accountImage = canonicalJson({
  accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
  readiness: "ready",
  updatedAt: FIXED_TIME,
})

interface FakeEntry {
  readonly entityType: string
  readonly entityId: string
  readonly postImageJson: string
}

/** Minimal SPEC §3 fake server for the fleet scope. */
class FakeFleetSyncServer {
  readonly log: Array<ChangelogEntry> = []
  socket: { handlers: LiveSocketHandlers; open: boolean } | null = null
  /** Mutator behavior switch: reject every push in-band (unauthorized_scope). */
  rejectPushes = false
  /** Hold pushes (network fault) so optimistic state stays unconfirmed. */
  holdPushes = false
  readonly seenAuthTokens: Array<string> = []
  readonly pushedMutations: Array<MutationEnvelope> = []
  clientLastMutationId = 0

  lastVersion(): number {
    return this.log.length === 0 ? 0 : this.log[this.log.length - 1]!.version
  }

  commit(entries: ReadonlyArray<FakeEntry>): void {
    const version = SyncVersion.make(this.lastVersion() + 1)
    const rows = entries.map(
      entry =>
        new ChangelogEntry({
          scope: SCOPE,
          version,
          entityType: EntityType.make(entry.entityType),
          entityId: EntityId.make(entry.entityId),
          op: "upsert",
          postImageJson: entry.postImageJson,
          committedAt: FIXED_TIME,
        }),
    )
    this.log.push(...rows)
    if (this.socket !== null && this.socket.open) {
      this.socket.handlers.onFrame({
        _tag: "DeltaFrame",
        scope: SCOPE,
        entries: rows,
        cursor: version,
      } as never)
    }
  }

  replaceScope(entries: ReadonlyArray<FakeEntry>): void {
    this.log.length = 0
    this.commit(entries)
  }

  emitMustRefetch(): void {
    if (this.socket !== null && this.socket.open) {
      this.socket.handlers.onFrame(
        new MustRefetchFrame({ scope: SCOPE, reason: "scope_reset" }) as never,
      )
    }
  }

  currentEntities(): Array<FakeEntry> {
    const state = new Map<string, FakeEntry>()
    for (const entry of this.log) {
      state.set(`${entry.entityType}/${entry.entityId}`, {
        entityType: entry.entityType,
        entityId: entry.entityId,
        postImageJson: entry.postImageJson!,
      })
    }
    return [...state.values()]
  }
}

const fakeTransport = (
  server: FakeFleetSyncServer,
  authToken: () => string,
): KhalaSyncTransport => ({
  bootstrap: (request: BootstrapRequest) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      return new BootstrapResponse({
        protocolVersion: 1,
        scope: request.scope,
        entities: server
          .currentEntities()
          .map(
            entity =>
              new BootstrapEntity({
                entityType: EntityType.make(entity.entityType),
                entityId: EntityId.make(entity.entityId),
                postImageJson: entity.postImageJson,
              }),
          ),
        cursor: SyncVersionWatermark.make(server.lastVersion()),
      })
    }),
  logPage: (scope: SyncScope, cursor) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      const entries = server.log.filter(entry => entry.version > cursor)
      const next = entries.length === 0
        ? cursor
        : entries[entries.length - 1]!.version
      return new LogPage({
        protocolVersion: 1,
        scope,
        entries,
        nextCursor: SyncVersionWatermark.make(next),
        upToDate: true,
      })
    }),
  push: request =>
    Effect.suspend(() => {
      server.seenAuthTokens.push(authToken())
      if (server.holdPushes) {
        return Effect.fail(
          new KhalaSyncTransportError("network", true, "fake offline push"),
        )
      }
      const results: Array<MutationResult> = []
      let last = server.clientLastMutationId
      for (const mutation of request.mutations) {
        server.pushedMutations.push(mutation)
        if (mutation.mutationId <= last) {
          results.push(
            new MutationResult({ mutationId: mutation.mutationId, status: "duplicate" }),
          )
          continue
        }
        last = mutation.mutationId
        if (server.rejectPushes) {
          results.push(
            new MutationResult({
              errorCode: "unauthorized_scope",
              errorMessageSafe: "this fleet run scope belongs to a different user",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        const args = JSON.parse(mutation.argsJson) as {
          runId: string
          desiredSlots?: number
          workerId?: string
          flagRef?: string
          confirm?: boolean
        }
        const currentOf = (entityType: string, entityId: string) =>
          server
            .currentEntities()
            .find(entity => entity.entityType === entityType && entity.entityId === entityId)
        if (
          mutation.name === FLEET_PAUSE_WORKER_MUTATOR_NAME ||
          mutation.name === FLEET_RESUME_WORKER_MUTATOR_NAME
        ) {
          const workerId = args.workerId!
          const current = currentOf("fleet_worker", workerId)
          const base = current === undefined
            ? { updatedAt: FIXED_TIME, workerId }
            : (JSON.parse(current.postImageJson) as Record<string, unknown>)
          server.commit([
            {
              entityType: "fleet_worker",
              entityId: workerId,
              postImageJson: canonicalJson({
                ...base,
                phase: mutation.name === FLEET_PAUSE_WORKER_MUTATOR_NAME ? "paused" : "idle",
              }),
            },
          ])
        } else if (mutation.name === FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME) {
          const flagRef = args.flagRef!
          const current = currentOf("fleet_inbox_flag", flagRef)
          const base = current === undefined
            ? { flagRef, kind: "unclassified", updatedAt: FIXED_TIME }
            : (JSON.parse(current.postImageJson) as Record<string, unknown>)
          server.commit([
            {
              entityType: "fleet_inbox_flag",
              entityId: flagRef,
              postImageJson: canonicalJson({
                ...base,
                acknowledgedAt: FIXED_TIME,
                status: "acknowledged",
              }),
            },
          ])
        } else {
          const current = currentOf("fleet_run", args.runId)
          const base = current === undefined
            ? (JSON.parse(fleetRunImage()) as Record<string, unknown>)
            : (JSON.parse(current.postImageJson) as Record<string, unknown>)
          const patch: Record<string, unknown> =
            mutation.name === FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME
              ? { desiredSlots: args.desiredSlots }
              : mutation.name === FLEET_STOP_RUN_MUTATOR_NAME
                ? { desiredSlots: 0, status: "stopped" }
                : mutation.name === FLEET_PAUSE_RUN_MUTATOR_NAME
                  ? { status: "paused" }
                  : { status: "running" }
          server.commit([
            {
              entityType: "fleet_run",
              entityId: args.runId,
              postImageJson: canonicalJson({ ...base, ...patch }),
            },
          ])
        }
        results.push(
          new MutationResult({ mutationId: mutation.mutationId, status: "applied" }),
        )
      }
      server.clientLastMutationId = last
      return Effect.succeed(
        new PushResponse({ protocolVersion: 1, results, lastMutationId: last }),
      )
    }),
  connectLive: (_scope, _cursor, handlers) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      const record = { handlers, open: true }
      server.socket = record
      return {
        close: () => {
          record.open = false
        },
      }
    }),
})

type ServiceHarness = {
  readonly server: FakeFleetSyncServer
  readonly service: ReturnType<typeof createKhalaCodeDesktopKhalaSyncService>
  readonly env: Record<string, string | undefined>
}

const makeHarness = (
  envOverrides: Record<string, string | undefined> = {},
): ServiceHarness => {
  const server = new FakeFleetSyncServer()
  const env: Record<string, string | undefined> = {
    OPENAGENTS_AGENT_TOKEN: "oa_agent_test_token_1",
    OPENAGENTS_BASE_URL: "https://openagents.test",
    KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: "/tmp/khala-sync-service-test-missing-settings.json",
    ...envOverrides,
  }
  const service = createKhalaCodeDesktopKhalaSyncService({
    env,
    storePath: ":memory:",
    sleep: () => tick(),
    random: () => 0.5,
    now: () => new Date(FIXED_TIME),
    transport: config => fakeTransport(server, config.authToken),
  })
  return { server, service, env }
}

const CHAT_OWNER_ID = "user-chat-owner"
const CHAT_THREAD_ID = "thread.remote.test"
const CHAT_MESSAGE_ID = "chat-message.remote.test.1"

class FakeChatSyncServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  readonly sockets = new Map<SyncScope, { handlers: LiveSocketHandlers; open: boolean }>()
  readonly seenAuthTokens: Array<string> = []
  readonly seenScopes: Array<SyncScope> = []
  readonly pushedMutations: Array<MutationEnvelope> = []
  clientLastMutationId = 0
  holdPushes = false
  rejectPushes = false

  logOf(scope: SyncScope): Array<ChangelogEntry> {
    let log = this.logs.get(scope)
    if (log === undefined) {
      log = []
      this.logs.set(scope, log)
    }
    return log
  }

  lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : log[log.length - 1]!.version
  }

  fold(scope: SyncScope): Array<FakeEntry> {
    const state = new Map<string, FakeEntry>()
    for (const entry of this.logOf(scope)) {
      const key = `${entry.entityType}/${entry.entityId}`
      if (entry.op === "delete") state.delete(key)
      else {
        state.set(key, {
          entityId: entry.entityId,
          entityType: entry.entityType,
          postImageJson: entry.postImageJson!,
        })
      }
    }
    return [...state.values()]
  }

  currentThread(threadId: string): ChatThreadEntity | null {
    const row = this
      .fold(personalScope(CHAT_OWNER_ID))
      .find(entity =>
        entity.entityType === CHAT_THREAD_ENTITY_TYPE &&
        entity.entityId === threadId
      )
    return row === undefined
      ? null
      : decodeChatThreadEntity(JSON.parse(row.postImageJson) as unknown)
  }

  currentMessage(threadId: string, messageId: string): ChatMessageEntity | null {
    const row = this
      .fold(threadScope(threadId))
      .find(entity =>
        entity.entityType === CHAT_MESSAGE_ENTITY_TYPE &&
        entity.entityId === messageId
      )
    return row === undefined
      ? null
      : decodeChatMessageEntity(JSON.parse(row.postImageJson) as unknown)
  }

  commit(scope: SyncScope, entries: ReadonlyArray<FakeEntry>): void {
    const version = SyncVersion.make(this.lastVersion(scope) + 1)
    const rows = entries.map(
      entry =>
        new ChangelogEntry({
          scope,
          version,
          entityType: EntityType.make(entry.entityType),
          entityId: EntityId.make(entry.entityId),
          op: "upsert",
          postImageJson: entry.postImageJson,
          committedAt: FIXED_TIME,
        }),
    )
    this.logOf(scope).push(...rows)
    const socket = this.sockets.get(scope)
    if (socket !== undefined && socket.open) {
      socket.handlers.onFrame({
        _tag: "DeltaFrame",
        scope,
        entries: rows,
        cursor: version,
      } as never)
    }
  }

  commitThread(thread: ChatThreadEntity): void {
    const entry = {
      entityType: CHAT_THREAD_ENTITY_TYPE,
      entityId: thread.threadId,
      postImageJson: canonicalJson(encodeChatThreadEntity(thread)),
    }
    this.commit(personalScope(thread.ownerUserId), [entry])
    this.commit(threadScope(thread.threadId), [entry])
  }

  commitMessage(message: ChatMessageEntity): void {
    this.commit(threadScope(message.threadId), [
      {
        entityType: CHAT_MESSAGE_ENTITY_TYPE,
        entityId: message.messageId,
        postImageJson: canonicalJson(encodeChatMessageEntity(message)),
      },
    ])
  }

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    this.seenScopes.push(request.scope)
    const cursor = this.lastVersion(request.scope)
    return new BootstrapResponse({
      protocolVersion: 1,
      scope: request.scope,
      entities: this.fold(request.scope).map(entity =>
        new BootstrapEntity({
          entityType: EntityType.make(entity.entityType),
          entityId: EntityId.make(entity.entityId),
          postImageJson: entity.postImageJson,
        })
      ),
      cursor: SyncVersionWatermark.make(cursor),
    })
  }

  logPage(scope: SyncScope, cursor: number): LogPage {
    this.seenScopes.push(scope)
    const entries = this.logOf(scope).filter(entry => entry.version > cursor)
    const last = entries[entries.length - 1]
    const next = last === undefined ? cursor : last.version
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries,
      nextCursor: SyncVersionWatermark.make(next),
      upToDate: true,
    })
  }

  push(request: { readonly mutations: readonly MutationEnvelope[] }): PushResponse {
    if (this.holdPushes) {
      throw new KhalaSyncTransportError("network", true, "fake offline push")
    }
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
          status: "rejected",
        }))
        continue
      }
      if (mutation.name === CHAT_CREATE_THREAD_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as { threadId: string; title: string }
        this.commitThread(decodeChatThreadEntity({
          createdAt: FIXED_TIME,
          lastMessageAt: null,
          messageCount: 0,
          ownerUserId: CHAT_OWNER_ID,
          status: "active",
          threadId: args.threadId,
          title: args.title.trim(),
          updatedAt: FIXED_TIME,
        }))
      } else if (mutation.name === CHAT_APPEND_MESSAGE_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as {
          body: string
          messageId: string
          threadId: string
        }
        const current = this.currentThread(args.threadId)
        if (current === null) {
          results.push(new MutationResult({
            errorCode: "thread_not_found",
            errorMessageSafe: "this chat thread does not exist",
            mutationId: mutation.mutationId,
            status: "rejected",
          }))
          continue
        }
        if (this.currentMessage(args.threadId, args.messageId) !== null) {
          results.push(new MutationResult({
            errorCode: "message_exists",
            errorMessageSafe: "this chat message already exists",
            mutationId: mutation.mutationId,
            status: "rejected",
          }))
          continue
        }
        this.commitThread(decodeChatThreadEntity({
          ...current,
          lastMessageAt: FIXED_TIME,
          messageCount: current.messageCount + 1,
          updatedAt: FIXED_TIME,
        }))
        this.commitMessage(decodeChatMessageEntity({
          authorUserId: CHAT_OWNER_ID,
          body: args.body,
          createdAt: FIXED_TIME,
          deletedAt: null,
          messageId: args.messageId,
          threadId: args.threadId,
          updatedAt: FIXED_TIME,
        }))
      } else if (mutation.name === CHAT_RENAME_THREAD_MUTATOR_NAME) {
        const args = JSON.parse(mutation.argsJson) as { threadId: string; title: string }
        const current = this.currentThread(args.threadId)
        if (current !== null) {
          this.commitThread(decodeChatThreadEntity({
            ...current,
            title: args.title.trim(),
            updatedAt: FIXED_TIME,
          }))
        }
      }
      results.push(new MutationResult({ mutationId: mutation.mutationId, status: "applied" }))
    }
    this.clientLastMutationId = last
    return new PushResponse({ protocolVersion: 1, results, lastMutationId: last })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    this.seenScopes.push(scope)
    const record = { handlers, open: true }
    this.sockets.set(scope, record)
    return {
      close: () => {
        record.open = false
      },
    }
  }
}

const fakeChatTransport = (
  server: FakeChatSyncServer,
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
            : new KhalaSyncTransportError("network", true, String(error), {
                cause: error,
              }),
        )
      }
    })

  return {
    bootstrap: request => attempt(() => server.bootstrap(request)),
    logPage: (scope, cursor) => attempt(() => server.logPage(scope, cursor)),
    push: request => attempt(() => server.push(request)),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
  }
}

const makeChatHarness = (
  envOverrides: Record<string, string | undefined> = {},
) => {
  const server = new FakeChatSyncServer()
  const env: Record<string, string | undefined> = {
    KHALA_SYNC_CHAT: "1",
    KHALA_SYNC_CHAT_OWNER_USER_ID: CHAT_OWNER_ID,
    OPENAGENTS_AGENT_TOKEN: "oa_agent_chat_token_1",
    OPENAGENTS_BASE_URL: "https://openagents.test",
    KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: "/tmp/khala-sync-chat-service-test-missing-settings.json",
    ...envOverrides,
  }
  const service = createKhalaCodeDesktopKhalaSyncService({
    env,
    storePath: ":memory:",
    sleep: () => tick(),
    random: () => 0.5,
    now: () => new Date(FIXED_TIME),
    transport: config => fakeChatTransport(server, config.authToken),
  })
  return { env, server, service }
}

describe("khala-sync-service flag gating", () => {
  test("fleet flag defaults on and accepts explicit opt-out values", () => {
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({})).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: undefined })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "" })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "1" })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "true" })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "0" })).toBe(false)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "false" })).toBe(false)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "off" })).toBe(false)
  })

  test("explicit flag off: state answers honestly disabled and mutate refuses", async () => {
    const { service } = makeHarness({ KHALA_SYNC_FLEET: "0" })
    expect(await service.fleetState({ runId: RUN_ID })).toEqual(khalaSyncFleetDisabledState())
    expect(await service.fleetMutate({ action: "pause", runId: RUN_ID })).toEqual({
      ok: false,
      error: "khala_sync_fleet_disabled",
    })
    await service.close()
  })

  test("missing OpenAgents auth: enabled but honestly not connected", async () => {
    const { service } = makeHarness({ OPENAGENTS_AGENT_TOKEN: undefined })
    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.enabled).toBe(true)
    expect(state.authState).toBe("missing")
    expect(state.phase).toBe("idle")
    expect(state.error).toBe("missing_openagents_auth")
    expect(await service.fleetMutate({ action: "resume", runId: RUN_ID })).toEqual({
      ok: false,
      error: "missing_openagents_auth",
    })
    await service.close()
  })
})

describe("khala-sync-service chat control bridge", () => {
  test("create, append, rename, and read messages through authenticated chat scopes", async () => {
    const { server, service } = makeChatHarness()

    await expect(service.chatCreateThread({
      threadId: CHAT_THREAD_ID,
      title: "Remote thread",
    })).resolves.toEqual({ ok: true, threadId: CHAT_THREAD_ID })

    await expect(service.chatAppendMessage({
      body: "hello from mobile",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })).resolves.toEqual({
      ok: true,
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })

    await expect(service.chatRenameThread({
      threadId: CHAT_THREAD_ID,
      title: "Renamed remote thread",
    })).resolves.toEqual({ ok: true, threadId: CHAT_THREAD_ID })

    const messages = await service.chatMessages({ threadId: CHAT_THREAD_ID })
    expect(messages).toMatchObject({
      authState: "connected",
      enabled: true,
      ok: true,
      ownerUserId: CHAT_OWNER_ID,
      phase: "live",
      threadId: CHAT_THREAD_ID,
    })
    expect(messages.messages.map(message => [message.messageId, message.body])).toEqual([
      [CHAT_MESSAGE_ID, "hello from mobile"],
    ])

    await waitFor(async () => {
      const state = await service.chatThreads({})
      const first = state.threads[0]
      return first?.messageCount === 1 && first.title === "Renamed remote thread"
    }, "chat sidebar projection includes appended message metadata")
    const threads = await service.chatThreads({})
    expect(threads.threads[0]).toMatchObject({
      messageCount: 1,
      threadId: CHAT_THREAD_ID,
      title: "Renamed remote thread",
    })
    expect(server.seenScopes).toContain(threadScope(CHAT_THREAD_ID))
    expect(server.pushedMutations.map(mutation => String(mutation.name))).toEqual([
      CHAT_CREATE_THREAD_MUTATOR_NAME,
      CHAT_APPEND_MESSAGE_MUTATOR_NAME,
      CHAT_RENAME_THREAD_MUTATOR_NAME,
    ])

    const personalScopePayload = server
      .fold(personalScope(CHAT_OWNER_ID))
      .map(entry => entry.postImageJson)
      .join("\n")
    expect(personalScopePayload).not.toContain("hello from mobile")
    expect(personalScopePayload).not.toContain(CHAT_MESSAGE_ENTITY_TYPE)
    await service.close()
  })

  test("append exposes pending mutation state while the push is retrying", async () => {
    const { server, service } = makeChatHarness()
    await service.chatCreateThread({
      threadId: CHAT_THREAD_ID,
      title: "Remote thread",
    })

    server.holdPushes = true
    const append = service.chatAppendMessage({
      body: "pending mobile message",
      messageId: "chat-message.remote.test.pending",
      threadId: CHAT_THREAD_ID,
    })

    await waitFor(async () => {
      const state = await service.chatMessages({ threadId: CHAT_THREAD_ID })
      return (
        state.pendingMutations === 1 &&
        state.messages.some(message => message.body === "pending mobile message")
      )
    }, "pending chat append visible")

    server.holdPushes = false
    await expect(append).resolves.toMatchObject({
      ok: true,
      messageId: "chat-message.remote.test.pending",
    })
    await waitFor(async () => {
      const state = await service.chatMessages({ threadId: CHAT_THREAD_ID })
      return state.pendingMutations === 0
    }, "pending chat append confirmed")
    await service.close()
  })

  test("server rejection is returned safely and retained in chat state", async () => {
    const { server, service } = makeChatHarness()
    await service.chatCreateThread({
      threadId: CHAT_THREAD_ID,
      title: "Remote thread",
    })

    server.rejectPushes = true
    const rejected = await service.chatAppendMessage({
      body: "should be rejected",
      messageId: "chat-message.remote.test.rejected",
      threadId: CHAT_THREAD_ID,
    })
    expect(rejected).toMatchObject({
      ok: false,
      messageId: "chat-message.remote.test.rejected",
      threadId: CHAT_THREAD_ID,
    })
    expect(rejected.error).toContain("different user")

    const state = await service.chatMessages({ threadId: CHAT_THREAD_ID })
    expect(state.rejections[0]).toMatchObject({
      errorCode: "unauthorized_scope",
      mutatorName: CHAT_APPEND_MESSAGE_MUTATOR_NAME,
      threadId: CHAT_THREAD_ID,
    })
    expect(state.messages.some(message => message.body === "should be rejected")).toBe(false)
    await service.close()
  })

  test("missing chat owner or auth returns typed public-safe errors", async () => {
    const missingOwner = makeChatHarness({
      KHALA_SYNC_CHAT_OWNER_USER_ID: undefined,
    })
    await expect(missingOwner.service.chatAppendMessage({
      body: "hello",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })).resolves.toEqual({
      ok: false,
      error: "missing_chat_owner_user_id",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })
    await missingOwner.service.close()

    const missingAuth = makeChatHarness({
      OPENAGENTS_AGENT_TOKEN: undefined,
    })
    await expect(missingAuth.service.chatAppendMessage({
      body: "hello",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })).resolves.toEqual({
      ok: false,
      error: "missing_openagents_auth",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })
    await missingAuth.service.close()

    const disabled = makeChatHarness({
      KHALA_SYNC_CHAT: undefined,
    })
    await expect(disabled.service.chatAppendMessage({
      body: "hello",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })).resolves.toEqual({
      ok: false,
      error: "khala_sync_chat_disabled",
      messageId: CHAT_MESSAGE_ID,
      threadId: CHAT_THREAD_ID,
    })
    await disabled.service.close()
  })
})

describe("khala-sync-service fleet scope consumption", () => {
  // Oracle khala_sync_phase_is_session_truth.service for contract
  // khala_code.fleet.khala_sync_indicator_truthful.v1: the RPC-exposed phase
  // is the session's real scope state — "live" appears only once bootstrap +
  // catch-up completed and the live socket is open.
  test("subscribe: synced fleet entities appear through the RPC view and phase reaches live", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
      { entityType: "fleet_worker", entityId: "worker.slot.1", postImageJson: workerImage },
      { entityType: "fleet_assignment", entityId: "assignment.test.1", postImageJson: assignmentImage },
      { entityType: "fleet_account", entityId: "account.pylon.codex.0123456789abcdef01234567", postImageJson: accountImage },
    ])

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.phase === "live" && state.run !== null
    }, "fleet scope live with entities")

    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.enabled).toBe(true)
    expect(state.authState).toBe("connected")
    expect(state.phase).toBe("live")
    expect(state.run).toMatchObject({
      runId: RUN_ID,
      status: "running",
      desiredSlots: 3,
      workerKind: "codex",
    })
    expect(state.workers).toHaveLength(1)
    expect(state.workers[0]).toMatchObject({ workerId: "worker.slot.1", phase: "dispatched" })
    expect(state.assignments).toHaveLength(1)
    expect(state.assignments[0]).toMatchObject({ assignmentRef: "assignment.test.1", issueRef: "#8303" })
    expect(state.accounts).toHaveLength(1)
    expect(state.accounts[0]).toMatchObject({ readiness: "ready" })

    // Live delta: a new confirmed post-image lands without any re-poll of
    // the server REST surface.
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage({ desiredSlots: 5 }) },
    ])
    await waitFor(async () => {
      const next = await service.fleetState({ runId: RUN_ID })
      return next.run?.desiredSlots === 5
    }, "live delta visible")
    await service.close()
  })

  test("mutate: optimistic overlay first, server confirmation after", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    server.holdPushes = true
    const mutate = await service.fleetMutate({ action: "pause", runId: RUN_ID })
    expect(mutate.ok).toBe(true)

    // Optimistic: the view flips immediately while the push queue waits.
    const optimistic = await service.fleetState({ runId: RUN_ID })
    expect(optimistic.run?.status).toBe("paused")
    expect(optimistic.pendingMutations).toBe(1)

    // Server confirms: queue drains, confirmed post-image wins, no residue.
    server.holdPushes = false
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.run?.status === "paused"
    }, "pause confirmed")
    expect(server.pushedMutations.some(m => m.name === FLEET_PAUSE_RUN_MUTATOR_NAME)).toBe(true)
    await service.close()
  })

  test("mutate set_desired_slots validates and applies", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    expect(
      await service.fleetMutate({ action: "set_desired_slots", runId: RUN_ID }),
    ).toEqual({ ok: false, error: "desired_slots_out_of_range" })
    expect(
      await service.fleetMutate({ action: "set_desired_slots", desiredSlots: 4096, runId: RUN_ID }),
    ).toEqual({ ok: false, error: "desired_slots_out_of_range" })

    const ok = await service.fleetMutate({
      action: "set_desired_slots",
      desiredSlots: 7,
      runId: RUN_ID,
    })
    expect(ok.ok).toBe(true)
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.run?.desiredSlots === 7
    }, "desired slots confirmed")
    await service.close()
  })

  test("mutate pause_worker/resume_worker: requires workerId, patches the synced worker", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
      { entityType: "fleet_worker", entityId: "worker.slot.1", postImageJson: workerImage },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    expect(
      await service.fleetMutate({ action: "pause_worker", runId: RUN_ID }),
    ).toEqual({ ok: false, error: "worker_id_required" })

    // Optimistic while the push is held, then server-confirmed.
    server.holdPushes = true
    const pause = await service.fleetMutate({
      action: "pause_worker",
      runId: RUN_ID,
      workerId: "worker.slot.1",
    })
    expect(pause.ok).toBe(true)
    const optimistic = await service.fleetState({ runId: RUN_ID })
    expect(optimistic.workers[0]).toMatchObject({ workerId: "worker.slot.1", phase: "paused" })
    // Allowlisted fields the mutator does not own survive the patch.
    expect(optimistic.workers[0]?.accountRefHash).toBe(
      "account.pylon.codex.0123456789abcdef01234567",
    )
    server.holdPushes = false
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.workers[0]?.phase === "paused"
    }, "pause_worker confirmed")

    const resume = await service.fleetMutate({
      action: "resume_worker",
      runId: RUN_ID,
      workerId: "worker.slot.1",
    })
    expect(resume.ok).toBe(true)
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.workers[0]?.phase === "idle"
    }, "resume_worker confirmed")
    expect(
      server.pushedMutations.some(m => m.name === FLEET_PAUSE_WORKER_MUTATOR_NAME),
    ).toBe(true)
    expect(
      server.pushedMutations.some(m => m.name === FLEET_RESUME_WORKER_MUTATOR_NAME),
    ).toBe(true)
    await service.close()
  })

  test("mutate acknowledge_inbox_flag: requires flagRef and queues the named mutator", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    expect(
      await service.fleetMutate({ action: "acknowledge_inbox_flag", runId: RUN_ID }),
    ).toEqual({ ok: false, error: "flag_ref_required" })

    const ack = await service.fleetMutate({
      action: "acknowledge_inbox_flag",
      flagRef: "inbox-flag.run_blocked.1",
      runId: RUN_ID,
    })
    expect(ack.ok).toBe(true)
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0
    }, "ack confirmed")
    const pushed = server.pushedMutations.find(
      m => m.name === FLEET_ACKNOWLEDGE_INBOX_FLAG_MUTATOR_NAME,
    )
    expect(pushed).toBeDefined()
    expect(JSON.parse(pushed!.argsJson)).toEqual({
      flagRef: "inbox-flag.run_blocked.1",
      runId: RUN_ID,
    })
    await service.close()
  })

  test("mutate stop: refuses locally without confirm, terminal when confirmed", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    // Unconfirmed stops are refused before anything is queued (they are a
    // guaranteed server-side confirmation_required rejection).
    expect(
      await service.fleetMutate({ action: "stop", runId: RUN_ID }),
    ).toEqual({ ok: false, error: "confirm_required" })
    expect(
      await service.fleetMutate({ action: "stop", confirm: false, runId: RUN_ID }),
    ).toEqual({ ok: false, error: "confirm_required" })
    expect(
      server.pushedMutations.some(m => m.name === FLEET_STOP_RUN_MUTATOR_NAME),
    ).toBe(false)

    const stop = await service.fleetMutate({ action: "stop", confirm: true, runId: RUN_ID })
    expect(stop.ok).toBe(true)
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return (
        state.pendingMutations === 0 &&
        state.run?.status === "stopped" &&
        state.run.desiredSlots === 0
      )
    }, "stop confirmed")
    await service.close()
  })

  test("in-band rejection: surfaced as state, queue drains, server truth wins", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    server.rejectPushes = true
    const mutate = await service.fleetMutate({ action: "pause", runId: RUN_ID })
    expect(mutate.ok).toBe(true) // queued optimistically; rejection is in-band

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.rejections.length > 0 && state.pendingMutations === 0
    }, "rejection surfaced and queue drained")

    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.rejections[0]).toMatchObject({
      errorCode: "unauthorized_scope",
      mutatorName: FLEET_PAUSE_RUN_MUTATOR_NAME,
      runId: RUN_ID,
    })
    // No optimistic residue: the rejected pause vanished with the ack and
    // the confirmed status stays the server's.
    expect(state.run?.status).toBe("running")
    await service.close()
  })

  // Oracle khala_sync_must_refetch_rebootstraps.service for contract
  // khala_code.fleet.khala_sync_must_refetch_recovers.v1: MustRefetch never
  // strands the consumer — the session re-bootstraps on its own and the view
  // converges on the replaced scope content.
  test("MustRefetch: session re-bootstraps automatically and the view recovers", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    // Server-side scope reset: versions restart, contents replaced.
    server.replaceScope([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage({ desiredSlots: 9, status: "paused" }) },
    ])
    server.emitMustRefetch()

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.phase === "live" && state.run?.desiredSlots === 9
    }, "re-bootstrap recovered the replaced scope")
    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.run?.status).toBe("paused")
    await service.close()
  })

  test("auth token propagation: transport reads the resolved token per request, rotation included", async () => {
    const harness = makeHarness()
    const { server, service, env } = harness
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")
    expect(server.seenAuthTokens).toContain("oa_agent_test_token_1")
    expect(server.seenAuthTokens).not.toContain("")

    // Rotate the token: the next RPC entry re-resolves it and the transport
    // picks it up on the next request without a rebuilt session.
    env.OPENAGENTS_AGENT_TOKEN = "oa_agent_test_token_2"
    await service.fleetState({ runId: RUN_ID })
    await service.fleetMutate({ action: "resume", runId: RUN_ID })
    await waitFor(
      () => server.seenAuthTokens.includes("oa_agent_test_token_2"),
      "rotated token observed by transport",
    )
    await service.close()
  })
})
