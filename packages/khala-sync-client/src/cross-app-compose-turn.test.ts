/**
 * Dual-client chat.composeTurn proof (Effect Native #64 residual).
 *
 * Two real `createKhalaSyncSession` clients (desktop + mobile clientIds)
 * share one FakeSyncServer over the real transport seam. Each mutates
 * `chat.composeTurn`; the server commits post-image changelog entries and
 * fans `DeltaFrame`s so both sessions converge on the same transcript
 * entities — the Khala Sync protocol algebra, not a hand-rolled hub.
 *
 * This is still an **in-process fake server** (no Cloud SQL / live staging).
 * It is strictly stronger than a framework-only memory hub: it exercises
 * overlay mutators, push, live socket apply, and dual durable stores from
 * `@openagentsinc/khala-sync-client`.
 */
import {
  BootstrapEntity,
  type BootstrapRequest,
  BootstrapResponse,
  ChangelogEntry,
  ClientGroupId,
  ClientId,
  DeltaFrame,
  EntityId,
  EntityType,
  type LiveFrame,
  LogPage,
  MutationEnvelope,
  MutationId,
  MutationResult as MutationResultClass,
  MutatorName,
  PushResponse,
  SyncSchemaVersion,
  SyncScope,
  SyncVersion,
  SyncVersionWatermark,
  canonicalJson,
} from "@openagentsinc/khala-sync"
import { afterEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { type ClientMutator, createOverlay } from "./overlay.js"
import { createKhalaSyncSession } from "./session.js"
import { openKhalaSyncStore } from "./sqlite-store.js"
import {
  type KhalaSyncTransport,
  KhalaSyncTransportError,
  type LiveSocketHandlers,
} from "./transport.js"

const tick = (): Promise<void> => new Promise((resolve) => setTimeout(resolve, 0))

const waitFor = async (
  condition: () => boolean,
  label: string,
  ticks = 3000,
): Promise<void> => {
  for (let i = 0; i < ticks; i++) {
    if (condition()) return
    await tick()
  }
  throw new Error(`timed out waiting for: ${label}`)
}

const FIXED_TIME = "2026-07-09T00:00:00.000Z"
const threadScope = SyncScope.make("scope.thread.cross-app-proof")

const cleanups: Array<() => void> = []
afterEach(() => {
  while (cleanups.length > 0) cleanups.pop()!()
})

interface SocketRecord {
  handlers: LiveSocketHandlers
  open: boolean
}

/** Minimal SPEC §3 server that understands chat.composeTurn for dual clients. */
class DualClientChatServer {
  readonly logs = new Map<SyncScope, Array<ChangelogEntry>>()
  /** Multiple live sockets per scope (desktop + mobile). */
  readonly sockets = new Map<SyncScope, Array<SocketRecord>>()
  readonly clientLast = new Map<string, number>()
  readonly pushCalls: Array<ReadonlyArray<MutationEnvelope>> = []

  private logOf(scope: SyncScope): Array<ChangelogEntry> {
    const existing = this.logs.get(scope)
    if (existing !== undefined) return existing
    const created: Array<ChangelogEntry> = []
    this.logs.set(scope, created)
    return created
  }

  private lastVersion(scope: SyncScope): number {
    const log = this.logOf(scope)
    return log.length === 0 ? 0 : Number(log[log.length - 1]!.version)
  }

  private emitFrame(scope: SyncScope, frame: LiveFrame): void {
    const sockets = this.sockets.get(scope) ?? []
    for (const socket of sockets) {
      if (socket.open) socket.handlers.onFrame(frame)
    }
  }

  commit(
    scope: SyncScope,
    change: {
      readonly entityType: string
      readonly entityId: string
      readonly postImageJson: string
    },
    mutationRef: string,
  ): number {
    const version = this.lastVersion(scope) + 1
    const entry = new ChangelogEntry({
      scope,
      version: SyncVersion.make(version),
      entityType: EntityType.make(change.entityType),
      entityId: EntityId.make(change.entityId),
      op: "upsert",
      postImageJson: change.postImageJson,
      mutationRef,
      committedAt: FIXED_TIME,
    })
    this.logOf(scope).push(entry)
    this.emitFrame(
      scope,
      new DeltaFrame({
        scope,
        entries: [entry],
        cursor: SyncVersion.make(version),
      }) as LiveFrame,
    )
    return version
  }

  bootstrap(request: BootstrapRequest): BootstrapResponse {
    const scope = request.scope
    const cursor = this.lastVersion(scope)
    const entities = this.logOf(scope)
      .filter((e) => e.op === "upsert" && e.postImageJson !== undefined)
      .map(
        (e) =>
          new BootstrapEntity({
            entityType: e.entityType,
            entityId: e.entityId,
            postImageJson: e.postImageJson!,
          }),
      )
    return new BootstrapResponse({
      protocolVersion: 1,
      scope,
      entities,
      cursor: SyncVersionWatermark.make(cursor),
    })
  }

  logPage(scope: SyncScope, cursor: number, limit: number): LogPage {
    const after = this.logOf(scope).filter((entry) => entry.version > cursor)
    const included = after.slice(0, limit)
    const next =
      included.length === 0
        ? cursor
        : Number(included[included.length - 1]!.version)
    return new LogPage({
      protocolVersion: 1,
      scope,
      entries: included,
      nextCursor: SyncVersionWatermark.make(next),
      upToDate: next >= this.lastVersion(scope),
    })
  }

  push(mutations: ReadonlyArray<MutationEnvelope>, clientKey: string): PushResponse {
    this.pushCalls.push([...mutations])
    let last = this.clientLast.get(clientKey) ?? 0
    const results = []
    for (const mutation of mutations) {
      if (mutation.mutationId <= last) {
        results.push(
          new MutationResultClass({
            mutationId: mutation.mutationId,
            status: "duplicate",
          }),
        )
        continue
      }
      if (mutation.name !== "chat.composeTurn") {
        results.push(
          new MutationResultClass({
            mutationId: mutation.mutationId,
            status: "rejected",
            errorCode: "mutation_rejected",
            errorMessageSafe: `unknown mutator ${mutation.name}`,
          }),
        )
        last = mutation.mutationId
        continue
      }
      const args = JSON.parse(mutation.argsJson) as {
        threadId: string
        text: string
        client: "desktop" | "mobile"
        author: string
        id: string
      }
      this.commit(
        SyncScope.make(args.threadId.startsWith("scope.")
          ? args.threadId
          : `scope.thread.${args.threadId}`),
        {
          entityType: "chat_turn_event",
          entityId: args.id,
          postImageJson: canonicalJson({
            id: args.id,
            threadId: args.threadId,
            role: "user",
            author: args.author,
            text: args.text,
            client: args.client,
            committedAt: FIXED_TIME,
          }),
        },
        `mut.${clientKey}.${mutation.mutationId}`,
      )
      results.push(
        new MutationResultClass({
          mutationId: mutation.mutationId,
          status: "applied",
        }),
      )
      last = mutation.mutationId
    }
    this.clientLast.set(clientKey, last)
    return new PushResponse({
      protocolVersion: 1,
      results,
      lastMutationId: last,
    })
  }

  connect(scope: SyncScope, _cursor: number, handlers: LiveSocketHandlers) {
    const record: SocketRecord = { handlers, open: true }
    const existing = this.sockets.get(scope) ?? []
    existing.push(record)
    this.sockets.set(scope, existing)
    return {
      close: () => {
        record.open = false
      },
    }
  }
}

const transportOf = (server: DualClientChatServer): KhalaSyncTransport => {
  const attempt = <A>(run: () => A): Effect.Effect<A, KhalaSyncTransportError> =>
    Effect.suspend(() => {
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
    bootstrap: (request) => attempt(() => server.bootstrap(request)),
    logPage: (scope, cursor, limit) =>
      attempt(() => server.logPage(scope, cursor, limit)),
    push: (request) =>
      attempt(() =>
        server.push(
          request.mutations,
          `${request.clientGroupId}:${request.clientId}`,
        ),
      ),
    connectLive: (scope, cursor, handlers) =>
      attempt(() => server.connect(scope, cursor, handlers)),
  }
}

interface ComposeArgs {
  readonly threadId: string
  readonly text: string
  readonly client: "desktop" | "mobile"
  readonly author: string
  readonly id: string
}

const composeTurn: ClientMutator<ComposeArgs> = {
  name: MutatorName.make("chat.composeTurn"),
  apply: (args) => [
    {
      kind: "upsert",
      scope: SyncScope.make(
        args.threadId.startsWith("scope.")
          ? args.threadId
          : `scope.thread.${args.threadId}`,
      ),
      entityType: "chat_turn_event",
      entityId: args.id,
      postImageJson: canonicalJson({
        id: args.id,
        threadId: args.threadId,
        role: "user",
        author: args.author,
        text: args.text,
        client: args.client,
        committedAt: FIXED_TIME,
      }),
    },
  ],
}

const makeClient = (
  server: DualClientChatServer,
  clientId: string,
) => {
  const store = openKhalaSyncStore(":memory:")
  cleanups.push(() => Effect.runSync(Effect.ignore(store.close())))
  const overlay = Effect.runSync(createOverlay(store, [composeTurn]))
  const session = createKhalaSyncSession(
    {
      baseUrl: "http://fake.test",
      clientGroupId: ClientGroupId.make("cg_cross_app"),
      clientId: ClientId.make(clientId),
      schemaVersion: SyncSchemaVersion.make(1),
      authToken: () => "test-token",
    },
    store,
    overlay,
    transportOf(server),
    {
      sleep: () => tick(),
      random: () => 0,
      backoffBaseMs: 1,
      backoffMaxMs: 4,
      maxBootstrapAttempts: 3,
      logPageLimit: 50,
      pushBatchSize: 10,
    },
  )
  cleanups.push(() => Effect.runSync(session.close()))
  return {
    session,
    overlay,
    listTurns: () => {
      const view = Effect.runSync(overlay.read(threadScope))
      return view
        .list("chat_turn_event")
        .map((row) => JSON.parse(row.postImageJson) as {
          id: string
          text: string
          client: string
          author: string
        })
        .sort((a, b) => a.id.localeCompare(b.id))
    },
  }
}

describe("cross-app chat.composeTurn over real khala-sync-client sessions", () => {
  test("desktop mutate lands on mobile and mobile mutate lands on desktop via live hub fan-out", async () => {
    const server = new DualClientChatServer()
    const desktop = makeClient(server, "c_desktop")
    const mobile = makeClient(server, "c_mobile")

    await Effect.runPromise(desktop.session.subscribe(threadScope))
    await Effect.runPromise(mobile.session.subscribe(threadScope))
    await waitFor(
      () =>
        desktop.session.state(threadScope).phase === "live" &&
        mobile.session.state(threadScope).phase === "live",
      "both clients live",
    )

    await Effect.runPromise(
      desktop.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "hello from desktop over Khala Sync",
        client: "desktop",
        author: "Desktop",
        id: "turn-desktop-1",
      }),
    )
    await waitFor(
      () => mobile.listTurns().some((t) => t.id === "turn-desktop-1"),
      "mobile sees desktop turn",
    )

    await Effect.runPromise(
      mobile.session.mutate(composeTurn, {
        threadId: "scope.thread.cross-app-proof",
        text: "hello from mobile over Khala Sync",
        client: "mobile",
        author: "Mobile",
        id: "turn-mobile-1",
      }),
    )
    await waitFor(
      () => desktop.listTurns().some((t) => t.id === "turn-mobile-1"),
      "desktop sees mobile turn",
    )

    const desktopTurns = desktop.listTurns()
    const mobileTurns = mobile.listTurns()
    expect(desktopTurns.map((t) => t.id).sort()).toEqual(
      mobileTurns.map((t) => t.id).sort(),
    )
    expect(desktopTurns.map((t) => t.text).sort()).toEqual([
      "hello from desktop over Khala Sync",
      "hello from mobile over Khala Sync",
    ])
    expect(server.pushCalls.length).toBeGreaterThanOrEqual(2)
    // Both clients applied both post-images (optimistic + confirmed).
    expect(desktopTurns).toHaveLength(2)
    expect(mobileTurns).toHaveLength(2)
  })
})
