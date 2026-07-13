import type { PortableAgentGraph } from "@openagentsinc/portable-session-contract"

import {
  appendPortableSessionEvent,
  readPortableSessionAuthoritySnapshot,
} from "./portable-session-authority.js"
import type { SyncTransactionWriter } from "./outbox-writer.js"
import type { SyncSql } from "./sql.js"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const LOOPBACK_HOSTS = new Set(["127.0.0.1", "localhost", "::1"])
const FORBIDDEN_RESPONSE =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|"(?:task|prompt|token|authorization|password|secret|credential|path|hostname|processId|pid|resourceRef)"\s*:/iu

type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>

export type PortableManagedContinuationPlan = Readonly<{
  operationRef: string
  providerLeaseRef: string
  turns: ReadonlyArray<Readonly<{
    agentRef: string
    turnRef: string
    task: string
  }>>
}>

export type PortableManagedContinuationInput = Readonly<{
  sessionRef: string
  attachmentRef: string
  generation: number
  expectedGraph: PortableAgentGraph
  expectedThreadCursors: ReadonlyArray<Readonly<{
    agentRef: string
    threadRef: string
    activityCursor: number
    eventCursor: number
  }>>
  plan: PortableManagedContinuationPlan
}>

export type PortableManagedContinuationReceipt = Readonly<{
  acceptedWorkRefs: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string }>>
  threadCursors: ReadonlyArray<Readonly<{
    agentRef: string
    threadRef: string
    activityCursor: number
    eventCursor: number
  }>>
  evidenceRefs: ReadonlyArray<string>
  replay: "executed" | "replayed"
}>

export type PortableManagedContinuation = Readonly<{
  run: (input: PortableManagedContinuationInput) => Promise<PortableManagedContinuationReceipt>
}>

export type PortableManagedContinuationAuthority = Readonly<{
  readExpectedCursors: (input: Readonly<{
    ownerRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    expectedGraph: PortableAgentGraph
  }>) => Promise<PortableManagedContinuationInput["expectedThreadCursors"]>
  commit: (input: Readonly<{
    ownerRef: string
    sessionRef: string
    attachmentRef: string
    generation: number
    expectedGraph: PortableAgentGraph
    expectedThreadCursors: PortableManagedContinuationInput["expectedThreadCursors"]
    plan: PortableManagedContinuationPlan
    receipt: PortableManagedContinuationReceipt
  }>) => Promise<void>
}>

export class PortableManagedContinuationError extends Error {
  readonly _tag = "PortableManagedContinuationError"
  override readonly name = "PortableManagedContinuationError"

  constructor(
    readonly code: "invalid" | "unavailable" | "rejected" | "unsafe_response",
    message: string,
  ) {
    super(message)
  }
}

const refs = (values: ReadonlyArray<string>, field: string): void => {
  if (values.length !== new Set(values).size || values.some(value => !SAFE_REF.test(value))) {
    throw new PortableManagedContinuationError("invalid", `${field} must contain unique public-safe refs`)
  }
}

const record = (value: unknown): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new PortableManagedContinuationError("rejected", "managed continuation returned an invalid response")
  }
  return value as Record<string, unknown>
}

const exactKeys = (value: Record<string, unknown>, expected: ReadonlyArray<string>): void => {
  const actual = Object.keys(value).sort()
  const sorted = [...expected].sort()
  if (actual.length !== sorted.length || actual.some((key, index) => key !== sorted[index])) {
    throw new PortableManagedContinuationError("unsafe_response", "managed continuation returned unexpected fields")
  }
}

export const createOaCodexControlPortableManagedContinuation = (config: Readonly<{
  baseUrl: string
  bearerToken: string
  ownerRef: string
  targetRef: string
  fetch?: FetchLike
  timeoutMs?: number
}>): PortableManagedContinuation => {
  let endpoint: URL
  try {
    endpoint = new URL("/v1/portable-agent-computers/continuations", config.baseUrl)
  } catch {
    throw new PortableManagedContinuationError("invalid", "managed continuation base URL is invalid")
  }
  if (endpoint.protocol !== "https:" && !LOOPBACK_HOSTS.has(endpoint.hostname)) {
    throw new PortableManagedContinuationError("invalid", "managed continuation requires HTTPS or authenticated loopback HTTP")
  }
  if (config.bearerToken.length < 16 || !SAFE_REF.test(config.ownerRef) || !SAFE_REF.test(config.targetRef)) {
    throw new PortableManagedContinuationError("invalid", "managed continuation authority is invalid")
  }
  const fetcher = config.fetch ?? globalThis.fetch
  const timeoutMs = config.timeoutMs ?? 120_000

  return {
    run: async input => {
      refs([
        input.sessionRef,
        input.attachmentRef,
        input.plan.operationRef,
        input.plan.providerLeaseRef,
      ], "continuation binding refs")
      if (!Number.isSafeInteger(input.generation) || input.generation <= 0 ||
          input.plan.turns.length === 0 || input.plan.turns.length > 64) {
        throw new PortableManagedContinuationError("invalid", "managed continuation generation or turn count is invalid")
      }
      const expected = new Map(input.expectedGraph.nodes.map(node => [node.agentRef, node]))
      const expectedCursors = new Map(input.expectedThreadCursors.map(row => [row.agentRef, row]))
      refs(input.expectedGraph.nodes.map(node => node.agentRef), "continuation graph agents")
      refs(input.expectedGraph.nodes.map(node => node.threadRef), "continuation graph threads")
      refs(input.plan.turns.map(turn => turn.agentRef), "continuation turn agents")
      refs(input.plan.turns.map(turn => turn.turnRef), "continuation turn refs")
      refs(input.expectedThreadCursors.map(row => row.agentRef), "expected cursor agents")
      refs(input.expectedThreadCursors.map(row => row.threadRef), "expected cursor threads")
      if (input.plan.turns.length !== expected.size || expectedCursors.size !== expected.size ||
          input.expectedThreadCursors.some(row => expected.get(row.agentRef)?.threadRef !== row.threadRef ||
            !Number.isSafeInteger(row.activityCursor) || !Number.isSafeInteger(row.eventCursor) ||
            row.activityCursor < 0 || row.eventCursor < 0) ||
          input.plan.turns.some(turn => !expected.has(turn.agentRef) ||
            turn.task.length === 0 || Buffer.byteLength(turn.task, "utf8") > 16 * 1024)) {
        throw new PortableManagedContinuationError("invalid", "continuation turns do not cover the exact bounded graph")
      }
      const request = {
        operationRef: input.plan.operationRef,
        ownerRef: config.ownerRef,
        targetRef: config.targetRef,
        sessionRef: input.sessionRef,
        attachmentRef: input.attachmentRef,
        generation: input.generation,
        providerLeaseRef: input.plan.providerLeaseRef,
        expectedThreadCursors: input.expectedThreadCursors,
        turns: input.plan.turns,
      }
      const controller = new AbortController()
      const timeout = setTimeout(() => controller.abort(), timeoutMs)
      let response: Response
      try {
        response = await fetcher(endpoint, {
          method: "POST",
          headers: {
            authorization: `Bearer ${config.bearerToken}`,
            "content-type": "application/json",
          },
          body: JSON.stringify(request),
          signal: controller.signal,
        })
      } catch {
        throw new PortableManagedContinuationError("unavailable", "managed continuation is unavailable")
      } finally {
        clearTimeout(timeout)
      }
      let decoded: unknown
      try {
        decoded = await response.json()
      } catch {
        throw new PortableManagedContinuationError("rejected", "managed continuation returned invalid JSON")
      }
      if (!response.ok) {
        throw new PortableManagedContinuationError("rejected", `managed continuation refused work (${response.status})`)
      }
      const envelope = record(decoded)
      if (FORBIDDEN_RESPONSE.test(JSON.stringify(envelope))) {
        throw new PortableManagedContinuationError("unsafe_response", "managed continuation echoed private material")
      }
      exactKeys(envelope, ["acceptedWorkRefs", "threadCursors", "evidenceRefs", "replay", "material"])
      if (!Array.isArray(envelope.acceptedWorkRefs) || !Array.isArray(envelope.threadCursors) ||
          !Array.isArray(envelope.evidenceRefs) ||
          !["executed", "replayed"].includes(String(envelope.replay)) || envelope.material !== "excluded") {
        throw new PortableManagedContinuationError("rejected", "managed continuation response shape is invalid")
      }
      const acceptedWorkRefs = envelope.acceptedWorkRefs.map(value => {
        const row = record(value)
        exactKeys(row, ["agentRef", "turnRef"])
        return { agentRef: String(row.agentRef), turnRef: String(row.turnRef) }
      })
      const plannedPairs = input.plan.turns.map(turn => `${turn.agentRef}:${turn.turnRef}`).sort()
      const acceptedPairs = acceptedWorkRefs.map(turn => `${turn.agentRef}:${turn.turnRef}`).sort()
      refs(acceptedWorkRefs.map(row => row.agentRef), "accepted agents")
      refs(acceptedWorkRefs.map(row => row.turnRef), "accepted turns")
      if (acceptedPairs.length !== plannedPairs.length || acceptedPairs.some((pair, index) => pair !== plannedPairs[index])) {
        throw new PortableManagedContinuationError("rejected", "managed continuation accepted different turns")
      }
      const threadCursors = envelope.threadCursors.map(value => {
        const row = record(value)
        exactKeys(row, ["agentRef", "threadRef", "activityCursor", "eventCursor"])
        return {
          agentRef: String(row.agentRef),
          threadRef: String(row.threadRef),
          activityCursor: Number(row.activityCursor),
          eventCursor: Number(row.eventCursor),
        }
      })
      refs(threadCursors.map(row => row.agentRef), "continuation cursor agents")
      refs(threadCursors.map(row => row.threadRef), "continuation cursor threads")
      if (threadCursors.length !== expected.size || threadCursors.some(row => {
        const node = expected.get(row.agentRef)
        const prior = expectedCursors.get(row.agentRef)
        return node === undefined || node.threadRef !== row.threadRef ||
          prior === undefined || prior.threadRef !== row.threadRef ||
          !Number.isSafeInteger(row.activityCursor) || !Number.isSafeInteger(row.eventCursor) ||
          row.activityCursor <= prior.activityCursor || row.eventCursor !== prior.eventCursor + 1
      })) {
        throw new PortableManagedContinuationError("rejected", "managed continuation cursors do not advance the exact graph")
      }
      const evidenceRefs = envelope.evidenceRefs.map(String)
      refs(evidenceRefs, "continuation evidence")
      return {
        acceptedWorkRefs,
        threadCursors,
        evidenceRefs,
        replay: envelope.replay as "executed" | "replayed",
      }
    },
  }
}

const jsonRecord = (value: unknown): Record<string, unknown> => {
  const decoded = typeof value === "string" ? JSON.parse(value) : value
  return decoded !== null && typeof decoded === "object" && !Array.isArray(decoded)
    ? decoded as Record<string, unknown>
    : {}
}

export class PostgresPortableManagedContinuationAuthority
  implements PortableManagedContinuationAuthority
{
  constructor(private readonly config: Readonly<{
    sql: SyncSql
    transaction: <A>(run: (writer: SyncTransactionWriter) => Promise<A>) => Promise<A>
  }>) {}

  async readExpectedCursors(
    input: Parameters<PortableManagedContinuationAuthority["readExpectedCursors"]>[0],
  ): Promise<PortableManagedContinuationInput["expectedThreadCursors"]> {
    const snapshot = await readPortableSessionAuthoritySnapshot(this.config.sql, {
      sessionRef: input.sessionRef,
      ownerUserId: input.ownerRef,
    })
    if (snapshot === null || snapshot.session.current_attachment_ref !== input.attachmentRef ||
        Number(snapshot.session.current_attachment_generation) !== input.generation) {
      throw new PortableManagedContinuationError("rejected", "continuation authority is not the exact active attachment")
    }
    const agents = new Map(snapshot.agents.map(row => [String(row.agent_ref), row]))
    const current = new Map(snapshot.current.map(row => [String(row.thread_ref), row]))
    return input.expectedGraph.nodes.map(node => {
      const agent = agents.get(node.agentRef)
      const thread = current.get(node.threadRef)
      const state = jsonRecord(thread?.current_json)
      if (agent === undefined || thread === undefined ||
          agent.thread_ref !== node.threadRef || agent.transcript_ref !== node.transcriptRef ||
          Number(agent.attachment_generation) !== input.generation ||
          Number(agent.activity_cursor) !== node.activityCursor) {
        throw new PortableManagedContinuationError("rejected", "continuation authority graph or cursor is stale")
      }
      const activityCursor = Number(state.activityCursor ?? agent.activity_cursor)
      const eventCursor = Number(thread.latest_cursor)
      if (!Number.isSafeInteger(activityCursor) || !Number.isSafeInteger(eventCursor) ||
          activityCursor < 0 || eventCursor < 0) {
        throw new PortableManagedContinuationError("rejected", "continuation authority cursor is invalid")
      }
      return { agentRef: node.agentRef, threadRef: node.threadRef, activityCursor, eventCursor }
    })
  }

  async commit(input: Parameters<PortableManagedContinuationAuthority["commit"]>[0]): Promise<void> {
    const turns = new Map(input.plan.turns.map(row => [row.agentRef, row]))
    const prior = new Map(input.expectedThreadCursors.map(row => [row.agentRef, row]))
    const cursors = new Map(input.receipt.threadCursors.map(row => [row.agentRef, row]))
    await this.config.transaction(async writer => {
      for (const node of input.expectedGraph.nodes) {
        const turn = turns.get(node.agentRef)
        const before = prior.get(node.agentRef)
        const after = cursors.get(node.agentRef)
        if (turn === undefined || before === undefined || after === undefined ||
            after.threadRef !== node.threadRef || after.activityCursor <= before.activityCursor ||
            after.eventCursor !== before.eventCursor + 1) {
          throw new PortableManagedContinuationError("rejected", "continuation commit does not cover the exact graph")
        }
        const eventRef = `event.${input.plan.operationRef}.${node.agentRef}`
        const existing: ReadonlyArray<Record<string, unknown>> = await writer.sql`
          SELECT thread_ref, thread_cursor, attachment_ref, attachment_generation, event_json
          FROM khala_sync_portable_events
          WHERE session_ref = ${input.sessionRef} AND event_ref = ${eventRef}
        `
        const current = {
          lifecycle: "waiting",
          activityCursor: after.activityCursor,
          turnRef: turn.turnRef,
        }
        if (existing[0] !== undefined) {
          if (existing[0].thread_ref !== node.threadRef ||
              Number(existing[0].thread_cursor) !== after.eventCursor ||
              existing[0].attachment_ref !== input.attachmentRef ||
              Number(existing[0].attachment_generation) !== input.generation ||
              jsonRecord(existing[0].event_json).lifecycle !== current.lifecycle ||
              Number(jsonRecord(existing[0].event_json).activityCursor) !== current.activityCursor ||
              jsonRecord(existing[0].event_json).turnRef !== current.turnRef) {
            throw new PortableManagedContinuationError("rejected", "continuation event replay conflicts with authority")
          }
          continue
        }
        await appendPortableSessionEvent(writer, {
          eventRef,
          sessionRef: input.sessionRef,
          threadRef: node.threadRef,
          threadCursor: after.eventCursor,
          attachmentRef: input.attachmentRef,
          attachmentGeneration: input.generation,
          eventKind: "activity_cursor",
          current,
        }, `mutation.${input.plan.operationRef}.${node.agentRef}`)
        const updated: ReadonlyArray<Record<string, unknown>> = await writer.sql`
          UPDATE khala_sync_portable_agent_nodes
          SET activity_cursor = ${after.activityCursor}, lifecycle = 'waiting',
              attachment_generation = ${input.generation}
          WHERE session_ref = ${input.sessionRef}
            AND agent_ref = ${node.agentRef}
            AND thread_ref = ${node.threadRef}
            AND attachment_generation = ${input.generation}
            AND activity_cursor = ${before.activityCursor}
          RETURNING agent_ref
        `
        if (updated.length !== 1) {
          throw new PortableManagedContinuationError("rejected", "continuation agent cursor update conflicted")
        }
      }
    })
  }
}
