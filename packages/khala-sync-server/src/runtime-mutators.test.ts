import {
  agentRunScope,
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  canonicalJson,
  ClientGroupId,
  ClientId,
  KHALA_SYNC_PROTOCOL_VERSION,
  LIVE_AGENT_GRAPH_ENTITY_TYPE,
  decodeLiveAgentGraphPostImageJson,
  KhalaRuntimeControlIntentSchemaLiteral,
  KhalaRuntimeEventSchemaLiteral,
  MutationEnvelope,
  MutationId,
  MutatorName,
  personalScope,
  PushRequest,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_INTERACTION_ENTITY_TYPE,
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
import { readPendingRuntimeControlIntents } from "./runtime-intents.js"
import { runMigrations } from "./migrate.js"
import { executePush, makeMutatorRegistry } from "./push-engine.js"
import {
  CHAT_APPEND_MESSAGE_MUTATOR_NAME,
  CHAT_BIND_THREAD_REPO_MUTATOR_NAME,
  CHAT_CREATE_THREAD_MUTATOR_NAME,
  chatMutators,
} from "./chat-mutators.js"
import {
  RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME,
  RUNTIME_CLOSE_TURN_MUTATOR_NAME,
  RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME,
  RUNTIME_EVENT_EXISTS_REJECTION,
  RUNTIME_EVENT_SEQUENCE_REJECTION,
  RUNTIME_EVENT_STATE_REJECTION,
  RUNTIME_EXPIRE_INTERACTION_MUTATOR_NAME,
  RUNTIME_INTERRUPT_TURN_MUTATOR_NAME,
  RUNTIME_INTENT_EXPIRY_REJECTION,
  RUNTIME_INTERACTION_CONFLICT_REJECTION,
  RUNTIME_INTERACTION_DECISION_REJECTION,
  RUNTIME_INTERACTION_EXPIRY_REJECTION,
  RUNTIME_INTERACTION_SEQUENCE_REJECTION,
  RUNTIME_RECORD_EVENT_MUTATOR_NAME,
  RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME,
  RUNTIME_RETRY_TURN_MUTATOR_NAME,
  RUNTIME_RAW_BODY_REJECTION,
  RUNTIME_SCOPE_REJECTION,
  RUNTIME_START_TURN_MUTATOR_NAME,
  RUNTIME_TARGET_LANE_REJECTION,
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
    expiresAt?: string | undefined
    lane?: "codex_app_server" | "claude_pylon" | undefined
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
    adapterKind: input.lane === "claude_pylon" ? "claude_code" : "codex",
    lane: input.lane ?? "codex_app_server",
  },
  threadId: input.threadId,
  visibility: "private",
  ...(input.turnId === undefined ? {} : { turnId: input.turnId }),
  ...(input.messageId === undefined ? {} : { messageId: input.messageId }),
  ...(input.body === undefined ? {} : { body: input.body }),
  ...(input.bodyRef === undefined ? {} : { bodyRef: input.bodyRef }),
  ...(input.promptRef === undefined ? {} : { promptRef: input.promptRef }),
  ...(input.reasonRef === undefined ? {} : { reasonRef: input.reasonRef }),
  ...(input.expiresAt === undefined ? {} : { expiresAt: input.expiresAt }),
})

const runtimeEvent = (
  input: Readonly<{
    eventId: string
    kind:
      | "turn.started"
      | "turn.finished"
      | "text.delta"
      | "agent.child.started"
      | "agent.child.progress"
      | "agent.child.finished"
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
    lane?: "codex_app_server" | "claude_pylon" | undefined
    providerRef?: string | undefined
    childAgentId?: string | undefined
    childRunId?: string | undefined
    parentAgentId?: string | undefined
    taskRef?: string | undefined
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
      adapterKind: input.lane === "claude_pylon" ? "claude_code" : "codex",
      lane: input.lane ?? "codex_app_server",
      surface: "desktop",
      ...(input.providerRef === undefined ? {} : { providerRef: input.providerRef }),
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
    case "agent.child.started":
    case "agent.child.progress":
      return {
        ...base,
        childAgentId: input.childAgentId!,
        childRunId: input.childRunId!,
        parentAgentId: input.parentAgentId!,
        ...(input.taskRef === undefined ? {} : { taskRef: input.taskRef }),
      }
    case "agent.child.finished":
      return {
        ...base,
        childAgentId: input.childAgentId!,
        childRunId: input.childRunId!,
        parentAgentId: input.parentAgentId!,
        ...(input.taskRef === undefined ? {} : { taskRef: input.taskRef }),
        finishReason: input.finishReason ?? "stop",
      }
  }
}

const runtimeQuestionInteraction = (input: Readonly<{
  interactionRef: string
  threadId: string
  turnId: string
  requestedSequence: number
  lane?: "codex_app_server" | "claude_pylon" | undefined
  expiresAt?: string | undefined
}>) => ({
  schema: "openagents.runtime_interaction.v1" as const,
  interactionRef: input.interactionRef,
  threadId: input.threadId,
  turnId: input.turnId,
  requestedSequence: input.requestedSequence,
  requestedAt: iso,
  expiresAt: input.expiresAt ?? "2099-07-11T22:05:00.000Z",
  source: {
    lane: input.lane ?? "codex_app_server",
    adapterKind: input.lane === "claude_pylon" ? "claude_code" as const : "codex" as const,
    surface: "server" as const,
  },
  visibility: "private" as const,
  redactionClass: "private_ref" as const,
  causalityRefs: ["event.runtime.question.request"],
  payload: {
    kind: "provider_question" as const,
    displayTitle: "Choose verification",
    questions: [{
      questionRef: "question.runtime.1",
      displayText: "Which verification should run?",
      multiSelect: false,
      options: [
        { optionRef: "option.tests", label: "Tests" },
        { optionRef: "option.smoke", label: "Smoke" },
      ],
    }],
  },
  lifecycle: { status: "pending" as const },
})

const runtimeQuestionDecision = (input: Readonly<{
  interactionRef: string
  threadId: string
  turnId: string
  optionRef?: string | undefined
  decisionRef?: string | undefined
  idempotencyKey?: string | undefined
}>) => ({
  interactionRef: input.interactionRef,
  threadId: input.threadId,
  turnId: input.turnId,
  envelope: {
    decisionRef: input.decisionRef ?? "decision.runtime.question.1",
    idempotencyKey: input.idempotencyKey ?? "idem.runtime.question.1",
    decidedAt: iso,
    surface: "mobile" as const,
    decision: {
      kind: "provider_question" as const,
      answers: [{
        questionRef: "question.runtime.1",
        optionRefs: [input.optionRef ?? "option.tests"],
      }],
    },
  },
})

const registry = makeMutatorRegistry([...chatMutators, ...runtimeMutators])

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
      expect(result.applied).toContain("0061_runtime_control_intent_expiry.sql")
      sql = new SQL({ url: pg.urlFor("khala_sync_runtime"), max: 10 })
    })

    afterAll(async () => {
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (sql !== undefined) await sql.end()
      // eslint-disable-next-line @typescript-eslint/no-unnecessary-condition
      if (pg !== undefined) await pg.stop()
    })

    test("conversation admission mirrors exact runtime refs into the canonical agent timeline and reconciles semantic retry", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.conversation.8676"
      const messageId = "runtime-message.conversation.8676"
      const turnId = "runtime-turn.conversation.8676"
      const startIntent = controlIntent({
        bodyRef: `chat_message.${messageId}`,
        intentId: "runtime-intent.conversation.8676",
        kind: "turn.start",
        threadId,
        turnId,
      })

      const admitted = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId,
            title: "Issue 8676",
          }),
          envelope(2, CHAT_BIND_THREAD_REPO_MUTATOR_NAME, {
            repo: {
              defaultBranch: "main",
              name: "openagents",
              owner: "OpenAgentsInc",
            },
            threadId,
          }),
          envelope(3, CHAT_APPEND_MESSAGE_MUTATOR_NAME, {
            body: "Start the real streamed conversation.",
            messageId,
            threadId,
          }),
          envelope(4, RUNTIME_START_TURN_MUTATOR_NAME, startIntent),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(admitted.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
        "applied",
      ])

      const exactRetry = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(5, RUNTIME_START_TURN_MUTATOR_NAME, startIntent),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(exactRetry.results[0]!.status).toBe("applied")

      const conflict = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(6, RUNTIME_START_TURN_MUTATOR_NAME, {
            ...startIntent,
            bodyRef: "chat_message.different",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(conflict.results[0]!.status).toBe("rejected")
      expect(conflict.results[0]!.errorCode).toBe("runtime_intent_conflict")

      await sql`
        UPDATE khala_sync_chat_threads
        SET repo_binding_owner = 'DifferentOrg',
            repo_binding_name = 'different-repo',
            repo_binding_default_branch = 'other'
        WHERE thread_id = ${threadId}
      `

      const streamed = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(7, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.conversation.started",
            kind: "turn.started",
            providerRef: "provider.codex.named",
            sequence: 0,
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(streamed.results[0]!.status).toBe("applied")

      const runLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 50,
        scope: agentRunScope(turnId),
      })
      const threadLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 50,
        scope: threadScope(threadId),
      })
      for (const log of [runLog, threadLog]) {
        expect(log.entries.some(entry => String(entry.entityType) === AGENT_RUN_ENTITY_TYPE)).toBe(true)
        expect(log.entries.some(entry => String(entry.entityType) === AGENT_RUN_EVENT_ENTITY_TYPE)).toBe(true)
      }
      const runPostImage = runLog.entries
        .filter(entry => String(entry.entityType) === AGENT_RUN_ENTITY_TYPE)
        .at(-1)?.postImageJson
      expect(runPostImage).toContain(`\"runId\":\"${turnId}\"`)
      expect(runPostImage).toContain(`\"routeId\":\"${threadId}\"`)
      expect(runPostImage).toContain("Start the real streamed conversation.")
      expect(runPostImage).toContain("OpenAgentsInc")
      expect(runPostImage).not.toContain("DifferentOrg")
      const graphJson = threadLog.entries
        .filter(entry => String(entry.entityType) === LIVE_AGENT_GRAPH_ENTITY_TYPE)
        .at(-1)?.postImageJson
      expect(graphJson).toBeDefined()
      const graph = decodeLiveAgentGraphPostImageJson(graphJson!)
      expect(graph.cursor).toBe(1)
      expect(graph.nodes[0]).toMatchObject({
        provider: { state: "known", kind: "codex", providerRef: "provider.codex.named" },
        runtime: { state: "known", kind: "codex_app_server" },
        status: "running",
      })
      const counts: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count
        FROM khala_sync_runtime_control_intents
        WHERE intent_id = ${startIntent.intentId}
      `
      expect(Number(counts[0]!.count)).toBe(1)
    })

    test("Claude runtime events converge through the same canonical graph writer", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.claude.graph.1"
      const turnId = "runtime-turn.claude.graph.1"
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.claude.graph.start",
            kind: "turn.start",
            lane: "claude_pylon",
            promptRef: "prompt.claude.graph",
            threadId,
            turnId,
          })),
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.claude.graph.started",
            kind: "turn.started",
            lane: "claude_pylon",
            providerRef: "provider.claude.named",
            sequence: 0,
            threadId,
            turnId,
          })),
          envelope(3, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.claude.graph.child.started",
            kind: "agent.child.started",
            lane: "claude_pylon",
            providerRef: "provider.claude.named",
            sequence: 1,
            threadId,
            turnId,
            childAgentId: "child.claude.task.1",
            childRunId: "run.child.claude.task.1",
            parentAgentId: turnId,
            taskRef: "task.claude.tool.1",
          })),
          envelope(4, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.claude.graph.child.progress",
            kind: "agent.child.progress",
            lane: "claude_pylon",
            providerRef: "provider.claude.named",
            sequence: 2,
            threadId,
            turnId,
            childAgentId: "child.claude.task.1",
            childRunId: "run.child.claude.task.1",
            parentAgentId: turnId,
            taskRef: "task.claude.tool.1",
          })),
          envelope(5, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.claude.graph.child.finished",
            kind: "agent.child.finished",
            lane: "claude_pylon",
            providerRef: "provider.claude.named",
            sequence: 3,
            threadId,
            turnId,
            childAgentId: "child.claude.task.1",
            childRunId: "run.child.claude.task.1",
            parentAgentId: turnId,
            taskRef: "task.claude.tool.1",
            finishReason: "stop",
          })),
          envelope(6, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.claude.graph.finished",
            kind: "turn.finished",
            lane: "claude_pylon",
            providerRef: "provider.claude.named",
            sequence: 4,
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
        "applied",
        "applied",
        "applied",
      ])
      const log = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 50,
        scope: threadScope(threadId),
      })
      const graphJson = log.entries
        .filter(entry => String(entry.entityType) === LIVE_AGENT_GRAPH_ENTITY_TYPE)
        .at(-1)?.postImageJson
      expect(graphJson).toBeDefined()
      const graph = decodeLiveAgentGraphPostImageJson(graphJson!)
      expect(graph).toMatchObject({ cursor: 5, threadRef: "runtime-thread.claude.graph.1" })
      expect(graph.nodes).toHaveLength(2)
      expect(graph.nodes[0]).toMatchObject({
        provider: { state: "known", kind: "claude", providerRef: "provider.claude.named" },
        runtime: { state: "known", kind: "claude_agent_sdk" },
        status: "completed",
        terminal: { state: "terminal", reason: "completed" },
      })
      expect(graph.nodes[1]).toMatchObject({
        agentRef: "agent.claude.child.claude.task.1",
        runRef: "run.claude.run.child.claude.task.1",
        parent: { kind: "agent", agentRef: `agent.claude.${turnId}` },
        status: "completed",
        terminal: { state: "terminal", reason: "completed" },
        activityCursor: 3,
        version: 3,
      })
      expect(graph.edges).toContainEqual(expect.objectContaining({
        kind: "parent",
        fromAgentRef: `agent.claude.${turnId}`,
        toAgentRef: "agent.claude.child.claude.task.1",
        version: 1,
      }))

      const retried = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(7, RUNTIME_RETRY_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.claude.graph.retry",
            kind: "turn.retry",
            lane: "claude_pylon",
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(retried.results[0]!.status).toBe("applied")
      const retryLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 50,
        scope: threadScope(threadId),
      })
      const retryGraph = decodeLiveAgentGraphPostImageJson(retryLog.entries
        .filter(entry => String(entry.entityType) === LIVE_AGENT_GRAPH_ENTITY_TYPE)
        .at(-1)!.postImageJson!)
      expect(retryGraph.attachmentGeneration).toBe(2)
      expect(retryGraph.cursor).toBe(6)
      expect(retryGraph.nodes).toHaveLength(1)
      expect(retryGraph.nodes[0]).toMatchObject({
        agentRef: `agent.claude.${turnId}.g2`,
        status: "queued",
        terminal: { state: "active" },
        version: 1,
      })
    })

    test("offline expiry is a durable terminal outcome and can never create or dispatch a turn", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.expired.8687"
      const turnId = "runtime-turn.expired.8687"
      const expiredIntent = controlIntent({
        expiresAt: "2000-01-01T00:00:00.000Z",
        intentId: "runtime-intent.expired.8687",
        kind: "turn.start",
        promptRef: "prompt.expired.8687",
        threadId,
        turnId,
      })
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
            threadId,
            title: "Expired offline command",
          }),
          envelope(2, RUNTIME_START_TURN_MUTATOR_NAME, expiredIntent),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
      ])

      const retry = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(3, RUNTIME_START_TURN_MUTATOR_NAME, expiredIntent),
          envelope(4, RUNTIME_START_TURN_MUTATOR_NAME, {
            ...expiredIntent,
            promptRef: "prompt.conflicting.8687",
          }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(retry.results[0]!.status).toBe("applied")
      expect(retry.results[1]!.status).toBe("rejected")
      expect(retry.results[1]!.errorCode).toBe("runtime_intent_conflict")

      const rows: Array<{ status: string; intent_count: number; turn_count: number }> =
        await sql`
          SELECT max(status) AS status,
                 count(*)::int AS intent_count,
                 (SELECT count(*)::int FROM khala_sync_runtime_turns
                  WHERE turn_id = ${turnId}) AS turn_count
          FROM khala_sync_runtime_control_intents
          WHERE intent_id = ${expiredIntent.intentId}
          GROUP BY intent_id
        `
      expect(rows).toEqual([{ status: "expired", intent_count: 1, turn_count: 0 }])
      expect(await readPendingRuntimeControlIntents(sql as unknown as SyncSql, {
        afterSeq: 0,
        ownerUserId: client.userId,
      })).toEqual([])

      for (const scope of [personalScope(client.userId), threadScope(threadId)]) {
        const log = await logPage(sql as unknown as SyncSql, {
          afterVersion: null,
          limit: 50,
          scope,
        })
        const projected = log.entries.find(entry =>
          String(entry.entityType) === RUNTIME_CONTROL_INTENT_ENTITY_TYPE &&
          String(entry.entityId) === expiredIntent.intentId)
        expect(projected?.postImageJson).toContain('"status":"expired"')
        expect(projected?.postImageJson).toContain(`"intentId":"${expiredIntent.intentId}"`)
      }
    })

    test("invalid expiry rejects without recording a command", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.expiry-invalid.8687"
      const invalid = controlIntent({
        expiresAt: "not-a-time",
        intentId: "runtime-intent.expiry-invalid.8687",
        kind: "turn.start",
        threadId,
        turnId: "runtime-turn.expiry-invalid.8687",
      })
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, { threadId, title: "Invalid" }),
          envelope(2, RUNTIME_START_TURN_MUTATOR_NAME, invalid),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(response.results[1]!.status).toBe("rejected")
      expect(response.results[1]!.errorCode).toBe(RUNTIME_INTENT_EXPIRY_REJECTION)
      const rows: Array<{ count: number }> = await sql`
        SELECT count(*)::int AS count
        FROM khala_sync_runtime_control_intents
        WHERE intent_id = ${invalid.intentId}
      `
      expect(rows[0]!.count).toBe(0)
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
              sequence: 0,
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
              sequence: 1,
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

    test("existing-turn controls require the durable provider lane before any mutation", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.lane-fence.8696"
      const turnId = "runtime-turn.lane-fence.8696"
      const started = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.lane-fence.start.8696",
            kind: "turn.start",
            lane: "claude_pylon",
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(started.results[0]!.status).toBe("applied")

      const mismatched = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(2, RUNTIME_INTERRUPT_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.lane-fence.wrong.8696",
            kind: "turn.interrupt",
            lane: "codex_app_server",
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(mismatched.results[0]!.status).toBe("rejected")
      expect(mismatched.results[0]!.errorCode).toBe(
        RUNTIME_TARGET_LANE_REJECTION,
      )
      const unchanged: Array<{
        intent_count: string | number
        status: string
      }> = await sql`
        SELECT
          (SELECT count(*) FROM khala_sync_runtime_control_intents
           WHERE thread_id = ${threadId}) AS intent_count,
          status
        FROM khala_sync_runtime_turns
        WHERE turn_id = ${turnId}
      `
      expect(Number(unchanged[0]!.intent_count)).toBe(1)
      expect(unchanged[0]!.status).toBe("queued")

      const matched = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(3, RUNTIME_INTERRUPT_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.lane-fence.correct.8696",
            kind: "turn.interrupt",
            lane: "claude_pylon",
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(matched.results[0]!.status).toBe("applied")
      const final: Array<{ intent_count: string | number; status: string }> =
        await sql`
          SELECT
            (SELECT count(*) FROM khala_sync_runtime_control_intents
             WHERE thread_id = ${threadId}) AS intent_count,
            status
          FROM khala_sync_runtime_turns
          WHERE turn_id = ${turnId}
        `
      expect(Number(final[0]!.intent_count)).toBe(2)
      expect(final[0]!.status).toBe("interrupted")
    })

    test("runtime interactions request, resolve, and reconcile exact semantic retries in the private thread", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.interaction.8696"
      const turnId = "runtime-turn.interaction.8696"
      const interactionRef = "interaction.runtime.question.8696"
      const interaction = runtimeQuestionInteraction({
        interactionRef,
        lane: "claude_pylon",
        requestedSequence: 1,
        threadId,
        turnId,
      })
      const admitted = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.interaction.start.8696",
            kind: "turn.start",
            lane: "claude_pylon",
            threadId,
            turnId,
          })),
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.interaction.started.8696",
            kind: "turn.started",
            lane: "claude_pylon",
            sequence: 0,
            threadId,
            turnId,
          })),
          envelope(3, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, interaction),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(admitted.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
      ])

      const decision = runtimeQuestionDecision({
        interactionRef,
        threadId,
        turnId,
      })
      const reconciled = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(4, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, interaction),
          envelope(5, RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME, decision),
          envelope(6, RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME, decision),
          envelope(7, RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME, {
            ...decision,
            envelope: {
              ...decision.envelope,
              decision: {
                kind: "provider_question",
                answers: [{
                  questionRef: "question.runtime.1",
                  optionRefs: ["option.smoke"],
                }],
              },
            },
          }),
          envelope(8, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, interaction),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })
      expect(reconciled.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
        "rejected",
        "applied",
      ])
      expect(reconciled.results[3]!.errorCode).toBe(
        RUNTIME_INTERACTION_CONFLICT_REJECTION,
      )

      const rows: Array<{ count: string | number; status: string }> = await sql`
        SELECT count(*) OVER () AS count, status
        FROM khala_sync_runtime_interactions
        WHERE interaction_ref = ${interactionRef}
      `
      expect(Number(rows[0]!.count)).toBe(1)
      expect(rows[0]!.status).toBe("resolved")
      const threadLog = await logPage(sql as unknown as SyncSql, {
        afterVersion: null,
        limit: 100,
        scope: threadScope(threadId),
      })
      expect(threadLog.entries.filter(entry =>
        String(entry.entityType) === RUNTIME_INTERACTION_ENTITY_TYPE
      )).toHaveLength(2)
      const personalRows: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count
        FROM khala_sync_changelog
        WHERE entity_type = ${RUNTIME_INTERACTION_ENTITY_TYPE}
          AND scope <> ${threadScope(threadId)}
      `
      expect(Number(personalRows[0]!.count)).toBe(0)
    })

    test("runtime interaction admission fences lane, sequence, choices, expiry, and foreign owners", async () => {
      const owner = freshClient()
      const intruder = freshClient()
      const threadId = "runtime-thread.interaction-fence.8696"
      const turnId = "runtime-turn.interaction-fence.8696"
      await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(1, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.interaction-fence.start.8696",
            kind: "turn.start",
            lane: "claude_pylon",
            threadId,
            turnId,
          })),
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.interaction-fence.started.8696",
            kind: "turn.started",
            lane: "claude_pylon",
            sequence: 0,
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })

      const wrongLane = runtimeQuestionInteraction({
        interactionRef: "interaction.runtime.wrong-lane.8696",
        lane: "codex_app_server",
        requestedSequence: 1,
        threadId,
        turnId,
      })
      const wrongSequence = runtimeQuestionInteraction({
        interactionRef: "interaction.runtime.wrong-sequence.8696",
        lane: "claude_pylon",
        requestedSequence: 2,
        threadId,
        turnId,
      })
      const interactionRef = "interaction.runtime.expiry.8696"
      const valid = runtimeQuestionInteraction({
        interactionRef,
        lane: "claude_pylon",
        requestedSequence: 1,
        threadId,
        turnId,
      })
      const fenced = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(3, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, wrongLane),
          envelope(4, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, wrongSequence),
          envelope(5, RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME, valid),
          envelope(6, RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME,
            runtimeQuestionDecision({
              interactionRef,
              optionRef: "option.unknown",
              threadId,
              turnId,
            })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(fenced.results.map(result => result.status)).toEqual([
        "rejected",
        "rejected",
        "applied",
        "rejected",
      ])
      expect(fenced.results[0]!.errorCode).toBe(RUNTIME_TARGET_LANE_REJECTION)
      expect(fenced.results[1]!.errorCode).toBe(
        RUNTIME_INTERACTION_SEQUENCE_REJECTION,
      )
      expect(fenced.results[3]!.errorCode).toBe(
        RUNTIME_INTERACTION_DECISION_REJECTION,
      )

      const tooEarly = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(7, RUNTIME_EXPIRE_INTERACTION_MUTATOR_NAME, { interactionRef, threadId, turnId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(tooEarly.results[0]!.status).toBe("rejected")
      expect(tooEarly.results[0]!.errorCode).toBe(RUNTIME_INTERACTION_EXPIRY_REJECTION)

      const expired = {
        ...valid,
        expiresAt: "2000-01-01T00:00:00.000Z",
      }
      await sql`
        UPDATE khala_sync_runtime_interactions
        SET expires_at = ${expired.expiresAt},
            interaction_json = ${expired}::jsonb
        WHERE interaction_ref = ${interactionRef}
      `
      const late = await executePush({
        registry,
        request: pushRequest(owner, [
          envelope(8, RUNTIME_EXPIRE_INTERACTION_MUTATOR_NAME,
            { interactionRef, threadId, turnId }),
        ]),
        sql: sql as unknown as SyncSql,
        userId: owner.userId,
      })
      expect(late.results[0]!.status).toBe("applied")
      const terminal: Array<{ status: string }> = await sql`
        SELECT status FROM khala_sync_runtime_interactions
        WHERE interaction_ref = ${interactionRef}
      `
      expect(terminal[0]!.status).toBe("expired")

      const foreign = await executePush({
        registry,
        request: pushRequest(intruder, [
          envelope(1, RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME,
            runtimeQuestionDecision({ interactionRef, threadId, turnId })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: intruder.userId,
      })
      expect(foreign.results[0]!.status).toBe("rejected")
      expect(foreign.results[0]!.errorCode).toBe(RUNTIME_SCOPE_REJECTION)
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

    test("concurrent duplicate delivery serializes before commit and creates one dispatchable intent", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.concurrent-duplicate.8687"
      const turnId = "runtime-turn.concurrent-duplicate.8687"
      const request = pushRequest(client, [
        envelope(1, CHAT_CREATE_THREAD_MUTATOR_NAME, {
          threadId,
          title: "Concurrent duplicate",
        }),
        envelope(2, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
          intentId: "runtime-intent.concurrent-duplicate.8687",
          kind: "turn.start",
          promptRef: "prompt.concurrent-duplicate.8687",
          threadId,
          turnId,
        })),
      ])

      const responses = await Promise.all([
        executePush({ registry, request, sql: sql as unknown as SyncSql, userId: client.userId }),
        executePush({ registry, request, sql: sql as unknown as SyncSql, userId: client.userId }),
      ])
      expect(responses
        .map(response => response.results.map(result => result.status).join(","))
        .sort()).toEqual([
          "applied,applied",
          "duplicate,duplicate",
        ])
      const rows: Array<{ intents: number; turns: number }> = await sql`
        SELECT
          (SELECT count(*)::int FROM khala_sync_runtime_control_intents
           WHERE intent_id = 'runtime-intent.concurrent-duplicate.8687') AS intents,
          (SELECT count(*)::int FROM khala_sync_runtime_turns
           WHERE turn_id = ${turnId}) AS turns
      `
      expect(rows).toEqual([{ intents: 1, turns: 1 }])
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
          envelope(
            2,
            RUNTIME_RECORD_EVENT_MUTATOR_NAME,
            runtimeEvent({
              eventId: "runtime-event.event-duplicate.started",
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
          envelope(3, RUNTIME_RECORD_EVENT_MUTATOR_NAME, duplicateEvent),
          envelope(4, RUNTIME_RECORD_EVENT_MUTATOR_NAME, duplicateEvent),
          envelope(
            5,
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
      expect(Number(response.lastMutationId)).toBe(5)

      const events: Array<{ count: string | number }> = await sql`
        SELECT count(*) AS count FROM khala_sync_runtime_events
        WHERE turn_id = ${turnId}
      `
      expect(Number(events[0]!.count)).toBe(2)
    })

    test("interrupt fences stale worker events and the durable sequence refuses gaps", async () => {
      const client = freshClient()
      const threadId = "runtime-thread.generation-fence.8689"
      const turnId = "runtime-turn.generation-fence.8689"
      const response = await executePush({
        registry,
        request: pushRequest(client, [
          envelope(1, RUNTIME_START_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.generation-fence.start.8689",
            kind: "turn.start",
            threadId,
            turnId,
          })),
          envelope(2, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.generation-fence.started.8689",
            kind: "turn.started",
            sequence: 0,
            threadId,
            turnId,
          })),
          envelope(3, RUNTIME_INTERRUPT_TURN_MUTATOR_NAME, controlIntent({
            intentId: "runtime-intent.generation-fence.interrupt.8689",
            kind: "turn.interrupt",
            reasonRef: "reason.authority_revoked",
            threadId,
            turnId,
          })),
          envelope(4, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.generation-fence.late-text.8689",
            kind: "text.delta",
            sequence: 1,
            text: "stale provider output",
            threadId,
            turnId,
          })),
          envelope(5, RUNTIME_RECORD_EVENT_MUTATOR_NAME, runtimeEvent({
            eventId: "runtime-event.generation-fence.gap.8689",
            kind: "text.delta",
            sequence: 3,
            text: "out-of-order provider output",
            threadId,
            turnId,
          })),
        ]),
        sql: sql as unknown as SyncSql,
        userId: client.userId,
      })

      expect(response.results.map(result => result.status)).toEqual([
        "applied",
        "applied",
        "applied",
        "rejected",
        "rejected",
      ])
      expect(response.results[3]!.errorCode).toBe(RUNTIME_EVENT_STATE_REJECTION)
      expect(response.results[4]!.errorCode).toBe(RUNTIME_EVENT_SEQUENCE_REJECTION)
      const rows: Array<{ event_count: number; status: string; events: number }> = await sql`
        SELECT t.event_count::int AS event_count,
               t.status,
               (SELECT count(*)::int FROM khala_sync_runtime_events e
                WHERE e.turn_id = t.turn_id) AS events
        FROM khala_sync_runtime_turns t
        WHERE t.turn_id = ${turnId}
      `
      expect(rows).toEqual([{ event_count: 1, events: 1, status: "interrupted" }])
    })
  },
)
