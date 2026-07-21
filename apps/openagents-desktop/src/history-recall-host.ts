/**
 * RLM-03 — desktop main-process wiring for the `history_recall` host tool.
 *
 * Resolves against the owner's local stores (HistoryCorpus builder over the
 * durable harness event log + optional thread snapshots), dispatches through
 * HistoryRecall Tier D, and re-enters the neutral stream as `tool.call` /
 * `tool.result` (payload stays on the host-tool result path as a ref).
 *
 * Boundaries:
 * - Recall output is an untrusted cited candidate — never authority.
 * - Raw history never leaves owner-local execution.
 * - Caps truncate; honesty is required.
 * - Exact-only usage (Tier D records modelCalls: 0).
 */

import { Effect } from "effect"
import {
  HISTORY_RECALL_TOOL_NAME,
  HISTORY_RECALL_TURN_POLICY_CAPABILITY,
  REGISTERED_HARNESS_HOST_TOOLS,
  historyRecallHostToolSpec,
  type HarnessEventLogStore,
  type HarnessHostToolCall,
  type HarnessHostToolResult,
  type HarnessHostToolSpec,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract"
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema"
import {
  buildHistoryCorpus,
  dispatchHistoryRecallHostTool,
  makeHistoryRecallTierD,
  summarizeHistoryRecallAnswer,
  type HistoryCorpusPolicy,
  type HistoryRecallResponse,
  type NeutralThreadSnapshot,
} from "@openagentsinc/history-corpus"

/** Owner-local policy: admit private/operator/public history for local recall. */
export const desktopHistoryRecallPolicy: HistoryCorpusPolicy = {
  includeVisibilities: ["public", "operator", "private"],
  includeRedactionClasses: [
    "public_ref",
    "redacted_summary",
    "operator_summary",
    "private_ref",
  ],
}

/** Host tools the desktop turn policy admits for owner-local lanes. */
export const desktopAdmittedHostTools: ReadonlyArray<HarnessHostToolSpec> =
  REGISTERED_HARNESS_HOST_TOOLS

/** Capability refs for Stack B turn-policy / capability surfaces. */
export const desktopAdmittedHostToolCapabilities: ReadonlyArray<string> = [
  HISTORY_RECALL_TURN_POLICY_CAPABILITY,
]

export interface HistoryRecallHostSources {
  /** Durable harness event log (HARN-02 store). */
  readonly eventLog: HarnessEventLogStore
  /** Turn ids the store holds for the requested scope (caller enumerates). */
  readonly turnIdsForThread: (
    threadId: string,
  ) => ReadonlyArray<string> | Promise<ReadonlyArray<string>>
  /** Optional thread snapshots for display-side gap fill. */
  readonly threadSnapshot?: (
    threadId: string,
  ) => NeutralThreadSnapshot | null | Promise<NeutralThreadSnapshot | null>
  /** Build timestamp supplier — tests inject a fixed clock. */
  readonly builtAt?: () => string
  /** Neutral event source label for re-entry. */
  readonly source?: KhalaRuntimeSource
}

export interface HistoryRecallHostDispatchInput {
  readonly call: HarnessHostToolCall
  readonly turnId: string
  readonly threadId: string
  /** Sequence for the tool.call event (result uses +1). */
  readonly sequence: number
  readonly observedAt?: string
  readonly source?: KhalaRuntimeSource
}

export interface HistoryRecallHostDispatchResult {
  readonly result: HarnessHostToolResult
  readonly neutralEvents: ReadonlyArray<HarnessStreamEvent>
  readonly answer: HistoryRecallResponse | null
  /** Bounded public-safe summary for transcript / tool-card rows. */
  readonly summary: string
  /** Cited span rows ready for the renderer (empty on error). */
  readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
}

/** Renderer-facing cited span — redaction boundary already applied by corpus. */
export interface HistoryRecallCitedSpanRow {
  readonly turnId: string
  readonly sequenceStart: number
  readonly sequenceEnd: number
  readonly excerpt: string
  readonly kind: string
  readonly scopeRef: string
}

const defaultBuiltAt = (): string => new Date().toISOString()

const defaultSource: KhalaRuntimeSource = { lane: "test_fixture" }

const collectThreadIds = (scope: {
  readonly _tag: string
  readonly threadId?: string
  readonly threadIds?: ReadonlyArray<string>
}): ReadonlyArray<string> => {
  if (scope._tag === "Thread" && typeof scope.threadId === "string") {
    return [scope.threadId]
  }
  if (Array.isArray(scope.threadIds)) return scope.threadIds
  return []
}

/**
 * Build a HistoryRecall Tier D service over the host's local stores. Each
 * Scope request rebuilds a deterministic corpus (visibility/redaction
 * filtered) from the event log + optional thread snapshots.
 */
export const makeDesktopHistoryRecall = (sources: HistoryRecallHostSources) =>
  makeHistoryRecallTierD({
    corpusForScope: (scope) =>
      Effect.gen(function* () {
        const threadIds = collectThreadIds(scope)
        const turnIds: Array<string> = []
        const threads: Array<NeutralThreadSnapshot> = []
        for (const threadId of threadIds) {
          const ids = yield* Effect.tryPromise({
            try: async () => await sources.turnIdsForThread(threadId),
            catch: (cause) => cause,
          }).pipe(Effect.orElseSucceed(() => [] as ReadonlyArray<string>))
          turnIds.push(...ids)
          if (sources.threadSnapshot !== undefined) {
            const snap = yield* Effect.tryPromise({
              try: async () => await sources.threadSnapshot!(threadId),
              catch: () => null,
            }).pipe(Effect.orElseSucceed(() => null))
            if (snap !== null) threads.push(snap)
          }
        }
        return yield* buildHistoryCorpus({
          scope,
          eventLog: sources.eventLog,
          turnIds,
          threads,
          policy: desktopHistoryRecallPolicy,
          builtAt: (sources.builtAt ?? defaultBuiltAt)(),
        })
      }),
  })

/**
 * Dispatch one `history_recall` host-tool call: resolve through Tier D,
 * emit neutral stream re-entry, and project a renderer-safe summary + cited
 * spans. Never throws — failures become `isError` results with tool.error
 * re-entry.
 */
export const dispatchDesktopHistoryRecall = (
  sources: HistoryRecallHostSources,
  input: HistoryRecallHostDispatchInput,
): Effect.Effect<HistoryRecallHostDispatchResult> =>
  Effect.gen(function* () {
    const recall = makeDesktopHistoryRecall(sources)
    const dispatched = yield* dispatchHistoryRecallHostTool({
      recall,
      call: input.call,
      stream: {
        turnId: input.turnId,
        threadId: input.threadId,
        source: input.source ?? sources.source ?? defaultSource,
        sequence: input.sequence,
        toolCallId: input.call.toolCallId,
        ...(input.observedAt === undefined ? {} : { observedAt: input.observedAt }),
      },
    })
    const citedSpans: Array<HistoryRecallCitedSpanRow> =
      dispatched.answer?.answers.map((span) => ({
        turnId: span.turnId,
        sequenceStart: span.sequenceStart,
        sequenceEnd: span.sequenceEnd,
        excerpt: span.excerpt,
        kind: span.kind,
        scopeRef: span.scopeRef,
      })) ?? []
    const summary =
      dispatched.answer !== null
        ? summarizeHistoryRecallAnswer(dispatched.answer)
        : dispatched.result.isError === true
          ? `history_recall failed · ${safeErrorLabel(dispatched.result)}`
          : "history_recall · no answer"
    return {
      result: dispatched.result,
      neutralEvents: dispatched.neutralEvents,
      answer: dispatched.answer,
      summary,
      citedSpans,
    }
  })

const safeErrorLabel = (result: HarnessHostToolResult): string => {
  if (typeof result.output === "object" && result.output !== null) {
    const record = result.output as Record<string, unknown>
    if (typeof record.error === "string") return record.error.slice(0, 120)
    if (typeof record.detail === "string") return record.detail.slice(0, 120)
  }
  return "error"
}

/**
 * Synchronous convenience for main-process callbacks that cannot be Effects.
 * Failures are still typed isError results — never thrown.
 */
export const dispatchDesktopHistoryRecallSync = (
  sources: HistoryRecallHostSources,
  input: HistoryRecallHostDispatchInput,
): HistoryRecallHostDispatchResult =>
  Effect.runSync(dispatchDesktopHistoryRecall(sources, input))

/** Whether a tool name is the registered history_recall host tool. */
export const isHistoryRecallHostTool = (toolName: string): boolean =>
  toolName === HISTORY_RECALL_TOOL_NAME

/** The wire spec handed to `promptTurn({ tools })` for owner-local lanes. */
export const historyRecallToolsForTurn = (): ReadonlyArray<HarnessHostToolSpec> => [
  historyRecallHostToolSpec,
]

/**
 * Stack B turn-policy exposure: admitted host-tool capability refs for a
 * local turn. Composed into desktop turn policy / provider start without
 * changing route decisions.
 */
export const desktopHostToolCapabilitiesForTurn = (): ReadonlyArray<string> =>
  desktopAdmittedHostToolCapabilities
