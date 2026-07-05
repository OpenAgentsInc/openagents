import {
  canonicalJson,
  ClientGroupId,
  ClientId,
  KHALA_SYNC_PROTOCOL_VERSION,
  KhalaRuntimeControlIntentSchemaLiteral,
  KhalaRuntimeEventSchemaLiteral,
  MutationEnvelope,
  MutationId,
  MutatorName,
  PushRequest,
  SyncSchemaVersion,
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
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  chatMutators,
} from "./chat-mutators.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import {
  readChatMessageById,
  readPendingRuntimeControlIntents,
  readRuntimeTurnById,
} from "./runtime-intents.js"
import {
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  RUNTIME_RETRY_TURN_MUTATOR_NAME,
  RUNTIME_START_TURN_MUTATOR_NAME,
  runtimeMutators,
} from "./runtime-mutators.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const schemaVersion = SyncSchemaVersion.make(1)

let clientCounter = 0
const freshClient = () => {
  clientCounter += 1
  return {
    clientGroupId: ClientGroupId.make(`cg-runtime-intents-${clientCounter}`),
    clientId: ClientId.make(`c-runtime-intents-${clientCounter}`),
    userId: `user-runtime-intents-${clientCounter}`,
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

const iso = "2026-07-05T12:00:00.000Z"

const turnStartIntent = (input: {
  intentId: string
  threadId: string
  turnId: string
  bodyRef: string
}) => ({
  bodyRef: input.bodyRef,
  causalityRefs: [],
  createdAt: iso,
  idempotencyKey: `idem.${input.intentId}`,
  intentId: input.intentId,
  kind: "turn.start" as const,
  origin: {
    lane: "khala_sync_mobile_control" as const,
    surface: "mobile" as const,
  },
  redactionClass: "private_ref" as const,
  schema: KhalaRuntimeControlIntentSchemaLiteral,
  target: {
    adapterKind: "codex" as const,
    lane: "codex_app_server" as const,
  },
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private" as const,
})

const runtimeEvent = (input: {
  eventId: string
  threadId: string
  turnId: string
  sequence: number
}) => ({
  causalityRefs: [],
  eventId: input.eventId,
  kind: "turn.started" as const,
  observedAt: iso,
  redactionClass: "private_ref" as const,
  schema: KhalaRuntimeEventSchemaLiteral,
  sequence: input.sequence,
  source: {
    adapterKind: "codex" as const,
    lane: "codex_app_server" as const,
    surface: "desktop" as const,
  },
  threadId: input.threadId,
  turnId: input.turnId,
  visibility: "private" as const,
})

const registry = makeMutatorRegistry([...runtimeMutators, ...chatMutators])

describe.skipIf(!hasLocalPostgres())(
  "runtime control-intent + chat-message readers against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_runtime_intents")
      await admin.end()
      const result = await runMigrations({
        databaseUrl: pg.urlFor("khala_sync_runtime_intents"),
      })
      expect(result.applied).toContain(
        "0032_khala_sync_runtime_control_intents_seq.sql",
      )
      sql = new SQL({ url: pg.urlFor("khala_sync_runtime_intents"), max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("readPendingRuntimeControlIntents pages oldest-first from a seq watermark", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.reader.1"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            turnStartIntent({
              bodyRef: "chat_message.reader-msg-1",
              intentId: "runtime-intent.reader.1",
              threadId,
              turnId: "runtime-turn.reader.1",
            }),
          ),
          envelope(
            2,
            RUNTIME_START_TURN_MUTATOR_NAME,
            turnStartIntent({
              bodyRef: "chat_message.reader-msg-2",
              intentId: "runtime-intent.reader.2",
              threadId,
              turnId: "runtime-turn.reader.2",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual(["applied", "applied"])

      const firstPage = await readPendingRuntimeControlIntents(
        sql as unknown as SyncSql,
        { limit: 1, ownerUserId: client.userId },
      )
      expect(firstPage).toHaveLength(1)
      expect(firstPage[0]!.intentId).toBe("runtime-intent.reader.1")
      expect(firstPage[0]!.kind).toBe("turn.start")
      expect(firstPage[0]!.threadId).toBe(threadId)
      expect(firstPage[0]!.intent.kind).toBe("turn.start")

      const nextPage = await readPendingRuntimeControlIntents(
        sql as unknown as SyncSql,
        { afterSeq: firstPage[0]!.seq, ownerUserId: client.userId },
      )
      expect(nextPage).toHaveLength(1)
      expect(nextPage[0]!.intentId).toBe("runtime-intent.reader.2")
      expect(nextPage[0]!.seq).toBeGreaterThan(firstPage[0]!.seq)

      const upToDate = await readPendingRuntimeControlIntents(
        sql as unknown as SyncSql,
        { afterSeq: nextPage[0]!.seq, ownerUserId: client.userId },
      )
      expect(upToDate).toHaveLength(0)
    })

    test("readPendingRuntimeControlIntents scoped to ownerUserId never leaks a different owner's intents", async () => {
      const clientA = freshClient()
      const clientB = freshClient()
      await executePush({
        registry,
        request: pushRequest(clientA, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            turnStartIntent({
              bodyRef: "chat_message.owner-a-msg",
              intentId: "runtime-intent.owner-a.1",
              threadId: "runtime-thread.owner-a.1",
              turnId: "runtime-turn.owner-a.1",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: clientA.userId,
      })
      await executePush({
        registry,
        request: pushRequest(clientB, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            turnStartIntent({
              bodyRef: "chat_message.owner-b-msg",
              intentId: "runtime-intent.owner-b.1",
              threadId: "runtime-thread.owner-b.1",
              turnId: "runtime-turn.owner-b.1",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: clientB.userId,
      })

      const ownedByA = await readPendingRuntimeControlIntents(
        sql as unknown as SyncSql,
        { ownerUserId: clientA.userId },
      )
      expect(ownedByA.map((row) => row.intentId)).toEqual([
        "runtime-intent.owner-a.1",
      ])
    })

    test("readChatMessageById resolves the real prompt text written by chat.appendMessage", async () => {
      const client = freshClient()
      const threadId = "chat-thread.runtime-reader.1"
      const messageId = "chat-message.runtime-reader.1"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId,
            title: "Runtime reader fixture thread",
          }),
          envelope(2, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "the real prompt text for a turn.start bodyRef",
            messageId,
            threadId,
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map((r) => r.status)).toEqual(["applied", "applied"])

      const message = await readChatMessageById(sql as unknown as SyncSql, {
        messageId,
        threadId,
      })
      expect(message).not.toBeNull()
      expect(message?.body).toBe("the real prompt text for a turn.start bodyRef")
      expect(message?.authorUserId).toBe(client.userId)
      expect(message?.threadId).toBe(threadId)

      // Cross-thread confusion guard: a message id that exists but under a
      // different thread must not resolve.
      const wrongThread = await readChatMessageById(sql as unknown as SyncSql, {
        messageId,
        threadId: "chat-thread.wrong.1",
      })
      expect(wrongThread).toBeNull()
    })

    test("readChatMessageById returns null for an unknown message id — a real error condition, never silently faked", async () => {
      const message = await readChatMessageById(sql as unknown as SyncSql, {
        messageId: "chat-message.does-not-exist.1",
      })
      expect(message).toBeNull()
    })

    test("readRuntimeTurnById reports the CURRENT event_count so a turn.retry redispatch can resume numbering past it (#8410 follow-up)", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.turn-reader.1"
      const turnId = "runtime-turn.turn-reader.1"

      const started = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            turnStartIntent({
              bodyRef: "chat_message.turn-reader-msg-1",
              intentId: "runtime-intent.turn-reader.start",
              threadId,
              turnId,
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(started.results.map((r) => r.status)).toEqual(["applied"])

      const freshTurn = await readRuntimeTurnById(sql as unknown as SyncSql, { turnId })
      expect(freshTurn).not.toBeNull()
      expect(freshTurn?.status).toBe("queued")
      expect(freshTurn?.eventCount).toBe(0)
      expect(freshTurn?.ownerUserId).toBe(client.userId)
      expect(freshTurn?.threadId).toBe(threadId)
      expect(freshTurn?.lane).toBe("codex_app_server")

      // Record 3 real runtime events against this turn (as the dispatch
      // consumer would while the turn is running), then close it out.
      // mutationId continues from where the `started` push left off (1) —
      // the push engine's idempotency ledger is a per-client monotonic
      // sequence, not scoped to a single request.
      const eventsPush = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.turn-reader.1",
            sequence: 1,
            threadId,
            turnId,
          })),
          envelope(3, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.turn-reader.2",
            sequence: 2,
            threadId,
            turnId,
          })),
          envelope(4, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.turn-reader.3",
            sequence: 3,
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(eventsPush.results.map((r) => r.status)).toEqual(["applied", "applied", "applied"])

      const afterEvents = await readRuntimeTurnById(sql as unknown as SyncSql, { turnId })
      expect(afterEvents?.eventCount).toBe(3)

      // A real turn.retry control intent re-queues the SAME turnId (the
      // mutator does not create a new turn row) — readRuntimeTurnById must
      // keep reporting the accumulated event_count so a redispatch resumes
      // numbering at 4, not 1.
      const retryPush = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(5, RUNTIME_RETRY_TURN_MUTATOR_NAME, {
            ...turnStartIntent({
              bodyRef: "chat_message.turn-reader-msg-1",
              intentId: "runtime-intent.turn-reader.retry",
              threadId,
              turnId,
            }),
            kind: "turn.retry",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(retryPush.results.map((r) => r.status)).toEqual(["applied"])

      const afterRetry = await readRuntimeTurnById(sql as unknown as SyncSql, { turnId })
      expect(afterRetry?.status).toBe("queued")
      expect(afterRetry?.eventCount).toBe(3)
    })

    test("readRuntimeTurnById returns null for a turnId nobody ever started — a real error condition for turn.continue/turn.retry, never a silent skip", async () => {
      const turn = await readRuntimeTurnById(sql as unknown as SyncSql, {
        turnId: "runtime-turn.does-not-exist.1",
      })
      expect(turn).toBeNull()
    })
  },
)
