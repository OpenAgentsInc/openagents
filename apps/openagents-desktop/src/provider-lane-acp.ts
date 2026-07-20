/**
 * ACP canonical vocabulary → provider-lane stream envelope (L1 #8899).
 *
 * The ACP runtime bridge (#8891, `@openagentsinc/agent-client-runtime-bridge`)
 * projects Grok/Cursor peer sessions into a canonical event vocabulary
 * (`AcpProjectionEvent` = `KhalaRuntimeEvent` runtime events + bounded
 * canonical state snapshots). The provider lane SPI's stream envelope is the
 * frozen claude-local event envelope (see ./provider-lane.ts) — deliberately
 * NOT a third vocabulary. This module is the explicit, typed mapping between
 * the two, so the ACP session lanes (#8892) implement `ProviderLane` by
 * folding their projector output through ONE audited function instead of
 * hand-wiring a new stream shape.
 *
 * Mapping policy (public-safe, no synthesis):
 * - Only events with an exact envelope counterpart map; everything else
 *   returns null with the rationale documented inline. A null is "this fact
 *   rides another surface", never a silent drop of transcript content.
 * - Every string is bounded to the envelope's frozen limits.
 * - Usage maps only the exact fields the provider reported; the mapping never
 *   invents token splits (a partial split degrades to the honest subset).
 * - Bridge `degraded` states surface as VISIBLE `lane_notice` lines,
 *   matching the envelope's no-silent-degradation law.
 */
import type { AcpCanonicalStateEvent, AcpProjectionEvent } from "@openagentsinc/agent-client-runtime-bridge"
import {
  CLAUDE_LOCAL_DELTA_LIMIT,
  CLAUDE_LOCAL_PLAN_ENTRY_LIMIT,
  CLAUDE_LOCAL_SUMMARY_LIMIT,
  type ClaudeLocalEvent,
} from "./claude-local-contract.ts"
import {
  type ProviderLane,
  type ProviderLaneCapabilityReport,
  type ProviderLaneHistoryMessage,
  type ProviderLaneTurnResult,
} from "./provider-lane.ts"

const summary = (value: string): string => value.slice(0, CLAUDE_LOCAL_SUMMARY_LIMIT)
const itemRef = (value: string): string => value.slice(0, 120)
const toolName = (value: string): string => value.slice(0, 120)

const isCanonicalState = (event: AcpProjectionEvent): event is AcpCanonicalStateEvent =>
  typeof (event as { stateRef?: unknown }).stateRef === "string"

type PlanEntryStatus = "pending" | "in_progress" | "completed"
const planStatus = (value: unknown): PlanEntryStatus =>
  value === "in_progress" || value === "completed" ? value : "pending"

/**
 * Map one bridge projection event onto the frozen lane stream envelope.
 * Returns null for events that have no envelope counterpart (state
 * snapshots owned by L2 capability surfaces, sidecar refs, provider
 * metadata, and stream-internal boundaries the envelope derives itself).
 */
export const acpProjectionEventToLaneEvent = (
  event: AcpProjectionEvent,
): ClaudeLocalEvent | null => {
  if (isCanonicalState(event)) {
    switch (event.kind) {
      // Plan replacement snapshot → the envelope's replace-rendered plan
      // card. Entry text is the bridge's bounded public-safe content ref —
      // the bridge deliberately redacts raw plan text into refs, and this
      // mapping never un-redacts.
      case "plan-snapshot": {
        const entries = Array.isArray((event.snapshot as { entries?: unknown }).entries)
          ? ((event.snapshot as { entries: ReadonlyArray<Record<string, unknown>> }).entries)
          : []
        return {
          kind: "plan_updated",
          entries: entries.slice(0, CLAUDE_LOCAL_PLAN_ENTRY_LIMIT).map(entry => ({
            step: summary(String(entry.contentRef ?? entry.entryRef ?? "")),
            status: planStatus(entry.status),
          })),
        }
      }
      // A quarantined/degraded projection is a VISIBLE lane notice — the
      // envelope's no-silent-degradation law.
      case "degraded":
        return { kind: "lane_notice", text: summary(event.safeSummary) }
      // Session state snapshots (modes, commands, config, session info,
      // usage/context snapshots, retained user chunks) are capability/state
      // surfaces (L2), not turn-stream transcript items.
      case "user-message":
      case "available-commands":
      case "mode-snapshot":
      case "config-snapshot":
      case "session-info":
      case "usage-snapshot":
        return null
    }
  }
  switch (event.kind) {
    case "turn.started":
      // The dispatcher attaches the persisted thread snapshot on forward.
      return { kind: "turn_started" }
    case "turn.finished": {
      const usage = event.usage
      const exactSplit = usage !== undefined &&
        typeof usage.inputTokens === "number" &&
        typeof usage.cacheReadInputTokens === "number" &&
        typeof usage.outputTokens === "number" &&
        typeof usage.reasoningTokens === "number" &&
        typeof usage.totalTokens === "number"
      return {
        kind: "turn_completed",
        totalTokens: usage?.totalTokens ?? null,
        // Exact split only when the provider reported one — the mapping
        // never fabricates a five-field split from a bare total.
        ...(exactSplit
          ? {
              usage: {
                inputTokens: usage.inputTokens,
                cachedInputTokens: usage.cacheReadInputTokens,
                outputTokens: usage.outputTokens,
                reasoningTokens: usage.reasoningTokens,
                totalTokens: usage.totalTokens,
              },
            }
          : {}),
      }
    }
    case "turn.interrupted":
      return { kind: "turn_failed", reason: "interrupted", detail: summary(event.reasonRef ?? "turn interrupted") }
    case "text.delta":
      return { kind: "text_delta", text: event.text.slice(0, CLAUDE_LOCAL_DELTA_LIMIT) }
    case "reasoning.delta":
      return { kind: "reasoning", text: summary(event.text) }
    case "tool.call":
      return {
        kind: "tool_use",
        toolName: toolName(event.toolName),
        summary: "",
        itemRef: itemRef(event.toolCallId),
      }
    case "tool.result":
      return {
        kind: "tool_result",
        toolName: toolName(event.toolName),
        ok: true,
        summary: "",
        itemRef: itemRef(event.toolCallId),
      }
    case "tool.error":
      return {
        kind: "tool_result",
        toolName: toolName(event.toolName),
        ok: false,
        summary: summary(event.messageSafe),
        itemRef: itemRef(event.toolCallId),
      }
    // Exact usage records ride the envelope's context/usage meter — only the
    // fields the provider actually reported are carried, never a zero fill.
    case "usage.recorded":
      return {
        kind: "meter_updated",
        ...(event.usage.inputTokens === undefined ? {} : { inputTokens: event.usage.inputTokens }),
        ...(event.usage.cacheReadInputTokens === undefined
          ? {}
          : { cachedInputTokens: event.usage.cacheReadInputTokens }),
        ...(event.usage.outputTokens === undefined ? {} : { outputTokens: event.usage.outputTokens }),
        ...(event.usage.reasoningTokens === undefined
          ? {}
          : { reasoningTokens: event.usage.reasoningTokens }),
        ...(event.usage.totalTokens === undefined ? {} : { totalTokens: event.usage.totalTokens }),
      }
    // Stream-internal boundaries: the envelope derives segment boundaries
    // from any non-text event, so completed/step markers add nothing.
    case "text.completed":
    case "reasoning.completed":
    case "step.started":
    case "step.finished":
    // Incremental tool-input streaming has no envelope counterpart; the
    // typed tool card lands on tool.call/tool.result.
    case "tool.input.delta":
    case "tool.input.completed":
    // ACP child-agent lifecycle projects through the session lane's own
    // delegate surface (#8892) — the envelope's child_* events require
    // account attribution ACP peers do not report.
    case "agent.child.started":
    case "agent.child.progress":
    case "agent.child.finished":
    // Evidence refs and provider metadata are sidecar/authority surfaces,
    // never renderer transcript items.
    case "provider.metadata":
    case "file.change":
    case "writeback.recorded":
    case "compaction.recorded":
    case "raw.sidecar_ref":
      return null
  }
}

/**
 * Narrow runtime seam supplied by ACP-5 (#8892). The provider-lane adapter
 * owns vocabulary conversion; the ACP session runtime owns process/session
 * lifecycle and emits only canonical bridge events here.
 */
export type AcpProviderLaneDriver = Readonly<{
  runTurn: (input: Readonly<{
    threadRef: string
    turnRef: string
    model: string
    history: ReadonlyArray<ProviderLaneHistoryMessage>
    message: string
    background: boolean
    emit: (event: AcpProjectionEvent) => void
  }>) => Promise<ProviderLaneTurnResult>
  interrupt: (turnRef: string) => boolean
}>

/**
 * The ACP bridge's concrete `ProviderLane` implementation. Grok and Cursor
 * instantiate this same adapter with different truthful capability reports;
 * neither gets a private dispatcher or renderer projection.
 */
export const makeAcpProviderLane = (input: Readonly<{
  laneRef: string
  graphLaneRef: string
  eventChannel: string
  capabilities: ProviderLaneCapabilityReport
  driver: AcpProviderLaneDriver
}>): ProviderLane<null> => ({
  laneRef: input.laneRef,
  graphLaneRef: input.graphLaneRef,
  eventChannel: input.eventChannel,
  usageProvider: input.capabilities.provider,
  capabilities: () => input.capabilities,
  admit: request => {
    if (request.target !== undefined) {
      return { ok: false, error: "That built-in provider target is not available on this ACP lane." }
    }
    if (request.skill !== undefined) {
      return { ok: false, error: "Local Claude skills are not available on this ACP lane." }
    }
    if (request.permissionMode === "plan_only" && !input.capabilities.features.planOnly) {
      return { ok: false, error: "Plan-only permission mode is not available on this ACP lane." }
    }
    const model = request.model !== undefined && input.capabilities.models.includes(request.model)
      ? request.model
      : input.capabilities.models[0]
    return model === undefined
      ? { ok: false, error: "This ACP lane has no available model." }
      : { ok: true, model, context: null }
  },
  streamMeta: ctx => ({
    lane: input.laneRef,
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? ctx.requestedModel,
  }),
  modelNoteText: model => `ACP · ${model}`,
  runTurn: ({ request, model, history, message, background, emit }) =>
    input.driver.runTurn({
      threadRef: request.threadRef,
      turnRef: request.turnRef,
      model,
      history,
      message,
      background,
      emit: event => {
        const mapped = acpProjectionEventToLaneEvent(event)
        if (mapped !== null) emit(mapped)
      },
    }),
  interrupt: turnRef => input.driver.interrupt(turnRef),
  finalMeta: ctx => ({
    lane: input.laneRef,
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? ctx.requestedModel,
    ...(ctx.result.accountRef === undefined ? {} : { accountRef: ctx.result.accountRef }),
    ...(ctx.result.providerSessionRef === undefined || ctx.result.providerSessionRef === null
      ? {}
      : { requestId: ctx.result.providerSessionRef }),
    totalTokens: ctx.result.totalTokens,
    durationMs: ctx.durationMs,
  }),
  failureMessage: (reason, detail) =>
    `The ACP lane turn failed (${reason}${detail === "" ? "" : ` · ${summary(detail)}`}).`,
})
