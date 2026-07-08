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
  personalScope,
  PushRequest,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
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
import { readScopeOwner } from "./fleet-projection.js"
import { logPage } from "./read-service.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import {
  RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
  RUNTIME_CLOSE_TURN_MUTATOR_NAME,
  RUNTIME_EVENT_EXISTS_REJECTION,
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  RUNTIME_RAW_BODY_REJECTION,
  RUNTIME_SCOPE_REJECTION,
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
    clientGroupId: ClientGroupId.make(`cg-runtime-${clientCounter}`),
    clientId: ClientId.make(`c-runtime-${clientCounter}`),
    userId: `user-runtime-${clientCounter}`,
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

const iso = "2026-07-04T12:00:00.000Z"

const controlIntent = (
  input: Readonly<{
    kind:
      | "message.append"
      | "turn.start"
      | "turn.interrupt"
      | "turn.continue"
      | "turn.retry"
      | "turn.close"
    intentId: string
    threadId: string
    turnId?: string | undefined
    messageId?: string | undefined
    body?: string | undefined
    bodyRef?: string | undefined
    promptRef?: string | undefined
    reasonRef?: string | undefined
  }>,
) => ({
  schema: KhalaRuntimeControlIntentSchemaLiteral,
  causalityRefs: [],
  createdAt: iso,
  idempotencyKey: `idem.${input.intentId}`,
  intentId: input.intentId,
  kind: input.kind,
  origin: {
    lane: "khala_sync_mobile_control",
    surface: "mobile",
    userRef: "user.ref.test",
  },
  redactionClass: "private_ref",
  target: {
    adapterKind: "codex",
    lane: "codex_app_server",
  },
  threadId: input.threadId,
  visibility: "private",
  ...(input.turnId === undefined ? {} : { turnId: input.turnId }),
  ...(input.messageId === undefined ? {} : { messageId: input.messageId }),
  ...(input.body === undefined ? {} : { body: input.body }),
  ...(input.bodyRef === undefined ? {} : { bodyRef: input.bodyRef }),
  ...(input.promptRef === undefined ? {} : { promptRef: input.promptRef }),
  ...(input.reasonRef === undefined ? {} : { reasonRef: input.reasonRef }),
})

const runtimeEvent = (
  input: Readonly<{
    eventId: string
    kind: "turn.started" | "turn.finished" | "text.delta"
    threadId: string
    turnId: string
    sequence: number
    text?: string | undefined
    finishReason?:
      | "stop"
      | "length"
      | "tool-calls"
      | "content-filter"
      | "error"
      | "cancelled"
      | "interrupted"
      | "unknown"
      | undefined
  }>,
) => {
  const base = {
    schema: KhalaRuntimeEventSchemaLiteral,
    causalityRefs: [],
    eventId: input.eventId,
    kind: input.kind,
    observedAt: iso,
    redactionClass: "private_ref",
    sequence: input.sequence,
    source: {
      adapterKind: "codex",
      lane: "codex_app_server",
      surface: "desktop",
    },
    threadId: input.threadId,
    turnId: input.turnId,
    visibility: "private",
  }
  switch (input.kind) {
    case "turn.started":
      return { ...base }
    case "turn.finished":
      return { ...base, finishReason: input.finishReason ?? "stop" }
    case "text.delta":
      return {
        ...base,
        chunkId: `chunk.${input.eventId}`,
        messageId: `message.${input.eventId}`,
        text: input.text ?? "private runtime text",
      }
  }
}

const registry = makeMutatorRegistry([...runtimeMutators])

describe.skipIf(!hasLocalPostgres())(
  "Khala runtime mutators against local Postgres",
  () => {
    let pg: LocalPostgres
    let sql: SQL

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_sync_runtime")
      await admin.end()
      const result = await runMigrations({
        databaseUrl: pg.urlFor("khala_sync_runtime"),
      })
      expect(result.applied).toContain("0029_khala_sync_runtime.sql")
      sql = new SQL({ url: pg.urlFor("khala_sync_runtime"), max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("accepted control intents and events project only safe data outside the private thread scope", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.flow.1"
      const turnId = "runtime-turn.flow.1"
      const privateText = "private runtime text from the harness"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.flow.start",
              promptRef: "prompt.private.flow",
              threadId,
              turnId,
              kind: "turn.start",
            }),
          ),
          envelope(
            2,
            RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
            controlIntent({
              bodyRef: "body.private.flow",
              intentId: "runtime-intent.flow.append",
              messageId: "runtime-message.flow.1",
              threadId,
              turnId,
              kind: "message.append",
            }),
          ),
          envelope(
            3,
            RUNTIME_RECORD_EVENT_MUTATOR_NAME,
            runtimeEvent({
              eventId: "runtime-event.flow.started",
              kind: "turn.started",
              sequence: 1,
              threadId,
              turnId,
            }),
          ),
          envelope(
            4,
            RUNTIME_RECORD_EVENT_MUTATOR_NAME,
            runtimeEvent({
              eventId: "runtime-event.flow.text",
              kind: "text.delta",
              sequence: 2,
              text: privateText,
              threadId,
              turnId,
            }),
          ),
          envelope(
            5,
            RUNTIME_CLOSE_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.flow.close",
              reasonRef: "reason.closed.flow",
              threadId,
              turnId,
              kind: "turn.close",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
        "applied",
        "applied",
      ])
      expect(Number(response.lastMutationId)).toBe(5)
      expect(await readScopeOwner(sql as unknown as SyncSql, threadScope(threadId))).toBe(
        client.userId,
      )

      const ownerLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 20,
        scope: personalScope(client.userId),
      })
      const threadLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 20,
        scope: threadScope(threadId),
      })

      expect(ownerLog.entries.map((entry) => String(entry.entityType))).toContain(
        RUNTIME_TURN_ENTITY_TYPE,
      )
      expect(ownerLog.entries.map((entry) => String(entry.entityType))).toContain(
        RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
      )
      expect(
        ownerLog.entries.map((entry) => String(entry.entityType)),
      ).not.toContain(RUNTIME_EVENT_ENTITY_TYPE)
      expect(JSON.stringify(ownerLog.entries)).not.toContain(privateText)

      expect(threadLog.entries.map((entry) => String(entry.entityType))).toContain(
        RUNTIME_EVENT_ENTITY_TYPE,
      )
      expect(JSON.stringify(threadLog.entries)).toContain(privateText)

      const publicRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_changelog
        WHERE scope LIKE 'scope.public.%'
      `
      expect(Number(publicRows[0]!.count)).toBe(0)

      const turns: Array<{
        event_count: string | number
        status: string
      }> = await sql`
        SELECT event_count, status
        FROM khala_sync_runtime_turns
        WHERE turn_id = ${turnId}
      `
      expect(Number(turns[0]!.event_count)).toBe(2)
      expect(turns[0]!.status).toBe("closed")
    })

    test("foreign runtime mutation rejects in-band and later queued mutation still applies", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const ownerThread = "runtime-thread.foreign.owner"
      const ownerTurn = "runtime-turn.foreign.owner"
      const intruderThread = "runtime-thread.foreign.intruder"
      const intruderTurn = "runtime-turn.foreign.intruder"

      await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.foreign.owner.start",
              promptRef: "prompt.foreign.owner",
              threadId: ownerThread,
              turnId: ownerTurn,
              kind: "turn.start",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })

      const response = await executePush({
        registry,
        request: pushRequest(intruder, [
          envelope(
            1,
            "runtime.interruptTurn",
            controlIntent({
              intentId: "runtime-intent.foreign.interrupt",
              reasonRef: "reason.foreign.interrupt",
              threadId: ownerThread,
              turnId: ownerTurn,
              kind: "turn.interrupt",
            }),
          ),
          envelope(
            2,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.foreign.intruder.start",
              promptRef: "prompt.foreign.intruder",
              threadId: intruderThread,
              turnId: intruderTurn,
              kind: "turn.start",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })

      expect(response.results[0]!.status).toBe("rejected")
      expect(response.results[0]!.errorCode).toBe(RUNTIME_SCOPE_REJECTION)
      expect(response.results[1]!.status).toBe("applied")
      expect(Number(response.lastMutationId)).toBe(2)
      expect(
        await readScopeOwner(sql as unknown as SyncSql, threadScope(ownerThread)),
      ).toBe(owner.userId)
      expect(
        await readScopeOwner(sql as unknown as SyncSql, threadScope(intruderThread)),
      ).toBe(intruder.userId)

      const leaked: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_runtime_control_intents
        WHERE thread_id = ${ownerThread} AND owner_user_id = ${intruder.userId}
      `
      expect(Number(leaked[0]!.count)).toBe(0)
    })

    test("raw body append rejects without retaining the prompt and the following ref-only append applies", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.body-reject.1"
      const turnId = "runtime-turn.body-reject.1"
      const rawPrompt = "raw prompt secret should not persist"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.body-reject.start",
              promptRef: "prompt.body-reject.start",
              threadId,
              turnId,
              kind: "turn.start",
            }),
          ),
          envelope(
            2,
            RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
            controlIntent({
              body: rawPrompt,
              intentId: "runtime-intent.body-reject.raw",
              messageId: "runtime-message.body-reject.raw",
              threadId,
              turnId,
              kind: "message.append",
            }),
          ),
          envelope(
            3,
            RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
            controlIntent({
              bodyRef: "body.body-reject.good",
              intentId: "runtime-intent.body-reject.good",
              messageId: "runtime-message.body-reject.good",
              threadId,
              turnId,
              kind: "message.append",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "applied",
        "rejected",
        "applied",
      ])
      expect(response.results[1]!.errorCode).toBe(RUNTIME_RAW_BODY_REJECTION)
      expect(Number(response.lastMutationId)).toBe(3)

      const controlRows: Array<{ serialized: string }> = await sql`
        SELECT coalesce(string_agg(intent_json::text, ' '), '') AS serialized
        FROM khala_sync_runtime_control_intents
        WHERE thread_id = ${threadId}
      `
      expect(controlRows[0]!.serialized).not.toContain(rawPrompt)

      const changelogRows: Array<{ serialized: string }> = await sql`
        SELECT coalesce(string_agg(post_image_json::text, ' '), '') AS serialized
        FROM khala_sync_changelog
        WHERE scope = ${threadScope(threadId)}
      `
      expect(changelogRows[0]!.serialized).not.toContain(rawPrompt)
    })

    test("intent_json and event_json are stored as jsonb OBJECTS (not double-encoded string scalars)", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.jsonb-object.1"
      const turnId = "runtime-turn.jsonb-object.1"
      const bodyRef = "chat_message.jsonb-object.prompt"

      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              bodyRef,
              intentId: "runtime-intent.jsonb-object.start",
              threadId,
              turnId,
              kind: "turn.start",
            }),
          ),
          envelope(
            2,
            RUNTIME_RECORD_EVENT_MUTATOR_NAME,
            runtimeEvent({
              eventId: "runtime-event.jsonb-object.started",
              kind: "turn.started",
              sequence: 0,
              threadId,
              turnId,
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "applied",
        "applied",
      ])

      // The regression: intent_json must be a real jsonb OBJECT so
      // `->>'bodyRef'` resolves. A double-encoded string scalar would report
      // `jsonb_typeof = 'string'` and a NULL extraction.
      const intentRows: Array<{ typ: string; body_ref: string | null }> =
        await sql`
          SELECT jsonb_typeof(intent_json) AS typ,
                 intent_json->>'bodyRef' AS body_ref
          FROM khala_sync_runtime_control_intents
          WHERE turn_id = ${turnId} AND kind = 'turn.start'
        `
      expect(intentRows).toHaveLength(1)
      expect(intentRows[0]!.typ).toBe("object")
      expect(intentRows[0]!.body_ref).toBe(bodyRef)

      const eventRows: Array<{ typ: string; kind: string | null }> = await sql`
        SELECT jsonb_typeof(event_json) AS typ,
               event_json->>'kind' AS kind
        FROM khala_sync_runtime_events
        WHERE turn_id = ${turnId}
      `
      expect(eventRows).toHaveLength(1)
      expect(eventRows[0]!.typ).toBe("object")
      expect(eventRows[0]!.kind).toBe("turn.started")
    })

    test("duplicate replay answers from the mutation ledger without re-executing", async () => {
      const client = freshClient()
      const turnId = "runtime-turn.duplicate.1"
      const request = pushRequest(client, [
        envelope(
          1,
          RUNTIME_START_TURN_MUTATOR_NAME,
          controlIntent({
            intentId: "runtime-intent.duplicate.start",
            promptRef: "prompt.duplicate.start",
            threadId: "runtime-thread.duplicate.1",
            turnId,
            kind: "turn.start",
          }),
        ),
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

      const turns: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_runtime_turns
        WHERE turn_id = ${turnId}
      `
      expect(Number(turns[0]!.count)).toBe(1)
    })

    test("duplicate runtime event rejects without blocking the following close", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.event-duplicate.1"
      const turnId = "runtime-turn.event-duplicate.1"

      await executePush({
        registry,
        request: pushRequest(client, [
          envelope(
            1,
            RUNTIME_START_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.event-duplicate.start",
              promptRef: "prompt.event-duplicate.start",
              threadId,
              turnId,
              kind: "turn.start",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      const duplicateEvent = runtimeEvent({
        eventId: "runtime-event.event-duplicate.text",
        kind: "text.delta",
        sequence: 1,
        text: "private duplicated event text",
        threadId,
        turnId,
      })
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, duplicateEvent),
          envelope(3, RUNTIME_RECORD_EVENT_MUTATOR_NAME, duplicateEvent),
          envelope(
            4,
            RUNTIME_CLOSE_TURN_MUTATOR_NAME,
            controlIntent({
              intentId: "runtime-intent.event-duplicate.close",
              reasonRef: "reason.event-duplicate.close",
              threadId,
              turnId,
              kind: "turn.close",
            }),
          ),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map((result) => result.status)).toEqual([
        "applied",
        "rejected",
        "applied",
      ])
      expect(response.results[1]!.errorCode).toBe(RUNTIME_EVENT_EXISTS_REJECTION)
      expect(Number(response.lastMutationId)).toBe(4)

      const events: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_runtime_events
        WHERE turn_id = ${turnId}
      `
      expect(Number(events[0]!.count)).toBe(1)
    })
  },
)
