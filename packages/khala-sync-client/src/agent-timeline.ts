import {
  AGENT_RUN_ENTITY_TYPE,
  AGENT_RUN_EVENT_ENTITY_TYPE,
  agentRunScope,
  decodeKhalaRuntimeEvent,
  decodeAgentRunEntity,
  decodeAgentRunEventEntity,
  RUNTIME_INTERACTION_ENTITY_TYPE,
  threadScope,
} from "@openagentsinc/khala-sync"
import { Effect, Schema } from "effect"
import type { OverlayError } from "./overlay.js"
import type { KhalaSyncSession, ScopeSyncState } from "./session.js"
import type {
  ConfirmedEntity,
  KhalaSyncClientStoreError,
  KhalaSyncLocalStore,
} from "./store.js"
import {
  confirmedRuntimeInteractions,
  type ConfirmedRuntimeInteraction,
} from "./runtime-interactions.js"

export const MAX_CONFIRMED_AGENT_TIMELINE_EVENTS = 500

const TimelineRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(1_024),
)
const TimelineTimestampSchema = Schema.String.check(Schema.isMaxLength(64))
const TimelineLabelSchema = Schema.String.check(Schema.isMaxLength(256))
const TimelineDetailSchema = Schema.String.check(Schema.isMaxLength(2_000))
const TimelineTextSchema = Schema.String.check(Schema.isMaxLength(20_000))
const TimelineIntSchema = Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
)

const TimelineInteractionOptionSchema = Schema.Struct({
  optionRef: TimelineRefSchema,
  label: TimelineLabelSchema,
  description: Schema.optional(TimelineDetailSchema),
})

const TimelineInteractionQuestionSchema = Schema.Struct({
  questionRef: TimelineRefSchema,
  displayText: TimelineDetailSchema,
  options: Schema.Array(TimelineInteractionOptionSchema).check(Schema.isMaxLength(12)),
  multiSelect: Schema.Boolean,
})

export const ConfirmedAgentTimelineItemSchema = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("connected"), turnRef: TimelineRefSchema, lane: TimelineLabelSchema }),
  Schema.Struct({ kind: Schema.Literal("heartbeat"), detail: TimelineDetailSchema }),
  Schema.Struct({ kind: Schema.Literal("text"), messageRef: TimelineRefSchema, text: TimelineTextSchema }),
  Schema.Struct({ kind: Schema.Literal("reasoning"), messageRef: TimelineRefSchema, text: TimelineTextSchema }),
  Schema.Struct({
    kind: Schema.Literal("plan"),
    stepRef: TimelineRefSchema,
    status: TimelineLabelSchema,
    interactionRef: Schema.optional(TimelineRefSchema),
    prompt: Schema.optional(TimelineTextSchema),
    expiresAt: Schema.optional(TimelineTimestampSchema),
    decisionRef: Schema.optional(TimelineRefSchema),
  }),
  Schema.Struct({ kind: Schema.Literal("tool"), toolCallRef: TimelineRefSchema, toolName: TimelineLabelSchema, status: Schema.Literals(["called", "completed", "failed"]) }),
  Schema.Struct({
    kind: Schema.Literal("question"),
    questionRef: TimelineRefSchema,
    prompt: TimelineTextSchema,
    status: Schema.optional(TimelineLabelSchema),
    title: Schema.optional(TimelineLabelSchema),
    questions: Schema.optional(Schema.Array(TimelineInteractionQuestionSchema).check(Schema.isMaxLength(8))),
    expiresAt: Schema.optional(TimelineTimestampSchema),
    decisionRef: Schema.optional(TimelineRefSchema),
  }),
  Schema.Struct({
    kind: Schema.Literal("approval"),
    authorityRef: Schema.optional(TimelineRefSchema),
    toolRef: Schema.optional(TimelineRefSchema),
    status: TimelineLabelSchema,
    interactionRef: Schema.optional(TimelineRefSchema),
    prompt: Schema.optional(TimelineTextSchema),
    expiresAt: Schema.optional(TimelineTimestampSchema),
    decisionRef: Schema.optional(TimelineRefSchema),
  }),
  Schema.Struct({ kind: Schema.Literal("error"), messageSafe: TimelineTextSchema }),
  Schema.Struct({ kind: Schema.Literal("usage"), inputTokens: Schema.optional(TimelineIntSchema), outputTokens: Schema.optional(TimelineIntSchema), totalTokens: Schema.optional(TimelineIntSchema) }),
  Schema.Struct({ kind: Schema.Literal("stale"), detail: TimelineDetailSchema }),
  Schema.Struct({ kind: Schema.Literal("reconnect"), detail: TimelineDetailSchema }),
  Schema.Struct({ kind: Schema.Literal("interrupted"), reasonRef: Schema.optional(TimelineRefSchema) }),
  Schema.Struct({ kind: Schema.Literal("terminal"), status: Schema.Literals(["completed", "failed", "canceled"]) }),
])
export type ConfirmedAgentTimelineItem =
  typeof ConfirmedAgentTimelineItemSchema.Type

export const ConfirmedAgentRunSchema = Schema.Struct({
  runRef: TimelineRefSchema,
  routeRef: TimelineRefSchema,
  workContextRef: Schema.optionalKey(TimelineRefSchema),
  runtime: Schema.optionalKey(Schema.Literals(["opencode_codex", "codex", "claude_code", "openagents_native"])),
  backend: Schema.optionalKey(Schema.Literals(["shc_vm", "gcloud_vm", "pylon", "hosted"])),
  status: Schema.Literals(["queued", "running", "waiting_for_input", "completed", "failed", "canceled"]),
  createdAt: TimelineTimestampSchema,
  updatedAt: TimelineTimestampSchema,
  startedAt: Schema.NullOr(TimelineTimestampSchema),
  completedAt: Schema.NullOr(TimelineTimestampSchema),
  failedAt: Schema.NullOr(TimelineTimestampSchema),
  canceledAt: Schema.NullOr(TimelineTimestampSchema),
  version: TimelineIntSchema,
})
export type ConfirmedAgentRun = typeof ConfirmedAgentRunSchema.Type

export const ConfirmedAgentTimelineEventSchema = Schema.Struct({
  eventRef: TimelineRefSchema,
  runRef: TimelineRefSchema,
  sequence: TimelineIntSchema,
  eventType: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256)),
  summary: Schema.String.check(Schema.isMaxLength(20_000)),
  status: Schema.NullOr(Schema.String.check(Schema.isMaxLength(256))),
  artifactRefs: Schema.Array(TimelineRefSchema).check(Schema.isMaxLength(100)),
  item: Schema.optionalKey(Schema.NullOr(ConfirmedAgentTimelineItemSchema)),
  createdAt: TimelineTimestampSchema,
  version: TimelineIntSchema,
})
export type ConfirmedAgentTimelineEvent =
  typeof ConfirmedAgentTimelineEventSchema.Type

export type KhalaSyncAgentTimelineStatus = Readonly<{
  phase: ScopeSyncState["phase"]
  cursor: number | null
  pendingMutationCount: number
}>

export type ConfirmedAgentTimelineSnapshot = Readonly<{
  status: KhalaSyncAgentTimelineStatus
  run: ConfirmedAgentRun | null
  events: ReadonlyArray<ConfirmedAgentTimelineEvent>
}>

export type KhalaSyncAgentTimeline = Readonly<{
  status: (runRef: string) => KhalaSyncAgentTimelineStatus
  open: (runRef: string) => Effect.Effect<void, OverlayError>
  snapshot: (runRef: string) => Effect.Effect<
    ConfirmedAgentTimelineSnapshot,
    KhalaSyncClientStoreError
  >
  snapshotForThread: (threadRef: string) => Effect.Effect<
    ConfirmedAgentTimelineSnapshot,
    KhalaSyncClientStoreError
  >
}>

const cursorFromState = (state: ScopeSyncState): number | null =>
  state.phase === "live" || state.phase === "catching_up"
    ? Number(state.cursor)
    : null

const confirmedRun = (
  runRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ConfirmedAgentRun | null => {
  let result: ConfirmedAgentRun | null = null
  for (const row of rows) {
    try {
      const run = decodeAgentRunEntity(JSON.parse(row.postImageJson) as unknown)
      if (run.runId !== runRef) continue
      if (result !== null && result.version >= Number(row.version)) continue
      result = {
        runRef: run.runId,
        routeRef: run.routeId,
        ...(run.workContextRef === undefined ? {} : { workContextRef: run.workContextRef }),
        runtime: run.runtime,
        backend: run.backend,
        status: run.status,
        createdAt: run.createdAt,
        updatedAt: run.updatedAt,
        startedAt: run.startedAt,
        completedAt: run.completedAt,
        failedAt: run.failedAt,
        canceledAt: run.canceledAt,
        version: Number(row.version),
      }
    } catch {
      // Ignore malformed/pre-contract rows; confirmed replacement self-heals.
    }
  }
  return result
}

const confirmedRunForThread = (
  threadRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ConfirmedAgentRun | null => {
  let result: ConfirmedAgentRun | null = null
  for (const row of rows) {
    try {
      const run = decodeAgentRunEntity(JSON.parse(row.postImageJson) as unknown)
      if (run.routeId !== threadRef) continue
      if (result !== null && result.version >= Number(row.version)) continue
      result = {
        runRef: run.runId,
        routeRef: run.routeId,
        ...(run.workContextRef === undefined ? {} : { workContextRef: run.workContextRef }),
        runtime: run.runtime,
        backend: run.backend,
        status: run.status,
        createdAt: run.createdAt,
        updatedAt: run.updatedAt,
        startedAt: run.startedAt,
        completedAt: run.completedAt,
        failedAt: run.failedAt,
        canceledAt: run.canceledAt,
        version: Number(row.version),
      }
    } catch {
      // Ignore malformed/pre-contract rows; confirmed replacement self-heals.
    }
  }
  return result
}

const safeTokenCount = (value: number | undefined): number | undefined =>
  value === undefined || !Number.isSafeInteger(value) || value < 0
    ? undefined
    : value

const projectedTimelineItem = (payloadJson: string | null): ConfirmedAgentTimelineItem | null => {
  if (payloadJson === null) return null
  try {
    const event = decodeKhalaRuntimeEvent(JSON.parse(payloadJson) as unknown)
    switch (event.kind) {
      case "turn.started":
        return { kind: "connected", lane: event.source.lane, turnRef: event.turnId }
      case "text.delta":
        return { kind: "text", messageRef: event.messageId, text: event.text.slice(0, 20_000) }
      case "reasoning.delta":
        return { kind: "reasoning", messageRef: event.messageId, text: event.text.slice(0, 20_000) }
      case "step.started":
        return { kind: "plan", status: "running", stepRef: event.stepId }
      case "step.finished":
        return { kind: "plan", status: event.finishReason, stepRef: event.stepId }
      case "tool.call":
        if (event.authority.status === "operator_escalation_required") {
          return {
            authorityRef: event.authority.authorityRef,
            kind: "approval",
            status: event.authority.status,
            toolRef: event.authority.toolRef,
          }
        }
        return { kind: "tool", status: "called", toolCallRef: event.toolCallId, toolName: event.toolName.slice(0, 256) }
      case "tool.result":
        return { kind: "tool", status: "completed", toolCallRef: event.toolCallId, toolName: event.toolName.slice(0, 256) }
      case "tool.error":
        return { kind: "error", messageSafe: event.messageSafe.slice(0, 20_000) }
      case "usage.recorded":
        {
          const inputTokens = safeTokenCount(event.usage.inputTokens)
          const outputTokens = safeTokenCount(event.usage.outputTokens)
          const totalTokens = safeTokenCount(event.usage.totalTokens)
          return {
            kind: "usage",
            ...(inputTokens === undefined ? {} : { inputTokens }),
            ...(outputTokens === undefined ? {} : { outputTokens }),
            ...(totalTokens === undefined ? {} : { totalTokens }),
          }
        }
      case "provider.metadata":
        return { detail: "Provider connection active.", kind: "heartbeat" }
      case "turn.interrupted":
        return {
          kind: "interrupted",
          ...(event.reasonRef === undefined ? {} : { reasonRef: event.reasonRef }),
        }
      case "turn.finished":
        return {
          kind: "terminal",
          status: event.finishReason === "error"
            ? "failed"
            : event.finishReason === "cancelled" || event.finishReason === "interrupted"
              ? "canceled"
              : "completed",
        }
      default:
        return null
    }
  } catch {
    return null
  }
}

const confirmedEvents = (
  runRef: string,
  rows: ReadonlyArray<ConfirmedEntity>,
): ReadonlyArray<ConfirmedAgentTimelineEvent> => {
  const byRef = new Map<string, ConfirmedAgentTimelineEvent>()
  for (const row of rows) {
    try {
      const event = decodeAgentRunEventEntity(JSON.parse(row.postImageJson) as unknown)
      if (event.runId !== runRef) continue
      const projected: ConfirmedAgentTimelineEvent = {
        eventRef: event.id,
        runRef: event.runId,
        sequence: event.sequence,
        eventType: event.type,
        summary: event.summary,
        status: event.status,
        artifactRefs: event.artifactRefs,
        item: projectedTimelineItem(event.payloadJson),
        createdAt: event.createdAt,
        version: Number(row.version),
      }
      const previous = byRef.get(projected.eventRef)
      if (previous === undefined || previous.version < projected.version) {
        byRef.set(projected.eventRef, projected)
      }
    } catch {
      // Ignore malformed/pre-contract rows; confirmed replacement self-heals.
    }
  }
  return [...byRef.values()]
    .sort((left, right) =>
      left.sequence - right.sequence || left.eventRef.localeCompare(right.eventRef))
    .slice(-MAX_CONFIRMED_AGENT_TIMELINE_EVENTS)
}

const interactionTimelineItem = (
  interaction: ConfirmedRuntimeInteraction,
): ConfirmedAgentTimelineItem => {
  const terminalRef = interaction.decisionRef === undefined
    ? {}
    : { decisionRef: interaction.decisionRef }
  switch (interaction.kind) {
    case "provider_question":
      return {
        kind: "question",
        questionRef: interaction.interactionRef,
        prompt: interaction.displayText,
        status: interaction.status,
        title: interaction.displayTitle,
        questions: interaction.questions,
        expiresAt: interaction.expiresAt,
        ...terminalRef,
      }
    case "tool_approval":
      return {
        kind: "approval",
        interactionRef: interaction.interactionRef,
        prompt: interaction.displayText,
        status: interaction.status,
        expiresAt: interaction.expiresAt,
        ...terminalRef,
      }
    case "plan_review":
      return {
        kind: "plan",
        stepRef: interaction.interactionRef,
        interactionRef: interaction.interactionRef,
        prompt: interaction.displayText,
        status: interaction.status,
        expiresAt: interaction.expiresAt,
        ...terminalRef,
      }
  }
}

const interactionTimelineEvents = (
  runRef: string,
  interactions: ReadonlyArray<ConfirmedRuntimeInteraction>,
): ReadonlyArray<ConfirmedAgentTimelineEvent> => interactions
  .filter(interaction => interaction.turnId === runRef)
  .map(interaction => ({
    eventRef: interaction.interactionRef,
    runRef,
    sequence: interaction.requestedSequence,
    eventType: `runtime.interaction.${interaction.kind}`,
    summary: interaction.displayTitle,
    status: interaction.status,
    artifactRefs: [],
    item: interactionTimelineItem(interaction),
    createdAt: interaction.requestedAt,
    version: interaction.version,
  }))

const mergeTimelineEvents = (
  events: ReadonlyArray<ConfirmedAgentTimelineEvent>,
  interactions: ReadonlyArray<ConfirmedAgentTimelineEvent>,
): ReadonlyArray<ConfirmedAgentTimelineEvent> => [...events, ...interactions]
  .sort((left, right) =>
    left.sequence - right.sequence || left.eventRef.localeCompare(right.eventRef))
  .slice(-MAX_CONFIRMED_AGENT_TIMELINE_EVENTS)

export const createKhalaSyncAgentTimeline = (input: Readonly<{
  store: KhalaSyncLocalStore
  session: KhalaSyncSession
}>): KhalaSyncAgentTimeline => {
  const status = (runRef: string): KhalaSyncAgentTimelineStatus => {
    const state = input.session.state(agentRunScope(runRef))
    return {
      phase: state.phase,
      cursor: cursorFromState(state),
      pendingMutationCount: input.session.pending().length,
    }
  }

  return {
    status,
    open: runRef => input.session.subscribe(agentRunScope(runRef)),
    snapshot: runRef => {
      const timelineStatus = status(runRef)
      if (timelineStatus.phase !== "live") {
        return Effect.succeed({ status: timelineStatus, run: null, events: [] })
      }
      const scope = agentRunScope(runRef)
      return Effect.map(
        Effect.all([
          input.store.readEntities(scope, AGENT_RUN_ENTITY_TYPE),
          input.store.readEntities(scope, AGENT_RUN_EVENT_ENTITY_TYPE),
        ]),
        ([runRows, eventRows]) => ({
          status: timelineStatus,
          run: confirmedRun(runRef, runRows),
          events: confirmedEvents(runRef, eventRows),
        }),
      )
    },
    snapshotForThread: threadRef => {
      const state = input.session.state(threadScope(threadRef))
      const timelineStatus: KhalaSyncAgentTimelineStatus = {
        phase: state.phase,
        cursor: cursorFromState(state),
        pendingMutationCount: input.session.pending().length,
      }
      if (timelineStatus.phase !== "live") {
        return Effect.succeed({ status: timelineStatus, run: null, events: [] })
      }
      const scope = threadScope(threadRef)
      return Effect.map(
        Effect.all([
          input.store.readEntities(scope, AGENT_RUN_ENTITY_TYPE),
          input.store.readEntities(scope, AGENT_RUN_EVENT_ENTITY_TYPE),
          input.store.readEntities(scope, RUNTIME_INTERACTION_ENTITY_TYPE),
        ]),
        ([runRows, eventRows, interactionRows]) => {
          const run = confirmedRunForThread(threadRef, runRows)
          return {
            status: timelineStatus,
            run,
            events: run === null
              ? []
              : mergeTimelineEvents(
                  confirmedEvents(run.runRef, eventRows),
                  interactionTimelineEvents(
                    run.runRef,
                    confirmedRuntimeInteractions(threadRef, interactionRows),
                  ),
                ),
          }
        },
      )
    },
  }
}
