import {
  agentRunScope,
  adaptClaudeLiveAgentObservation,
  adaptCodexLiveAgentObservation,
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  canonicalJson,
  applyRuntimeInteractionDecision,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  decodeKhalaRuntimeControlIntent,
  decodeKhalaRuntimeEvent,
  decodeLiveAgentGraphEntity,
  decodeRuntimeControlIntentEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeInteraction,
  decodeRuntimeInteractionDecisionEnvelope,
  decodeRuntimeInteractionEntity,
  decodeRuntimeTurnEntity,
  EntityId,
  EntityType,
  liveAgentGraphScope,
  MutationResult,
  MutatorName,
  personalScope,
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_INTERACTION_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type KhalaRuntimeControlIntent,
  type KhalaRuntimeControlIntentKind,
  type KhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type RuntimeControlIntentEntity,
  type RuntimeControlIntentStatus,
  type RuntimeEventEntity,
  type RuntimeInteraction,
  type RuntimeInteractionDecisionEnvelope,
  type RuntimeInteractionEntity,
  type RuntimeTurnEntity,
  type RuntimeTurnStatus,
} from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"
import { ensureScopeOwner } from "./fleet-projection.js"
import { appendLiveAgentGraphChange } from "./live-agent-graph-projection.js"
import type { MutatorContext, MutatorDefinition } from "./push-engine.js"
import { defineMutator } from "./push-engine.js"

/**
 * Khala Code runtime mutators (#8370).
 *
 * These mutators give mobile/desktop/server surfaces an AI SDK-shaped,
 * server-authoritative control lane without exposing private runtime
 * material outside the owner/thread scopes:
 *
 * - `runtime.startTurn` records a body-free `turn.start` intent and creates
 *   a queued turn.
 * - `runtime.appendUserMessage` records a body-free `message.append`
 *   control intent, optionally tied to an existing turn.
 * - `runtime.interruptTurn`, `runtime.continueTurn`, `runtime.retryTurn`,
 *   and `runtime.closeTurn` advance existing owner-private turns.
 * - `runtime.recordEvent` records full canonical runtime events only in
 *   private thread authority, updates the turn, and transactionally mirrors a
 *   bounded canonical event into the existing thread/run agent timeline.
 *
 * Every mutator is in-band-rejecting and ledger-idempotent via `executePush`.
 * Runtime events can carry private text/tool deltas, so event projections stay
 * within the exact private thread and owner-authorized run scopes.
 */

export const RUNTIME_START_TURN_MUTATOR_NAME = "runtime.startTurn"
export const RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME =
  "runtime.appendUserMessage"
export const RUNTIME_INTERRUPT_TURN_MUTATOR_NAME = "runtime.interruptTurn"
export const RUNTIME_CONTINUE_TURN_MUTATOR_NAME = "runtime.continueTurn"
export const RUNTIME_RETRY_TURN_MUTATOR_NAME = "runtime.retryTurn"
export const RUNTIME_CLOSE_TURN_MUTATOR_NAME = "runtime.closeTurn"
export const RUNTIME_RECORD_EVENT_MUTATOR_NAME = "runtime.recordEvent"
export const RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME =
  "runtime.requestInteraction"
export const RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME =
  "runtime.decideInteraction"

export const RUNTIME_SCOPE_REJECTION = "unauthorized_scope"
export const RUNTIME_INTENT_KIND_REJECTION = "runtime_intent_kind_mismatch"
export const RUNTIME_TURN_REQUIRED_REJECTION = "runtime_turn_required"
export const RUNTIME_TURN_EXISTS_REJECTION = "runtime_turn_exists"
export const RUNTIME_TURN_NOT_FOUND_REJECTION = "runtime_turn_not_found"
export const RUNTIME_INTENT_CONFLICT_REJECTION = "runtime_intent_conflict"
export const RUNTIME_INTENT_EXPIRY_REJECTION = "runtime_intent_expiry_invalid"
export const RUNTIME_TARGET_LANE_REJECTION = "runtime_target_lane_mismatch"
/** @deprecated exact retries now reconcile; use the conflict code. */
export const RUNTIME_INTENT_EXISTS_REJECTION = RUNTIME_INTENT_CONFLICT_REJECTION
export const RUNTIME_MESSAGE_REQUIRED_REJECTION = "runtime_message_required"
export const RUNTIME_EVENT_EXISTS_REJECTION = "runtime_event_exists"
export const RUNTIME_EVENT_SEQUENCE_REJECTION = "runtime_event_sequence_invalid"
export const RUNTIME_EVENT_STATE_REJECTION = "runtime_event_state_invalid"
export const RUNTIME_RAW_BODY_REJECTION = "runtime_raw_body_not_allowed"
export const RUNTIME_INTERACTION_CONFLICT_REJECTION =
  "runtime_interaction_conflict"
export const RUNTIME_INTERACTION_STATE_REJECTION =
  "runtime_interaction_state_invalid"
export const RUNTIME_INTERACTION_DECISION_REJECTION =
  "runtime_interaction_decision_invalid"
export const RUNTIME_INTERACTION_SEQUENCE_REJECTION =
  "runtime_interaction_sequence_invalid"
export const RUNTIME_INTERACTION_EXPIRY_REJECTION =
  "runtime_interaction_expiry_invalid"

const RuntimeTurnEntityType = EntityType.make(RUNTIME_TURN_ENTITY_TYPE)
const RuntimeControlIntentEntityType = EntityType.make(
  RUNTIME_CONTROL_INTENT_ENTITY_TYPE,
)
const RuntimeEventEntityType = EntityType.make(RUNTIME_EVENT_ENTITY_TYPE)
const RuntimeInteractionEntityType = EntityType.make(
  RUNTIME_INTERACTION_ENTITY_TYPE,
)
const AgentRunEntityType = EntityType.make(AGENT_RUN_ENTITY_TYPE)
const AgentRunEventEntityType = EntityType.make(AGENT_RUN_EVENT_ENTITY_TYPE)

type RuntimeThreadContextRow = Readonly<{
  title: string
  repo_binding_owner: string | null
  repo_binding_name: string | null
  repo_binding_default_branch: string | null
}>

type RuntimeWorkContextSnapshotRow = Readonly<{
  work_context_ref: string | null
  goal_message_id: string | null
  repository_provider: string | null
  repository_owner: string | null
  repository_name: string | null
  repository_ref: string | null
}>

const readRuntimeThreadContext = async (
  ctx: MutatorContext,
  threadId: string,
): Promise<RuntimeThreadContextRow | null> => {
  const rows: Array<RuntimeThreadContextRow> = await ctx.writer.sql`
    SELECT title, repo_binding_owner, repo_binding_name,
           repo_binding_default_branch
    FROM khala_sync_chat_threads
    WHERE thread_id = ${threadId}
  `
  return rows[0] ?? null
}

const storedJsonObject = (value: unknown): Record<string, unknown> | null => {
  try {
    const decoded = typeof value === "string" ? JSON.parse(value) : value
    if (typeof decoded !== "object" || decoded === null || Array.isArray(decoded)) {
      return null
    }
    return decoded as Record<string, unknown>
  } catch {
    return null
  }
}

const messageIdFromBodyRef = (bodyRef: string | undefined): string | null => {
  const prefix = "chat_message."
  return bodyRef?.startsWith(prefix) === true ? bodyRef.slice(prefix.length) : null
}

const readRuntimeWorkContextSnapshot = async (
  ctx: MutatorContext,
  turnId: string,
): Promise<RuntimeWorkContextSnapshotRow | null> => {
  const rows: Array<RuntimeWorkContextSnapshotRow> = await ctx.writer.sql`
    SELECT work_context_ref, goal_message_id, repository_provider,
           repository_owner, repository_name, repository_ref
    FROM khala_sync_runtime_turns
    WHERE turn_id = ${turnId}
  `
  return rows[0] ?? null
}

const readRuntimeGoal = async (
  ctx: MutatorContext,
  turnId: string,
  threadId: string,
  fallback: string,
  goalMessageId?: string | null,
): Promise<string> => {
  let messageId = goalMessageId ?? null
  if (messageId === null) {
    const intentRows: Array<{ intent_json: unknown }> = await ctx.writer.sql`
      SELECT intent_json
      FROM khala_sync_runtime_control_intents
      WHERE turn_id = ${turnId} AND kind = 'turn.start'
      ORDER BY seq ASC
      LIMIT 1
    `
    const bodyRef = storedJsonObject(intentRows[0]?.intent_json)?.bodyRef
    messageId = messageIdFromBodyRef(
      typeof bodyRef === "string" ? bodyRef : undefined,
    )
  }
  if (messageId !== null) {
    const messageRows: Array<{ body: string }> = await ctx.writer.sql`
      SELECT body
      FROM khala_sync_chat_messages
      WHERE message_id = ${messageId} AND thread_id = ${threadId}
      LIMIT 1
    `
    const body = messageRows[0]?.body.trim()
    if (body !== undefined && body !== "") return body.slice(0, 10_000)
  }
  const normalized = fallback.trim()
  return (normalized === "" ? "Conversation turn" : normalized).slice(0, 10_000)
}

const runtimeProjectionKind = (lane: RuntimeTurnEntity["lane"]) => {
  switch (lane) {
    case "claude_pylon":
      return { backend: "pylon" as const, runtime: "claude_code" as const }
    case "hosted_khala":
      return { backend: "hosted" as const, runtime: "openagents_native" as const }
    default:
      return { backend: "pylon" as const, runtime: "codex" as const }
  }
}

const agentRunStatus = (
  status: RuntimeTurnStatus,
): "queued" | "running" | "waiting_for_input" | "completed" | "failed" | "canceled" =>
  status === "interrupted" || status === "closed" ? "canceled" : status

const appendRuntimeAgentRunChanges = async (
  ctx: MutatorContext,
  turn: RuntimeTurnEntity,
): Promise<void> => {
  const thread = await readRuntimeThreadContext(ctx, turn.threadId)
  // Historical runtime-only callers may use promptRef without first
  // admitting a canonical chat thread. They keep their runtime_* projection;
  // the agent-run route binding is created only for the conversation path.
  if (thread === null) return
  const snapshot = await readRuntimeWorkContextSnapshot(ctx, turn.turnId)
  const goal = await readRuntimeGoal(
    ctx,
    turn.turnId,
    turn.threadId,
    thread.title,
    snapshot?.goal_message_id,
  )
  const kind = runtimeProjectionKind(turn.lane)
  const repository =
    snapshot?.repository_provider !== "github" ||
    snapshot.repository_owner === null ||
    snapshot.repository_name === null ||
    snapshot.repository_ref === null
      ? undefined
      : {
          owner: snapshot.repository_owner,
          provider: "github" as const,
          ref: snapshot.repository_ref,
          repo: snapshot.repository_name,
        }
  const entity = decodeAgentRunEntity({
    backend: kind.backend,
    canceledAt:
      turn.status === "interrupted" || turn.status === "closed"
        ? turn.settledAt
        : null,
    completedAt: turn.status === "completed" ? turn.settledAt : null,
    createdAt: turn.createdAt,
    failedAt: turn.status === "failed" ? turn.settledAt : null,
    goal,
    goalId: null,
    projectId: null,
    ...(repository === undefined ? {} : { repository }),
    routeId: turn.threadId,
    ...(snapshot?.work_context_ref === null || snapshot?.work_context_ref === undefined
      ? {}
      : { workContextRef: snapshot.work_context_ref }),
    runId: turn.turnId,
    runtime: kind.runtime,
    startedAt: turn.startedAt,
    status: agentRunStatus(turn.status),
    teamId: null,
    updatedAt: turn.updatedAt,
    userId: turn.ownerUserId,
  })
  for (const scope of [
    personalScope(turn.ownerUserId),
    threadScope(turn.threadId),
    agentRunScope(turn.turnId),
  ]) {
    await ctx.writer.appendChange({
      entityId: EntityId.make(turn.turnId),
      entityType: AgentRunEntityType,
      mutationRef: ctx.mutationRef,
      op: "upsert",
      postImage: { ...entity },
      scope,
    })
  }
}

const runtimeGraphTerminal = (status: RuntimeTurnStatus): boolean =>
  status === "completed" || status === "failed" || status === "interrupted" || status === "closed"

type RuntimeChildEvent = Extract<KhalaRuntimeEvent, {
  kind: "agent.child.started" | "agent.child.progress" | "agent.child.finished"
}>

const isRuntimeChildEvent = (event: KhalaRuntimeEvent | undefined): event is RuntimeChildEvent =>
  event?.kind === "agent.child.started" ||
  event?.kind === "agent.child.progress" ||
  event?.kind === "agent.child.finished"

const readRuntimeLiveAgentGraph = async (
  ctx: MutatorContext,
  turn: RuntimeTurnEntity,
) => {
  const rows: Array<{ post_image_json: unknown }> = await ctx.writer.sql`
    SELECT post_image_json
    FROM khala_sync_changelog
    WHERE scope = ${liveAgentGraphScope(turn.threadId)}
      AND entity_type = 'live_agent_graph'
      AND entity_id = ${`graph.runtime.${turn.turnId}`}
      AND op = 'upsert'
    ORDER BY version DESC
    LIMIT 1
  `
  const value = rows[0]?.post_image_json
  return value === undefined
    ? null
    : decodeLiveAgentGraphEntity(typeof value === "string" ? JSON.parse(value) : value)
}

const appendRuntimeLiveAgentGraph = async (
  ctx: MutatorContext,
  turn: RuntimeTurnEntity,
  event?: KhalaRuntimeEvent,
): Promise<void> => {
  if (turn.lane !== "codex_app_server" && turn.lane !== "claude_pylon") return
  const previous = await readRuntimeLiveAgentGraph(ctx, turn)
  const previousNode = previous?.nodes.find(candidate => candidate.parent.kind === "root")
  const terminal = runtimeGraphTerminal(turn.status)
  const reopening = previousNode?.terminal.state === "terminal" && !terminal
  const attachmentGeneration = reopening
    ? (previous?.attachmentGeneration ?? 1) + 1
    : previous?.attachmentGeneration ?? 1
  const observationAgentId = attachmentGeneration === 1
    ? turn.turnId
    : `${turn.turnId}.g${attachmentGeneration}`
  const graphCursor = previous === null ? 0 : previous.cursor + 1
  const nodeVersion = reopening
    ? 1
    : Math.max(turn.eventCount + 1, (previousNode?.version ?? 0) + 1)
  const providerRef = event?.source.providerRef ?? (
    previousNode?.provider.state === "known" ? previousNode.provider.providerRef : null
  )
  const provider = providerRef === null
    ? { state: "omitted" as const, reason: "provider_omitted" as const }
    : { state: "known" as const, providerRef }
  const worktree = previousNode?.worktree.state === "known"
    ? previousNode.worktree
    : { state: "omitted" as const, reason: "provider_omitted" as const }
  const attention = turn.status === "waiting_for_input"
    ? { state: "omitted" as const, reason: "not_observed" as const }
    : { state: "none" as const }
  const common = {
    graphRef: `graph.runtime.${turn.turnId}`,
    sessionRef: `session.runtime.${turn.threadId}`,
    threadRef: turn.threadId,
    provider,
    runtimeRef: `runtime.${turn.lane}.${observationAgentId}`,
    attachmentGeneration,
    agent: {
      threadId: observationAgentId,
      runId: turn.turnId,
      parent: { state: "root" as const },
      worktree,
      attention,
      activityCursor: turn.eventCount,
      createdAt: turn.createdAt,
      updatedAt: turn.updatedAt,
      startedAt: turn.startedAt,
      endedAt: terminal ? turn.settledAt : null,
      version: nodeVersion,
    },
  }
  const toolEvent = event?.kind === "tool.input.delta" ||
    event?.kind === "tool.input.completed" ||
    event?.kind === "tool.call" ||
    event?.kind === "tool.result" ||
    event?.kind === "tool.error"
  const adapted = turn.lane === "codex_app_server"
    ? adaptCodexLiveAgentObservation({
        schema: "openagents.codex_live_agent_observation.v1",
        ...common,
        agent: {
          ...common.agent,
          status: turn.status === "queued"
            ? "notStarted"
            : turn.status === "running"
              ? "inProgress"
              : turn.status === "waiting_for_input"
                ? "waitingForInput"
                : turn.status === "completed"
                  ? "completed"
                  : turn.status === "failed"
                    ? "failed"
                    : "interrupted",
          currentTool: toolEvent && event !== undefined
            ? {
                state: "known",
                toolCallId: event.toolCallId,
                toolName: event.toolName,
                status: event.kind === "tool.result"
                  ? "completed"
                  : event.kind === "tool.error"
                    ? "failed"
                    : event.kind === "tool.call" || event.kind === "tool.input.completed"
                      ? "inProgress"
                      : "pending",
                version: nodeVersion,
              }
            : { state: "omitted", reason: "not_observed" },
        },
      })
    : adaptClaudeLiveAgentObservation({
        schema: "openagents.claude_live_agent_observation.v1",
        ...common,
        agent: {
          ...common.agent,
          status: turn.status === "queued"
            ? "queued"
            : turn.status === "running"
              ? "running"
              : turn.status === "waiting_for_input"
                ? "waiting_for_permission"
                : turn.status === "completed"
                  ? "succeeded"
                  : turn.status === "failed"
                    ? "errored"
                    : "interrupted",
          currentTool: toolEvent && event !== undefined
            ? {
                state: "known",
                toolUseId: event.toolCallId,
                toolName: event.toolName,
                status: event.kind === "tool.result"
                  ? "succeeded"
                  : event.kind === "tool.error"
                    ? "errored"
                    : event.kind === "tool.call" || event.kind === "tool.input.completed"
                      ? "running"
                      : "queued",
                version: nodeVersion,
              }
            : { state: "omitted", reason: "not_observed" },
        },
      })

  let node = adapted.node
  let rootEdges = adapted.edges
  const previousChildNodes = reopening || previousNode === undefined
    ? []
    : (previous?.nodes ?? []).filter(candidate => candidate.agentRef !== previousNode.agentRef)
  const previousChildRefs = new Set(previousChildNodes.map(candidate => candidate.agentRef))
  let childNodes = previousChildNodes
  let childEdges = reopening
    ? []
    : (previous?.edges ?? []).filter(edge => edge.kind === "parent"
      ? previousChildRefs.has(edge.toAgentRef)
      : previousChildRefs.has(edge.agentRef))
  if (!reopening && !toolEvent && previousNode?.currentTool.state === "known") {
    if (!terminal && (previousNode.currentTool.status === "called" || previousNode.currentTool.status === "running")) {
      node = { ...node, currentTool: previousNode.currentTool }
      rootEdges = (previous?.edges ?? []).filter(edge =>
        edge.kind === "tool" && edge.agentRef === previousNode.agentRef)
    } else {
      node = { ...node, currentTool: { state: "none" } }
      rootEdges = (previous?.edges ?? []).filter(edge =>
        edge.kind === "tool" && edge.agentRef === previousNode.agentRef).map(edge =>
        ({ ...edge, status: "unknown" as const, version: Math.max(edge.version + 1, node.version) }))
    }
  }

  if (!reopening && isRuntimeChildEvent(event)) {
    const childAgentRef = `agent.${turn.lane === "codex_app_server" ? "codex" : "claude"}.${event.childAgentId}`
    const previousChild = previousChildNodes.find(candidate => candidate.agentRef === childAgentRef)
    const childFinishReason = event.kind === "agent.child.finished" ? event.finishReason : null
    const childTerminal = childFinishReason !== null
    const childInterrupted = childFinishReason === "cancelled" || childFinishReason === "interrupted"
    const childFailed = childFinishReason === "error"
    const parentObservationId = event.parentAgentId === turn.turnId
      ? observationAgentId
      : event.parentAgentId
    const childVersion = (previousChild?.version ?? 0) + 1
    const childCommon = {
      graphRef: common.graphRef,
      sessionRef: common.sessionRef,
      threadRef: common.threadRef,
      provider,
      runtimeRef: `runtime.${turn.lane}.child.${event.childAgentId}`,
      attachmentGeneration,
      agent: {
        threadId: event.childAgentId,
        runId: event.childRunId,
        parent: { state: "known" as const, threadId: parentObservationId },
        worktree: previousChild?.worktree.state === "known"
          ? previousChild.worktree
          : { state: "omitted" as const, reason: "provider_omitted" as const },
        attention: { state: "none" as const },
        activityCursor: (previousChild?.activityCursor ?? 0) + 1,
        createdAt: previousChild?.createdAt ?? turn.updatedAt,
        updatedAt: turn.updatedAt,
        startedAt: previousChild?.startedAt ?? (childTerminal ? null : turn.updatedAt),
        endedAt: childTerminal ? turn.updatedAt : null,
        version: childVersion,
      },
    }
    const childAdapted = turn.lane === "codex_app_server"
      ? adaptCodexLiveAgentObservation({
          schema: "openagents.codex_live_agent_observation.v1",
          ...childCommon,
          agent: {
            ...childCommon.agent,
            status: childInterrupted
              ? "interrupted"
              : childFailed
                ? "failed"
                : childTerminal
                  ? "completed"
                  : "inProgress",
            currentTool: { state: "omitted", reason: "not_observed" },
          },
        })
      : adaptClaudeLiveAgentObservation({
          schema: "openagents.claude_live_agent_observation.v1",
          ...childCommon,
          agent: {
            ...childCommon.agent,
            status: childInterrupted
              ? "interrupted"
              : childFailed
                ? "errored"
                : childTerminal
                  ? "succeeded"
                  : "running",
            currentTool: { state: "omitted", reason: "not_observed" },
          },
        })
    childNodes = [
      ...childNodes.filter(candidate => candidate.agentRef !== childAdapted.node.agentRef),
      childAdapted.node,
    ]
    childEdges = [
      ...childEdges.filter(edge => edge.kind === "parent"
        ? edge.fromAgentRef !== childAdapted.node.agentRef &&
          edge.toAgentRef !== childAdapted.node.agentRef
        : edge.agentRef !== childAdapted.node.agentRef),
      ...childAdapted.edges,
    ]
  }
  await appendLiveAgentGraphChange(ctx.writer, {
    schema: "openagents.live_agent_graph.v1",
    graphRef: adapted.graphRef,
    sessionRef: node.sessionRef,
    threadRef: adapted.threadRef,
    attachmentGeneration: node.attachmentGeneration,
    cursor: graphCursor,
    lastDeltaRef: graphCursor === 0
      ? null
      : event?.eventId ?? `delta.graph.${turn.turnId}.${graphCursor}`,
    nodes: [node, ...childNodes],
    edges: [...rootEdges, ...childEdges],
    updatedAt: turn.updatedAt,
  }, ctx.mutationRef)
}

const runtimeEventSummary = (event: KhalaRuntimeEvent): string => {
  const summary = (() => {
    switch (event.kind) {
      case "text.delta":
      case "reasoning.delta":
        return event.text.trim() === "" ? `${event.kind} received` : event.text
      case "tool.call":
        return `Called ${event.toolName}`
      case "tool.result":
        return `${event.toolName} completed`
      case "tool.error":
        return event.messageSafe.trim() === ""
          ? `${event.toolName} failed`
          : event.messageSafe
      case "turn.finished":
        return `Turn finished: ${event.finishReason}`
      case "turn.interrupted":
        return "Turn interrupted"
      default:
        return event.kind.replaceAll(".", " ")
    }
  })()
  return summary.slice(0, 20_000)
}

const runtimeEventStatus = (event: KhalaRuntimeEvent): string | null => {
  switch (event.kind) {
    case "turn.started":
      return "running"
    case "turn.interrupted":
      return "canceled"
    case "turn.finished":
      return event.finishReason === "error"
        ? "failed"
        : event.finishReason === "cancelled" || event.finishReason === "interrupted"
          ? "canceled"
          : "completed"
    case "tool.call":
    case "agent.child.started":
    case "agent.child.progress":
      return "running"
    case "tool.result":
      return "completed"
    case "tool.error":
      return "failed"
    case "agent.child.finished":
      return event.finishReason === "error"
        ? "failed"
        : event.finishReason === "cancelled" || event.finishReason === "interrupted"
          ? "canceled"
          : "completed"
    default:
      return null
  }
}

const runtimeEventArtifactRefs = (
  event: KhalaRuntimeEvent,
): ReadonlyArray<string> => {
  switch (event.kind) {
    case "file.change":
      return [event.fileChange.fileChangeRef]
    case "writeback.recorded":
      return [event.writebackRef]
    case "raw.sidecar_ref":
      return [event.rawEventRef]
    default:
      return []
  }
}

const appendRuntimeAgentEventChanges = async (
  ctx: MutatorContext,
  event: KhalaRuntimeEvent,
  createdAt: string,
): Promise<void> => {
  if ((await readRuntimeThreadContext(ctx, event.threadId)) === null) return
  const payload = JSON.stringify(event)
  const entity = decodeAgentRunEventEntity({
    artifactRefs: runtimeEventArtifactRefs(event),
    createdAt,
    externalEventId: event.eventId,
    id: event.eventId,
    // Keep the canonical runtime event as the only payload format. An
    // abnormally large content chunk stays available in runtime_event but is
    // omitted from this bounded client timeline instead of aborting the
    // authoritative runtime transaction.
    payloadJson: payload.length <= 262_144 ? payload : null,
    runId: event.turnId,
    sequence: event.sequence,
    source: `runtime.${event.source.adapterKind ?? event.source.lane}`,
    status: runtimeEventStatus(event),
    summary: runtimeEventSummary(event),
    type: event.kind,
  })
  for (const scope of [
    threadScope(event.threadId),
    agentRunScope(event.turnId),
  ]) {
    await ctx.writer.appendChange({
      entityId: EntityId.make(event.eventId),
      entityType: AgentRunEventEntityType,
      mutationRef: ctx.mutationRef,
      op: "upsert",
      postImage: { ...entity },
      scope,
    })
  }
}

export const decodeRuntimeControlIntentArgs = (
  argsJson: string,
): KhalaRuntimeControlIntent =>
  decodeKhalaRuntimeControlIntent(JSON.parse(argsJson) as unknown)

export const decodeRuntimeEventArgs = (argsJson: string): KhalaRuntimeEvent =>
  decodeKhalaRuntimeEvent(JSON.parse(argsJson) as unknown)

export const decodeRuntimeInteractionArgs = (
  argsJson: string,
): RuntimeInteraction => decodeRuntimeInteraction(JSON.parse(argsJson) as unknown)

const RuntimeInteractionDecisionArgs = S.Struct({
  interactionRef: S.String,
  threadId: S.String,
  turnId: S.String,
  envelope: S.Unknown,
})

type RuntimeInteractionDecisionArgs = Readonly<{
  interactionRef: string
  threadId: string
  turnId: string
  envelope: RuntimeInteractionDecisionEnvelope
}>

export const decodeRuntimeInteractionDecisionArgs = (
  argsJson: string,
): RuntimeInteractionDecisionArgs => {
  const decoded = S.decodeUnknownSync(RuntimeInteractionDecisionArgs)(
    JSON.parse(argsJson) as unknown,
  )
  return {
    interactionRef: decoded.interactionRef,
    threadId: decoded.threadId,
    turnId: decoded.turnId,
    envelope: decodeRuntimeInteractionDecisionEnvelope(decoded.envelope),
  }
}

type RuntimeTurnRow = Readonly<{
  turn_id: string
  thread_id: string
  owner_user_id: string
  lane: string
  status: string
  event_count: string | number
  latest_intent_id: string | null
  started_at: string | null
  settled_at: string | null
  created_at: string
  updated_at: string
}>

type RuntimeControlIntentConflictRow = Readonly<{
  intent_id: string
  owner_user_id: string
  idempotency_key: string
  intent_json: unknown
}>

type RuntimeEventConflictRow = Readonly<{
  event_id: string
}>

type RuntimeInteractionRow = Readonly<{
  interaction_ref: string
  thread_id: string
  turn_id: string
  owner_user_id: string
  kind: RuntimeInteractionEntity["kind"]
  status: RuntimeInteractionEntity["status"]
  requested_sequence: string | number
  expires_at: string
  interaction_json: unknown
  created_at: string
  updated_at: string
}>

const transactionNowIso = async (ctx: MutatorContext): Promise<string> => {
  const rows: Array<{ now: Date | string }> = await ctx.writer.sql`
    SELECT now() AS now
  `
  const raw = rows[0]?.now
  if (raw === undefined) throw new Error("SELECT now() returned no row")
  return raw instanceof Date ? raw.toISOString() : new Date(raw).toISOString()
}

const reject = (
  ctx: MutatorContext,
  errorCode: string,
  errorMessageSafe: string,
): MutationResult =>
  new MutationResult({
    errorCode,
    errorMessageSafe,
    mutationId: ctx.mutationId,
    status: "rejected",
  })

const rejectForeignScope = (ctx: MutatorContext): MutationResult =>
  reject(
    ctx,
    RUNTIME_SCOPE_REJECTION,
    "this runtime thread scope belongs to a different user",
  )

const applied = (ctx: MutatorContext): MutationResult =>
  new MutationResult({ mutationId: ctx.mutationId, status: "applied" })

const ensureRuntimeThreadOwner = async (
  ctx: MutatorContext,
  threadId: string,
): Promise<MutationResult | null> => {
  const owner = await ensureScopeOwner(ctx.writer.sql, threadScope(threadId), ctx.userId)
  return owner === ctx.userId ? null : rejectForeignScope(ctx)
}

const readTurnForUpdate = async (
  ctx: MutatorContext,
  turnId: string,
): Promise<RuntimeTurnRow | null> => {
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    SELECT turn_id, thread_id, owner_user_id, lane, status, event_count,
           latest_intent_id, started_at, settled_at, created_at, updated_at
    FROM khala_sync_runtime_turns
    WHERE turn_id = ${turnId}
    FOR UPDATE
  `
  return rows[0] ?? null
}

const readInteractionForUpdate = async (
  ctx: MutatorContext,
  interactionRef: string,
): Promise<RuntimeInteractionRow | null> => {
  const rows: Array<RuntimeInteractionRow> = await ctx.writer.sql`
    SELECT interaction_ref, thread_id, turn_id, owner_user_id, kind, status,
           requested_sequence, expires_at, interaction_json, created_at,
           updated_at
    FROM khala_sync_runtime_interactions
    WHERE interaction_ref = ${interactionRef}
    FOR UPDATE
  `
  return rows[0] ?? null
}

const interactionEntityFromRecord = (
  interaction: RuntimeInteraction,
  input: Readonly<{
    ownerUserId: string
    createdAt: string
    updatedAt: string
  }>,
): RuntimeInteractionEntity => decodeRuntimeInteractionEntity({
  interactionRef: interaction.interactionRef,
  threadId: interaction.threadId,
  turnId: interaction.turnId,
  ownerUserId: input.ownerUserId,
  kind: interaction.payload.kind,
  status: interaction.lifecycle.status,
  interaction,
  createdAt: input.createdAt,
  updatedAt: input.updatedAt,
})

const interactionEntityFromRow = (
  row: RuntimeInteractionRow,
): RuntimeInteractionEntity => interactionEntityFromRecord(
  decodeRuntimeInteraction(storedJsonObject(row.interaction_json)),
  {
    ownerUserId: row.owner_user_id,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  },
)

const appendRuntimeInteractionEntityChange = async (
  ctx: MutatorContext,
  entity: RuntimeInteractionEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.interactionRef),
    entityType: RuntimeInteractionEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const insertRuntimeInteraction = async (
  ctx: MutatorContext,
  interaction: RuntimeInteraction,
  nowIso: string,
): Promise<RuntimeInteractionEntity> => {
  const entity = interactionEntityFromRecord(interaction, {
    ownerUserId: ctx.userId,
    createdAt: nowIso,
    updatedAt: nowIso,
  })
  await ctx.writer.sql`
    INSERT INTO khala_sync_runtime_interactions
      (interaction_ref, thread_id, turn_id, owner_user_id, kind, status,
       requested_sequence, expires_at, interaction_json, created_at, updated_at)
    VALUES
      (${entity.interactionRef}, ${entity.threadId}, ${entity.turnId},
       ${entity.ownerUserId}, ${entity.kind}, ${entity.status},
       ${interaction.requestedSequence}, ${interaction.expiresAt},
       ${interaction}::jsonb, ${entity.createdAt}, ${entity.updatedAt})
  `
  await appendRuntimeInteractionEntityChange(ctx, entity)
  return entity
}

const updateRuntimeInteraction = async (
  ctx: MutatorContext,
  row: RuntimeInteractionRow,
  interaction: RuntimeInteraction,
  nowIso: string,
): Promise<RuntimeInteractionEntity> => {
  const entity = interactionEntityFromRecord(interaction, {
    ownerUserId: row.owner_user_id,
    createdAt: row.created_at,
    updatedAt: nowIso,
  })
  await ctx.writer.sql`
    UPDATE khala_sync_runtime_interactions
    SET status = ${entity.status},
        interaction_json = ${interaction}::jsonb,
        updated_at = ${nowIso}
    WHERE interaction_ref = ${entity.interactionRef}
  `
  await appendRuntimeInteractionEntityChange(ctx, entity)
  return entity
}

const updateTurnWaitingForInteraction = async (
  ctx: MutatorContext,
  turnId: string,
  nowIso: string,
): Promise<void> => {
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    UPDATE khala_sync_runtime_turns
    SET status = 'waiting_for_input', updated_at = ${nowIso}
    WHERE turn_id = ${turnId}
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) throw new Error("runtime interaction turn disappeared")
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  await appendRuntimeAgentRunChanges(ctx, entity)
  await appendRuntimeLiveAgentGraph(ctx, entity)
}

const readControlIntentConflict = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): Promise<RuntimeControlIntentConflictRow | null> => {
  const rows: Array<RuntimeControlIntentConflictRow> = await ctx.writer.sql`
    SELECT intent_id, owner_user_id, idempotency_key, intent_json
    FROM khala_sync_runtime_control_intents
    WHERE intent_id = ${intent.intentId}
       OR (owner_user_id = ${ctx.userId}
           AND idempotency_key = ${intent.idempotencyKey})
    LIMIT 1
  `
  return rows[0] ?? null
}

const readRuntimeEventConflict = async (
  ctx: MutatorContext,
  event: KhalaRuntimeEvent,
): Promise<RuntimeEventConflictRow | null> => {
  const rows: Array<RuntimeEventConflictRow> = await ctx.writer.sql`
    SELECT event_id
    FROM khala_sync_runtime_events
    WHERE event_id = ${event.eventId}
       OR (turn_id = ${event.turnId} AND sequence = ${event.sequence})
    LIMIT 1
  `
  return rows[0] ?? null
}

const turnEntityFromRow = (row: RuntimeTurnRow): RuntimeTurnEntity =>
  decodeRuntimeTurnEntity({
    createdAt: row.created_at,
    eventCount: Number(row.event_count),
    latestIntentId: row.latest_intent_id,
    lane: row.lane,
    ownerUserId: row.owner_user_id,
    settledAt: row.settled_at,
    startedAt: row.started_at,
    status: row.status,
    threadId: row.thread_id,
    turnId: row.turn_id,
    updatedAt: row.updated_at,
  })

const controlIntentEntityFromIntent = (
  intent: KhalaRuntimeControlIntent,
  input: {
    readonly ownerUserId: string
    readonly status: RuntimeControlIntentStatus
    readonly nowIso: string
  },
): RuntimeControlIntentEntity =>
  decodeRuntimeControlIntentEntity({
    createdAt: input.nowIso,
    intent,
    intentId: intent.intentId,
    kind: intent.kind,
    ownerUserId: input.ownerUserId,
    status: input.status,
    threadId: intent.threadId,
    turnId: intent.turnId ?? null,
    updatedAt: input.nowIso,
  })

const runtimeEventEntityFromEvent = (
  event: KhalaRuntimeEvent,
  input: {
    readonly ownerUserId: string
    readonly nowIso: string
  },
): RuntimeEventEntity =>
  decodeRuntimeEventEntity({
    createdAt: input.nowIso,
    event,
    eventId: event.eventId,
    kind: event.kind,
    observedAt: event.observedAt,
    ownerUserId: input.ownerUserId,
    sequence: event.sequence,
    threadId: event.threadId,
    turnId: event.turnId,
  })

const appendTurnEntityChanges = async (
  ctx: MutatorContext,
  entity: RuntimeTurnEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.turnId),
    entityType: RuntimeTurnEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: personalScope(entity.ownerUserId),
  })
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.turnId),
    entityType: RuntimeTurnEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const appendControlIntentEntityChanges = async (
  ctx: MutatorContext,
  entity: RuntimeControlIntentEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.intentId),
    entityType: RuntimeControlIntentEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: personalScope(entity.ownerUserId),
  })
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.intentId),
    entityType: RuntimeControlIntentEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const appendRuntimeEventEntityChange = async (
  ctx: MutatorContext,
  entity: RuntimeEventEntity,
): Promise<void> => {
  await ctx.writer.appendChange({
    entityId: EntityId.make(entity.eventId),
    entityType: RuntimeEventEntityType,
    mutationRef: ctx.mutationRef,
    op: "upsert",
    postImage: { ...entity },
    scope: threadScope(entity.threadId),
  })
}

const validateControlIntentBasics = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  expectedKind: KhalaRuntimeControlIntentKind,
): Promise<MutationResult | null> => {
  if (intent.kind !== expectedKind) {
    return reject(
      ctx,
      RUNTIME_INTENT_KIND_REJECTION,
      "runtime control intent kind does not match the mutator",
    )
  }
  if (intent.body !== undefined) {
    return reject(
      ctx,
      RUNTIME_RAW_BODY_REJECTION,
      "runtime control intents must carry bodyRef or promptRef, not raw body",
    )
  }
  const conflict = await readControlIntentConflict(ctx, intent)
  if (conflict !== null) {
    const recorded = storedJsonObject(conflict.intent_json)
    if (
      conflict.owner_user_id === ctx.userId &&
      recorded !== null &&
      canonicalJson(recorded) === canonicalJson(intent)
    ) {
      // A retry may arrive under a fresh Sync mutation id after the first
      // acknowledgement was lost. The durable semantic identity already
      // exists and is byte-equivalent, so reconcile without re-inserting or
      // dispatching a second turn.
      return applied(ctx)
    }
    return reject(
      ctx,
      RUNTIME_INTENT_CONFLICT_REJECTION,
      "runtime control intent identity was reused with different semantics",
    )
  }
  return null
}

const requireTurnId = (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): string | MutationResult => {
  if (intent.turnId === undefined) {
    return reject(
      ctx,
      RUNTIME_TURN_REQUIRED_REJECTION,
      "this runtime control intent requires a turn id",
    )
  }
  return intent.turnId
}

const requireMessageId = (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
): string | MutationResult => {
  if (intent.messageId === undefined) {
    return reject(
      ctx,
      RUNTIME_MESSAGE_REQUIRED_REJECTION,
      "append user message requires a message id",
    )
  }
  return intent.messageId
}

const isMutationResult = (value: string | MutationResult): value is MutationResult =>
  value instanceof MutationResult

const insertControlIntent = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  status: RuntimeControlIntentStatus,
  nowIso: string,
): Promise<RuntimeControlIntentEntity> => {
  const entity = controlIntentEntityFromIntent(intent, {
    nowIso,
    ownerUserId: ctx.userId,
    status,
  })
  // `intent_json` is jsonb: bind the OBJECT, never a pre-stringified string.
  // Both drivers (Bun's `SQL` in tests, postgres.js over Hyperdrive in the
  // Worker) serialize a JS object to jsonb exactly once. Passing an
  // already-serialized string (e.g. `canonicalJson(...)`) makes the driver
  // JSON-encode it AGAIN, storing a jsonb string SCALAR
  // (`"{\"bodyRef\":...}"`, `jsonb_typeof = 'string'`) instead of an object,
  // so `intent_json->>'bodyRef'` is NULL. That double-encoding broke hosted
  // chat turn resolution; the readers stay defensive to both encodings.
  await ctx.writer.sql`
    INSERT INTO khala_sync_runtime_control_intents
      (intent_id, thread_id, turn_id, owner_user_id, kind, status,
       idempotency_key, intent_json, created_at, updated_at)
    VALUES
      (${entity.intentId}, ${entity.threadId}, ${entity.turnId},
       ${entity.ownerUserId}, ${entity.kind}, ${entity.status},
       ${intent.idempotencyKey}, ${entity.intent}::jsonb,
       ${entity.createdAt}, ${entity.updatedAt})
  `
  await appendControlIntentEntityChanges(ctx, entity)
  return entity
}

const expireControlIntentIfDue = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  nowIso: string,
): Promise<MutationResult | null> => {
  if (intent.expiresAt === undefined) return null
  const expiresAt = Date.parse(intent.expiresAt)
  if (
    !Number.isFinite(expiresAt) ||
    new Date(expiresAt).toISOString() !== intent.expiresAt
  ) {
    return reject(
      ctx,
      RUNTIME_INTENT_EXPIRY_REJECTION,
      "runtime control intent expiry must be an ISO timestamp",
    )
  }
  if (expiresAt > Date.parse(nowIso)) return null
  await insertControlIntent(ctx, intent, "expired", nowIso)
  return applied(ctx)
}

const insertTurn = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  nowIso: string,
): Promise<RuntimeTurnEntity> => {
  const turnId = intent.turnId
  if (turnId === undefined) {
    throw new Error("insertTurn requires a turn id after validation")
  }
  const thread = await readRuntimeThreadContext(ctx, intent.threadId)
  const goalMessageId = messageIdFromBodyRef(intent.bodyRef)
  const hasRepository =
    thread?.repo_binding_owner !== null &&
    thread?.repo_binding_owner !== undefined &&
    thread.repo_binding_name !== null &&
    thread.repo_binding_default_branch !== null
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    INSERT INTO khala_sync_runtime_turns
      (turn_id, thread_id, owner_user_id, lane, status, event_count,
       latest_intent_id, started_at, settled_at, created_at, updated_at,
       work_context_ref, goal_message_id, repository_provider,
       repository_owner, repository_name, repository_ref)
    VALUES
      (${turnId}, ${intent.threadId}, ${ctx.userId}, ${intent.target.lane},
       'queued', 0, ${intent.intentId}, ${null}, ${null}, ${nowIso}, ${nowIso},
       ${thread === null ? null : `work_context.thread.${intent.threadId}`},
       ${goalMessageId}, ${hasRepository ? "github" : null},
       ${hasRepository ? thread.repo_binding_owner : null},
       ${hasRepository ? thread.repo_binding_name : null},
       ${hasRepository ? thread.repo_binding_default_branch : null})
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn insert returned no row")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

const updateTurnForIntent = async (
  ctx: MutatorContext,
  input: {
    readonly turnId: string
    readonly status: RuntimeTurnStatus
    readonly latestIntentId: string
    readonly settledAt: string | null
    readonly nowIso: string
  },
): Promise<RuntimeTurnEntity> => {
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    UPDATE khala_sync_runtime_turns
    SET status = ${input.status},
        latest_intent_id = ${input.latestIntentId},
        settled_at = ${input.settledAt},
        updated_at = ${input.nowIso}
    WHERE turn_id = ${input.turnId}
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn disappeared during control update")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

const validateExistingTurnIntent = async (
  ctx: MutatorContext,
  intent: KhalaRuntimeControlIntent,
  expectedKind: KhalaRuntimeControlIntentKind,
): Promise<
  | {
      readonly kind: "ok"
      readonly turn: RuntimeTurnRow
      readonly turnId: string
      readonly nowIso: string
    }
  | { readonly kind: "complete"; readonly result: MutationResult }
> => {
  const basics = await validateControlIntentBasics(ctx, intent, expectedKind)
  if (basics !== null) return { kind: "complete", result: basics }

  const turnId = requireTurnId(ctx, intent)
  if (isMutationResult(turnId)) return { kind: "complete", result: turnId }

  const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
  if (ownerRejection !== null) {
    return { kind: "complete", result: ownerRejection }
  }

  const nowIso = await transactionNowIso(ctx)
  const expired = await expireControlIntentIfDue(ctx, intent, nowIso)
  if (expired !== null) return { kind: "complete", result: expired }

  const turn = await readTurnForUpdate(ctx, turnId)
  if (turn === null) {
    return {
      kind: "complete",
      result: reject(
        ctx,
        RUNTIME_TURN_NOT_FOUND_REJECTION,
        "this runtime turn does not exist",
      ),
    }
  }
  if (turn.owner_user_id !== ctx.userId || turn.thread_id !== intent.threadId) {
    return { kind: "complete", result: rejectForeignScope(ctx) }
  }
  if (turn.lane !== intent.target.lane) {
    return {
      kind: "complete",
      result: reject(
        ctx,
        RUNTIME_TARGET_LANE_REJECTION,
        "runtime control target lane does not match the durable turn lane",
      ),
    }
  }

  return { kind: "ok", nowIso, turn, turnId }
}

const executeExistingTurnIntent = async (
  intent: KhalaRuntimeControlIntent,
  ctx: MutatorContext,
  input: {
    readonly expectedKind: KhalaRuntimeControlIntentKind
    readonly status: RuntimeTurnStatus
    readonly controlStatus?: RuntimeControlIntentStatus | undefined
    readonly settled: boolean
  },
): Promise<MutationResult> => {
  const validated = await validateExistingTurnIntent(
    ctx,
    intent,
    input.expectedKind,
  )
  if (validated.kind === "complete") return validated.result

  const nowIso = validated.nowIso
  await insertControlIntent(
    ctx,
    intent,
    input.controlStatus ?? "accepted",
    nowIso,
  )
  const updatedTurn = await updateTurnForIntent(ctx, {
    latestIntentId: intent.intentId,
    nowIso,
    settledAt: input.settled ? nowIso : null,
    status: input.status,
    turnId: validated.turnId,
  })
  await appendRuntimeAgentRunChanges(ctx, updatedTurn)
  await appendRuntimeLiveAgentGraph(ctx, updatedTurn)
  return applied(ctx)
}

export const runtimeRequestInteractionMutator: MutatorDefinition =
  defineMutator<RuntimeInteraction>({
    decodeArgs: decodeRuntimeInteractionArgs,
    execute: async (interaction, ctx) => {
      if (interaction.lifecycle.status !== "pending") {
        return reject(
          ctx,
          RUNTIME_INTERACTION_STATE_REJECTION,
          "new runtime interactions must begin pending",
        )
      }
      const ownerRejection = await ensureRuntimeThreadOwner(
        ctx,
        interaction.threadId,
      )
      if (ownerRejection !== null) return ownerRejection
      const existing = await readInteractionForUpdate(
        ctx,
        interaction.interactionRef,
      )
      if (existing !== null) {
        const recorded = interactionEntityFromRow(existing).interaction
        const originalRequest = {
          ...recorded,
          lifecycle: { status: "pending" as const },
        }
        return existing.owner_user_id === ctx.userId &&
          canonicalJson(originalRequest) === canonicalJson(interaction)
          ? applied(ctx)
          : reject(
              ctx,
              RUNTIME_INTERACTION_CONFLICT_REJECTION,
              "runtime interaction identity was reused with different semantics",
            )
      }
      const turn = await readTurnForUpdate(ctx, interaction.turnId)
      if (turn === null) {
        return reject(
          ctx,
          RUNTIME_TURN_NOT_FOUND_REJECTION,
          "this runtime interaction turn does not exist",
        )
      }
      if (
        turn.owner_user_id !== ctx.userId ||
        turn.thread_id !== interaction.threadId
      ) return rejectForeignScope(ctx)
      if (turn.lane !== interaction.source.lane) {
        return reject(
          ctx,
          RUNTIME_TARGET_LANE_REJECTION,
          "runtime interaction source lane does not match the durable turn lane",
        )
      }
      if (turn.status !== "running" && turn.status !== "waiting_for_input") {
        return reject(
          ctx,
          RUNTIME_INTERACTION_STATE_REJECTION,
          "runtime interaction requires a running or waiting turn",
        )
      }
      if (Number(turn.event_count) !== interaction.requestedSequence) {
        return reject(
          ctx,
          RUNTIME_INTERACTION_SEQUENCE_REJECTION,
          "runtime interaction sequence must equal the durable next event sequence",
        )
      }
      const nowIso = await transactionNowIso(ctx)
      const expiresAt = Date.parse(interaction.expiresAt)
      if (
        !Number.isFinite(expiresAt) ||
        new Date(expiresAt).toISOString() !== interaction.expiresAt ||
        expiresAt <= Date.parse(nowIso)
      ) {
        return reject(
          ctx,
          RUNTIME_INTERACTION_EXPIRY_REJECTION,
          "runtime interaction deadline must be a future ISO timestamp",
        )
      }
      await insertRuntimeInteraction(ctx, interaction, nowIso)
      await updateTurnWaitingForInteraction(ctx, interaction.turnId, nowIso)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME),
  })

export const runtimeDecideInteractionMutator: MutatorDefinition =
  defineMutator<RuntimeInteractionDecisionArgs>({
    decodeArgs: decodeRuntimeInteractionDecisionArgs,
    execute: async (input, ctx) => {
      const ownerRejection = await ensureRuntimeThreadOwner(ctx, input.threadId)
      if (ownerRejection !== null) return ownerRejection
      const row = await readInteractionForUpdate(ctx, input.interactionRef)
      if (row === null) {
        return reject(
          ctx,
          RUNTIME_INTERACTION_STATE_REJECTION,
          "this runtime interaction does not exist",
        )
      }
      if (
        row.owner_user_id !== ctx.userId ||
        row.thread_id !== input.threadId ||
        row.turn_id !== input.turnId
      ) return rejectForeignScope(ctx)
      const entity = interactionEntityFromRow(row)
      const nowIso = await transactionNowIso(ctx)
      const decision = applyRuntimeInteractionDecision(
        entity.interaction,
        input.envelope,
        nowIso,
      )
      if (decision.state === "duplicate") return applied(ctx)
      if (decision.state === "conflict") {
        return reject(
          ctx,
          RUNTIME_INTERACTION_CONFLICT_REJECTION,
          "runtime interaction decision identity conflicts with the settled decision",
        )
      }
      if (decision.state === "invalid_decision") {
        return reject(
          ctx,
          RUNTIME_INTERACTION_DECISION_REJECTION,
          "runtime interaction decision does not match its kind or choices",
        )
      }
      if (decision.state === "revoked") {
        return reject(
          ctx,
          RUNTIME_INTERACTION_STATE_REJECTION,
          "runtime interaction authority was revoked",
        )
      }
      if (decision.state === "applied") {
        const conflicts: Array<{ interaction_ref: string }> =
          await ctx.writer.sql`
            SELECT interaction_ref
            FROM khala_sync_runtime_interactions
            WHERE owner_user_id = ${ctx.userId}
              AND status = 'resolved'
              AND interaction_ref <> ${input.interactionRef}
              AND interaction_json -> 'lifecycle' -> 'envelope'
                    ->> 'idempotencyKey' = ${input.envelope.idempotencyKey}
            LIMIT 1
          `
        if (conflicts.length > 0) {
          return reject(
            ctx,
            RUNTIME_INTERACTION_CONFLICT_REJECTION,
            "runtime interaction decision idempotency key is already bound",
          )
        }
      }
      await updateRuntimeInteraction(ctx, row, decision.interaction, nowIso)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_DECIDE_INTERACTION_MUTATOR_NAME),
  })

export const runtimeStartTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: async (intent, ctx) => {
      const basics = await validateControlIntentBasics(ctx, intent, "turn.start")
      if (basics !== null) return basics

      const turnId = requireTurnId(ctx, intent)
      if (isMutationResult(turnId)) return turnId

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const expired = await expireControlIntentIfDue(ctx, intent, nowIso)
      if (expired !== null) return expired

      const existing = await readTurnForUpdate(ctx, turnId)
      if (existing !== null) {
        return existing.owner_user_id === ctx.userId
          ? reject(
              ctx,
              RUNTIME_TURN_EXISTS_REJECTION,
              "this runtime turn already exists",
            )
          : rejectForeignScope(ctx)
      }

      await insertControlIntent(ctx, intent, "accepted", nowIso)
      const turn = await insertTurn(ctx, intent, nowIso)
      await appendRuntimeAgentRunChanges(ctx, turn)
      await appendRuntimeLiveAgentGraph(ctx, turn)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_START_TURN_MUTATOR_NAME),
  })

export const runtimeAppendUserMessageMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: async (intent, ctx) => {
      const basics = await validateControlIntentBasics(
        ctx,
        intent,
        "message.append",
      )
      if (basics !== null) return basics

      const messageId = requireMessageId(ctx, intent)
      if (isMutationResult(messageId)) return messageId

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, intent.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const expired = await expireControlIntentIfDue(ctx, intent, nowIso)
      if (expired !== null) return expired

      const turnId = intent.turnId
      let turn: RuntimeTurnRow | null = null
      if (turnId !== undefined) {
        turn = await readTurnForUpdate(ctx, turnId)
        if (turn === null) {
          return reject(
            ctx,
            RUNTIME_TURN_NOT_FOUND_REJECTION,
            "this runtime turn does not exist",
          )
        }
        if (turn.owner_user_id !== ctx.userId || turn.thread_id !== intent.threadId) {
          return rejectForeignScope(ctx)
        }
      }

      await insertControlIntent(ctx, intent, "accepted", nowIso)
      if (turn !== null && turnId !== undefined) {
        await updateTurnForIntent(ctx, {
          latestIntentId: intent.intentId,
          nowIso,
          settledAt: turn.settled_at,
          status: turn.status as RuntimeTurnStatus,
          turnId,
        })
      }
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_APPEND_USER_MESSAGE_MUTATOR_NAME),
  })

export const runtimeInterruptTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.interrupt",
        settled: true,
        status: "interrupted",
      }),
    name: MutatorName.make(RUNTIME_INTERRUPT_TURN_MUTATOR_NAME),
  })

export const runtimeContinueTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.continue",
        settled: false,
        status: "queued",
      }),
    name: MutatorName.make(RUNTIME_CONTINUE_TURN_MUTATOR_NAME),
  })

export const runtimeRetryTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        expectedKind: "turn.retry",
        settled: false,
        status: "queued",
      }),
    name: MutatorName.make(RUNTIME_RETRY_TURN_MUTATOR_NAME),
  })

export const runtimeCloseTurnMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeControlIntent>({
    decodeArgs: decodeRuntimeControlIntentArgs,
    execute: (intent, ctx) =>
      executeExistingTurnIntent(intent, ctx, {
        controlStatus: "settled",
        expectedKind: "turn.close",
        settled: true,
        status: "closed",
      }),
    name: MutatorName.make(RUNTIME_CLOSE_TURN_MUTATOR_NAME),
  })

const statusForRuntimeEvent = (
  event: KhalaRuntimeEvent,
  current: RuntimeTurnStatus,
): RuntimeTurnStatus => {
  switch (event.kind) {
    case "turn.started":
      return "running"
    case "turn.interrupted":
      return "interrupted"
    case "turn.finished":
      return turnFinishedStatus(event.finishReason)
    default:
      return current
  }
}

const turnFinishedStatus = (
  finishReason: KhalaRuntimeFinishReason,
): RuntimeTurnStatus => {
  switch (finishReason) {
    case "error":
      return "failed"
    case "cancelled":
    case "interrupted":
      return "interrupted"
    default:
      return "completed"
  }
}

const updateTurnForRuntimeEvent = async (
  ctx: MutatorContext,
  turn: RuntimeTurnRow,
  event: KhalaRuntimeEvent,
  nowIso: string,
): Promise<RuntimeTurnEntity> => {
  const status = statusForRuntimeEvent(event, turn.status as RuntimeTurnStatus)
  const startedAt =
    event.kind === "turn.started" ? (turn.started_at ?? nowIso) : turn.started_at
  const settledAt =
    event.kind === "turn.finished" || event.kind === "turn.interrupted"
      ? nowIso
      : turn.settled_at
  const rows: Array<RuntimeTurnRow> = await ctx.writer.sql`
    UPDATE khala_sync_runtime_turns
    SET status = ${status},
        event_count = event_count + 1,
        started_at = ${startedAt},
        settled_at = ${settledAt},
        updated_at = ${nowIso}
    WHERE turn_id = ${turn.turn_id}
    RETURNING turn_id, thread_id, owner_user_id, lane, status, event_count,
              latest_intent_id, started_at, settled_at, created_at, updated_at
  `
  const row = rows[0]
  if (row === undefined) {
    throw new Error("runtime turn disappeared during event update")
  }
  const entity = turnEntityFromRow(row)
  await appendTurnEntityChanges(ctx, entity)
  return entity
}

export const runtimeRecordEventMutator: MutatorDefinition =
  defineMutator<KhalaRuntimeEvent>({
    decodeArgs: decodeRuntimeEventArgs,
    execute: async (event, ctx) => {
      if (!Number.isSafeInteger(event.sequence) || event.sequence < 0) {
        return reject(
          ctx,
          RUNTIME_EVENT_SEQUENCE_REJECTION,
          "runtime event sequence must be a non-negative safe integer",
        )
      }

      const conflict = await readRuntimeEventConflict(ctx, event)
      if (conflict !== null) {
        return reject(
          ctx,
          RUNTIME_EVENT_EXISTS_REJECTION,
          "this runtime event was already recorded",
        )
      }

      const turn = await readTurnForUpdate(ctx, event.turnId)
      if (turn === null) {
        return reject(
          ctx,
          RUNTIME_TURN_NOT_FOUND_REJECTION,
          "this runtime turn does not exist",
        )
      }
      if (turn.owner_user_id !== ctx.userId || turn.thread_id !== event.threadId) {
        return rejectForeignScope(ctx)
      }

      // `event_count` is the durable provider-generation cursor. Requiring
      // the exact next value rejects delayed, skipped, and replayed messages
      // before any timeline projection mutates.
      if (event.sequence !== Number(turn.event_count)) {
        return reject(
          ctx,
          RUNTIME_EVENT_SEQUENCE_REJECTION,
          "runtime event sequence does not match the durable next sequence",
        )
      }

      const turnStatus = turn.status as RuntimeTurnStatus
      const terminal =
        turnStatus === "completed" ||
        turnStatus === "failed" ||
        turnStatus === "interrupted" ||
        turnStatus === "closed"
      if (
        terminal ||
        (turnStatus === "queued" && event.kind !== "turn.started") ||
        (turnStatus === "running" && event.kind === "turn.started")
      ) {
        return reject(
          ctx,
          RUNTIME_EVENT_STATE_REJECTION,
          "runtime event is not valid for the durable turn state",
        )
      }

      const ownerRejection = await ensureRuntimeThreadOwner(ctx, event.threadId)
      if (ownerRejection !== null) return ownerRejection

      const nowIso = await transactionNowIso(ctx)
      const eventEntity = runtimeEventEntityFromEvent(event, {
        nowIso,
        ownerUserId: ctx.userId,
      })
      // `event_json` is jsonb: bind the OBJECT (see `insertControlIntent`);
      // a pre-stringified string would be double-encoded into a jsonb string
      // scalar.
      await ctx.writer.sql`
        INSERT INTO khala_sync_runtime_events
          (event_id, turn_id, thread_id, owner_user_id, kind, sequence,
           observed_at, event_json, created_at)
        VALUES
          (${eventEntity.eventId}, ${eventEntity.turnId}, ${eventEntity.threadId},
           ${eventEntity.ownerUserId}, ${eventEntity.kind}, ${eventEntity.sequence},
           ${eventEntity.observedAt}, ${eventEntity.event}::jsonb,
           ${eventEntity.createdAt})
      `
      await appendRuntimeEventEntityChange(ctx, eventEntity)
      await appendRuntimeAgentEventChanges(ctx, event, nowIso)
      const updatedTurn = await updateTurnForRuntimeEvent(ctx, turn, event, nowIso)
      await appendRuntimeAgentRunChanges(ctx, updatedTurn)
      await appendRuntimeLiveAgentGraph(ctx, updatedTurn, event)
      return applied(ctx)
    },
    name: MutatorName.make(RUNTIME_RECORD_EVENT_MUTATOR_NAME),
  })

export const runtimeMutators: ReadonlyArray<MutatorDefinition> = [
  runtimeStartTurnMutator,
  runtimeAppendUserMessageMutator,
  runtimeInterruptTurnMutator,
  runtimeContinueTurnMutator,
  runtimeRetryTurnMutator,
  runtimeCloseTurnMutator,
  runtimeRecordEventMutator,
  runtimeRequestInteractionMutator,
  runtimeDecideInteractionMutator,
]
