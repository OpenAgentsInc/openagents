import { createHash } from "node:crypto"
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
  }>) => Promise<PortableManagedContinuationReceipt>
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

const continuationEventRef = (
  operationScope: string,
  agentRef: string,
  phase: "running" | "settled",
): string => `event.portable.continuation.${createHash("sha256")
  .update(`${operationScope}:${agentRef}:${phase}`)
  .digest("hex")
  .slice(0, 32)}`

const continuationMutationRef = (
  operationScope: string,
  agentRef: string,
  phase: "running" | "settled",
): string => `mutation.portable.continuation.${createHash("sha256")
  .update(`${operationScope}:${agentRef}:${phase}`)
  .digest("hex")
  .slice(0, 32)}`

const continuationEvidenceSetRef = (evidenceRefs: ReadonlyArray<string>): string =>
  `evidence.portable.continuation.set.${createHash("sha256")
    .update(JSON.stringify([...evidenceRefs].sort()))
    .digest("hex")
    .slice(0, 32)}`

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

  async commit(
    input: Parameters<PortableManagedContinuationAuthority["commit"]>[0],
  ): Promise<PortableManagedContinuationReceipt> {
    refs([input.ownerRef], "continuation commit owner")
    refs([input.sessionRef], "continuation commit session")
    refs([input.attachmentRef], "continuation commit attachment")
    refs([input.plan.operationRef], "continuation commit operation")
    refs([input.plan.providerLeaseRef], "continuation commit provider lease")
    if (!Number.isSafeInteger(input.generation) || input.generation <= 0) {
      throw new PortableManagedContinuationError("rejected", "continuation commit generation is invalid")
    }
    const turns = new Map(input.plan.turns.map(row => [row.agentRef, row]))
    const prior = new Map(input.expectedThreadCursors.map(row => [row.agentRef, row]))
    const cursors = new Map(input.receipt.threadCursors.map(row => [row.agentRef, row]))
    const accepted = new Map(input.receipt.acceptedWorkRefs.map(row => [row.agentRef, row]))
    refs(input.expectedGraph.nodes.map(node => node.agentRef), "continuation commit graph agents")
    refs(input.expectedGraph.nodes.map(node => node.threadRef), "continuation commit graph threads")
    refs(input.plan.turns.map(row => row.agentRef), "continuation commit plan agents")
    refs(input.plan.turns.map(row => row.turnRef), "continuation commit plan turns")
    refs(input.expectedThreadCursors.map(row => row.agentRef), "continuation commit prior agents")
    refs(input.receipt.threadCursors.map(row => row.agentRef), "continuation commit cursor agents")
    refs(input.receipt.acceptedWorkRefs.map(row => row.agentRef), "continuation commit accepted agents")
    refs(input.receipt.acceptedWorkRefs.map(row => row.turnRef), "continuation commit accepted turns")
    refs(input.receipt.evidenceRefs, "continuation commit evidence")
    if (turns.size !== input.expectedGraph.nodes.length || prior.size !== input.expectedGraph.nodes.length ||
        cursors.size !== input.expectedGraph.nodes.length || accepted.size !== input.expectedGraph.nodes.length) {
      throw new PortableManagedContinuationError("rejected", "continuation commit does not cover the exact graph")
    }
    const evidenceRefs = [...input.receipt.evidenceRefs].sort()
    if (evidenceRefs.length === 0) {
      throw new PortableManagedContinuationError("rejected", "continuation commit requires evidence")
    }
    const evidenceSetRef = continuationEvidenceSetRef(evidenceRefs)
    const operationScope = `${input.sessionRef}:${input.attachmentRef}:${input.generation}:${input.plan.operationRef}`
    const orderedNodes = [...input.expectedGraph.nodes].sort((left, right) =>
      left.agentRef.localeCompare(right.agentRef))
    const graphAgentRefs = new Set(orderedNodes.map(node => node.agentRef))
    if (orderedNodes.length === 0 || !graphAgentRefs.has(input.expectedGraph.rootAgentRef) ||
        orderedNodes.filter(node => node.parentAgentRef === undefined).length !== 1 ||
        orderedNodes.some(node => node.parentAgentRef !== undefined && !graphAgentRefs.has(node.parentAgentRef))) {
      throw new PortableManagedContinuationError("rejected", "continuation commit graph is invalid")
    }

    return this.config.transaction(async writer => {
      const sessions: ReadonlyArray<Record<string, unknown>> = await writer.sql`
        SELECT s.owner_user_id, s.root_agent_ref, s.current_attachment_ref, s.current_attachment_generation,
               a.state AS attachment_state
        FROM khala_sync_portable_sessions s
        JOIN khala_sync_portable_attachments a
          ON a.attachment_ref = s.current_attachment_ref
        WHERE s.session_ref = ${input.sessionRef}
        FOR UPDATE OF s, a
      `
      const session = sessions[0]
      if (session === undefined || session.owner_user_id !== input.ownerRef ||
          session.root_agent_ref !== input.expectedGraph.rootAgentRef ||
          session.current_attachment_ref !== input.attachmentRef ||
          Number(session.current_attachment_generation) !== input.generation ||
          session.attachment_state !== "active") {
        throw new PortableManagedContinuationError("rejected", "continuation commit attachment generation is stale")
      }

      const nodeRows: ReadonlyArray<Record<string, unknown>> = await writer.sql`
        SELECT agent_ref, parent_agent_ref, thread_ref, transcript_ref,
               activity_cursor, lifecycle, attachment_generation
        FROM khala_sync_portable_agent_nodes
        WHERE session_ref = ${input.sessionRef}
        ORDER BY agent_ref ASC
        FOR UPDATE
      `
      const storedNodes = new Map(nodeRows.map(row => [String(row.agent_ref), row]))
      if (storedNodes.size !== orderedNodes.length) {
        throw new PortableManagedContinuationError("rejected", "continuation commit graph is stale")
      }

      let replayed = true
      let durableAgentCount = 0
      for (const node of orderedNodes) {
        const turn = turns.get(node.agentRef)
        const before = prior.get(node.agentRef)
        const after = cursors.get(node.agentRef)
        const acceptedWork = accepted.get(node.agentRef)
        const storedNode = storedNodes.get(node.agentRef)
        if (turn === undefined || before === undefined || after === undefined ||
            acceptedWork === undefined || acceptedWork.turnRef !== turn.turnRef ||
            after.threadRef !== node.threadRef || after.activityCursor <= before.activityCursor ||
            after.eventCursor !== before.eventCursor + 1 ||
            before.threadRef !== node.threadRef ||
            storedNode === undefined || storedNode.thread_ref !== node.threadRef ||
            storedNode.transcript_ref !== node.transcriptRef ||
            (storedNode.parent_agent_ref ?? undefined) !== node.parentAgentRef ||
            Number(storedNode.attachment_generation) !== input.generation) {
          throw new PortableManagedContinuationError("rejected", "continuation commit does not cover the exact graph")
        }
        const runningEventRef = continuationEventRef(operationScope, node.agentRef, "running")
        const settledEventRef = continuationEventRef(operationScope, node.agentRef, "settled")
        const existing: ReadonlyArray<Record<string, unknown>> = await writer.sql`
          SELECT thread_ref, thread_cursor, attachment_ref, attachment_generation, event_json
          FROM khala_sync_portable_events
          WHERE session_ref = ${input.sessionRef}
            AND event_ref IN (${runningEventRef}, ${settledEventRef})
          ORDER BY thread_cursor ASC
        `
        if (existing.length === 0) {
          replayed = false
          continue
        }
        if (existing.length !== 2) {
          throw new PortableManagedContinuationError("rejected", "continuation commit has a partial durable replay")
        }
        durableAgentCount += 1
        const expectedStates = [
          {
            eventRef: runningEventRef,
            cursor: before.eventCursor + 1,
            lifecycle: "running",
            activityCursor: before.activityCursor,
            phase: "running",
          },
          {
            eventRef: settledEventRef,
            cursor: before.eventCursor + 2,
            lifecycle: "waiting",
            activityCursor: after.activityCursor,
            phase: "settled",
          },
        ] as const
        for (const [index, durable] of existing.entries()) {
          const expected = expectedStates[index]!
          const current = jsonRecord(durable.event_json)
          if (durable.thread_ref !== node.threadRef || Number(durable.thread_cursor) !== expected.cursor ||
              durable.attachment_ref !== input.attachmentRef ||
              Number(durable.attachment_generation) !== input.generation ||
              current.lifecycle !== expected.lifecycle ||
              Number(current.activityCursor) !== expected.activityCursor ||
              current.turnRef !== turn.turnRef || current.phase !== expected.phase ||
              current.evidenceRef !== evidenceSetRef) {
            throw new PortableManagedContinuationError("rejected", "continuation event replay conflicts with authority")
          }
        }
        if (Number(storedNode.activity_cursor) !== after.activityCursor || storedNode.lifecycle !== "waiting") {
          throw new PortableManagedContinuationError("rejected", "continuation replay conflicts with agent state")
        }
      }

      if (durableAgentCount !== 0 && durableAgentCount !== orderedNodes.length) {
        throw new PortableManagedContinuationError("rejected", "continuation commit has a partial graph replay")
      }

      if (replayed) {
        return {
          acceptedWorkRefs: orderedNodes.map(node => ({
            agentRef: node.agentRef,
            turnRef: turns.get(node.agentRef)!.turnRef,
          })),
          threadCursors: orderedNodes.map(node => ({
            agentRef: node.agentRef,
            threadRef: node.threadRef,
            activityCursor: cursors.get(node.agentRef)!.activityCursor,
            eventCursor: prior.get(node.agentRef)!.eventCursor + 2,
          })),
          evidenceRefs,
          replay: "replayed" as const,
        }
      }

      for (const node of orderedNodes) {
        const turn = turns.get(node.agentRef)!
        const before = prior.get(node.agentRef)!
        const after = cursors.get(node.agentRef)!
        const storedNode = storedNodes.get(node.agentRef)!
        if (Number(storedNode.activity_cursor) !== before.activityCursor ||
            !["waiting", "running"].includes(String(storedNode.lifecycle))) {
          throw new PortableManagedContinuationError("rejected", "continuation agent cursor is stale")
        }
        const threadRows: ReadonlyArray<Record<string, unknown>> = await writer.sql`
          SELECT latest_cursor FROM khala_sync_portable_thread_current
          WHERE session_ref = ${input.sessionRef} AND thread_ref = ${node.threadRef}
          FOR UPDATE
        `
        if (Number(threadRows[0]?.latest_cursor ?? 0) !== before.eventCursor) {
          throw new PortableManagedContinuationError("rejected", "continuation thread cursor is stale")
        }
        const running = {
          lifecycle: "running",
          activityCursor: before.activityCursor,
          turnRef: turn.turnRef,
          phase: "running",
          evidenceRef: evidenceSetRef,
        }
        await appendPortableSessionEvent(writer, {
          eventRef: continuationEventRef(operationScope, node.agentRef, "running"),
          sessionRef: input.sessionRef,
          threadRef: node.threadRef,
          threadCursor: before.eventCursor + 1,
          attachmentRef: input.attachmentRef,
          attachmentGeneration: input.generation,
          eventKind: "activity_cursor",
          current: running,
        }, continuationMutationRef(operationScope, node.agentRef, "running"))
        const markedRunning: ReadonlyArray<Record<string, unknown>> = await writer.sql`
          UPDATE khala_sync_portable_agent_nodes
          SET lifecycle = 'running'
          WHERE session_ref = ${input.sessionRef}
            AND agent_ref = ${node.agentRef}
            AND thread_ref = ${node.threadRef}
            AND attachment_generation = ${input.generation}
            AND activity_cursor = ${before.activityCursor}
            AND lifecycle IN ('waiting', 'running')
          RETURNING agent_ref
        `
        if (markedRunning.length !== 1) {
          throw new PortableManagedContinuationError("rejected", "continuation running transition conflicted")
        }
        const settled = {
          lifecycle: "waiting",
          activityCursor: after.activityCursor,
          turnRef: turn.turnRef,
          phase: "settled",
          evidenceRef: evidenceSetRef,
        }
        await appendPortableSessionEvent(writer, {
          eventRef: continuationEventRef(operationScope, node.agentRef, "settled"),
          sessionRef: input.sessionRef,
          threadRef: node.threadRef,
          threadCursor: before.eventCursor + 2,
          attachmentRef: input.attachmentRef,
          attachmentGeneration: input.generation,
          eventKind: "activity_cursor",
          current: settled,
        }, continuationMutationRef(operationScope, node.agentRef, "settled"))
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

      return {
        acceptedWorkRefs: orderedNodes.map(node => ({
          agentRef: node.agentRef,
          turnRef: turns.get(node.agentRef)!.turnRef,
        })),
        threadCursors: orderedNodes.map(node => ({
          agentRef: node.agentRef,
          threadRef: node.threadRef,
          activityCursor: cursors.get(node.agentRef)!.activityCursor,
          eventCursor: prior.get(node.agentRef)!.eventCursor + 2,
        })),
        evidenceRefs,
        replay: "executed" as const,
      }
    })
  }
}
