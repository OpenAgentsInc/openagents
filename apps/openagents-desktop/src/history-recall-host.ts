/**
 * RLM-03 / OPENRLM-SDK (#9154) — desktop main-process wiring for
 * `history_recall` over the published AI SDK train.
 *
 * Resolves against the owner's local stores (HistoryCorpus builder over the
 * durable harness event log + optional thread snapshots), dispatches through
 * HistoryRecall Tier D from `@openagentsinc/history-corpus`, and re-enters the
 * neutral stream as `tool.call` / `tool.result` (payload stays on the
 * host-tool result path as a ref).
 *
 * The same host stores also feed {@link DesktopHistoryCorpusSource} so the
 * first-class `@openagentsinc/rlm` engine can resolve an authorized corpus
 * handle without a second product-local engine. Tier D `history_recall` keeps
 * the full HistoryRecall question vocabulary; Rlm deterministic Grep is the
 * shared engine path for paper-faithful traversal and Tier S follow-on.
 *
 * Boundaries:
 * - Recall output is an untrusted cited candidate — never authority.
 * - Raw history never leaves owner-local execution.
 * - Caps truncate; honesty is required.
 * - Exact-only usage (Tier D records modelCalls: 0).
 * - Model input cannot set visibility/redaction policy or widen scope.
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
  HistoryCorpusError,
  makeHistoryRecallTierD,
  summarizeHistoryRecallAnswer,
  type HistoryCorpusPolicy,
  type HistoryCorpusScope,
  type HistoryRecallResponse,
  type NeutralThreadSnapshot,
} from "@openagentsinc/history-corpus"
import {
  defaultRlmDeterministicLimits,
  makeRlm,
  makeRlmToolHandler,
  type RlmError,
  type RlmShape,
  type RlmTerminalResult,
} from "@openagentsinc/rlm"

import {
  DESKTOP_RLM_STRATEGY_REF,
  decodeHistoryCursorAddress,
  desktopHistoryCorpusInputForScope,
  desktopHistoryCorpusPolicy,
  desktopHistoryCorpusSourceLayer,
  desktopRlmRootLimits,
  type DesktopHistoryCorpusSourceInput,
} from "./desktop-history-corpus-source.ts"

/** Owner-local policy: admit private/operator/public history for local recall. */
export const desktopHistoryRecallPolicy: HistoryCorpusPolicy =
  desktopHistoryCorpusPolicy

export {
  DESKTOP_HISTORY_ADDRESS_SCHEMA_ID,
  DESKTOP_RLM_STRATEGY_REF,
  desktopHistoryCorpusInputForScope,
  desktopHistoryCorpusSourceLayer,
  desktopRlmRootLimits,
  makeDesktopHistoryCorpusSource,
  resolveDesktopHistoryCorpus,
} from "./desktop-history-corpus-source.ts"

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
  /**
   * Session admission for a thread. Defaults to allow-all only for hermetic
   * tests; product hosts must pass a real checker.
   */
  readonly authorizeThread?: (
    threadId: string,
  ) => boolean | Promise<boolean>
  /** Optional run membership; required before Run scopes resolve. */
  readonly threadIdsForRun?: (
    runRef: string,
  ) => ReadonlyArray<string> | Promise<ReadonlyArray<string>>
}

/** Convert host sources into the RLM corpus-source input shape. */
export const toDesktopHistoryCorpusSourceInput = (
  sources: HistoryRecallHostSources,
): DesktopHistoryCorpusSourceInput => ({
  eventLog: sources.eventLog,
  turnIdsForThread: sources.turnIdsForThread,
  ...(sources.threadSnapshot === undefined
    ? {}
    : { threadSnapshot: sources.threadSnapshot }),
  ...(sources.builtAt === undefined ? {} : { builtAt: sources.builtAt }),
  authorizeThread: sources.authorizeThread ?? (() => true),
  ...(sources.threadIdsForRun === undefined
    ? {}
    : { threadIdsForRun: sources.threadIdsForRun }),
  policy: desktopHistoryRecallPolicy,
})

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
        // Fail closed when a product host supplied an authorizer and any thread is denied.
        if (sources.authorizeThread !== undefined) {
          for (const threadId of threadIds) {
            const allowed = yield* Effect.tryPromise({
              try: async () => await sources.authorizeThread!(threadId),
              catch: () => false,
            }).pipe(Effect.orElseSucceed(() => false))
            if (!allowed) {
              return yield* Effect.fail(
                new HistoryCorpusError({
                  operation: "authorize",
                  detail: `thread not authorized for current session (${threadId})`,
                }),
              )
            }
          }
        }
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
 * Build a deterministic-only `Rlm` service over the desktop corpus source.
 * Semantic mode is refused at the Layer (no spend without Tier S admission).
 */
export const makeDesktopRlmDeterministic = (
  sources: HistoryRecallHostSources,
): Effect.Effect<RlmShape> =>
  makeRlm({ admitSemantic: false, model: { refuseSemantic: true } }).pipe(
    Effect.provide(desktopHistoryCorpusSourceLayer(toDesktopHistoryCorpusSourceInput(sources))),
  )

/** Effect Tool-shaped RLM handler (no artifact sink; strategy pin recorded). */
export const makeDesktopRlmToolHandler = (rlm: RlmShape) => {
  const handler = makeRlmToolHandler(rlm)
  return {
    ...handler,
    strategyRef: DESKTOP_RLM_STRATEGY_REF,
    rootLimits: desktopRlmRootLimits,
  }
}

/**
 * Run deterministic Grep through first-class Rlm for an authorized scope.
 * Used for RLM engine parity and as the foundation for Tier S (#9141).
 */
export const runDesktopRlmDeterministicGrep = (
  sources: HistoryRecallHostSources,
  input: {
    readonly scope: HistoryCorpusScope
    readonly pattern: string
    readonly runRef: string
    readonly maxSpans?: number
  },
): Effect.Effect<RlmTerminalResult, RlmError> =>
  Effect.gen(function* () {
    const rlm = yield* makeDesktopRlmDeterministic(sources)
    const maxSpans = input.maxSpans ?? defaultRlmDeterministicLimits.maxSpans
    return yield* rlm.run({
      _tag: "Deterministic",
      schemaId: "openagents.ai.rlm_request.v1",
      runRef: input.runRef,
      corpus: desktopHistoryCorpusInputForScope(input.scope),
      operation: { _tag: "Grep", pattern: input.pattern },
      limits: {
        ...defaultRlmDeterministicLimits,
        maxSpans,
      },
    })
  })

/** Project Rlm findings onto the renderer cited-span rows when addresses match. */
export const citedSpansFromRlmResult = (
  result: RlmTerminalResult,
): ReadonlyArray<HistoryRecallCitedSpanRow> => {
  if (result._tag !== "Completed" && result._tag !== "Partial") return []
  const output =
    result._tag === "Completed"
      ? result.output
      : "bestOutput" in result
        ? result.bestOutput
        : undefined
  if (output === undefined || output._tag !== "DeterministicFindings") return []
  const rows: Array<HistoryRecallCitedSpanRow> = []
  for (const finding of output.findings) {
    const cursor = decodeHistoryCursorAddress(finding.citation.sourceAddress)
    if (cursor === null) continue
    rows.push({
      turnId: cursor.turnId,
      sequenceStart: cursor.sequence,
      sequenceEnd: cursor.sequence,
      excerpt: finding.excerpt,
      kind: "text.delta",
      scopeRef: finding.citation.scopeRef,
    })
  }
  return rows
}

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
