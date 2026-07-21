import { Effect, Schema as S } from "effect";
import { Tool, Toolkit } from "effect/unstable/ai";
import {
  HISTORY_RECALL_TOOL_DESCRIPTION,
  HISTORY_RECALL_TOOL_NAME,
  buildToolCall,
  buildToolError,
  buildToolResult,
  harnessHostToolSpecFromTool,
  historyRecallHostToolSpec,
  resolveHostToolCall,
  type HarnessHostToolCall,
  type HarnessHostToolResult,
  type HarnessHostToolSpec,
  type HarnessStreamEvent,
} from "@openagentsinc/agent-harness-contract";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";

import { HistoryCorpusScope } from "./corpus.ts";
import {
  HistoryRecallCaps,
  HistoryRecallQuestion,
  HistoryRecallResponse,
  type HistoryRecallRequest,
  type HistoryRecallResponse as HistoryRecallResponseType,
} from "./recall.ts";
import { HistoryRecall, type HistoryRecallShape } from "./recall-tier-d.ts";

/**
 * `history_recall` host tool (RLM-03, #9139).
 *
 * Effect AI `Tool` authoring form + Toolkit handler Layer over
 * {@link HistoryRecall}. The wire form is registered in
 * `@openagentsinc/agent-harness-contract` (`historyRecallHostToolSpec`);
 * STREAM-07's `harnessHostToolSpecFromTool` projects this Tool onto that
 * shape so one definition serves both Effect AI lanes and harness adapters.
 *
 * Boundaries (audit §5.2–5.5):
 * - Output is an untrusted cited candidate — never authority.
 * - Caps truncate; honesty is required on every answer.
 * - Raw history never leaves owner-local execution (handlers only see
 *   already-filtered corpus entries the host provided).
 * - Neutral stream re-entry is `tool.call` / `tool.result` with refs only —
 *   the bounded answer payload stays on the host-tool result path.
 */

/** Parameters the model supplies. Corpus is always scope-resolved by the host. */
export const HistoryRecallHostToolParams = S.Struct({
  scope: HistoryCorpusScope,
  question: HistoryRecallQuestion,
  caps: S.optionalKey(HistoryRecallCaps),
});
export interface HistoryRecallHostToolParams
  extends S.Schema.Type<typeof HistoryRecallHostToolParams> {}

/** Success payload: the bounded HistoryRecall response (cited spans + honesty). */
export const HistoryRecallHostToolSuccess = HistoryRecallResponse;
export type HistoryRecallHostToolSuccess = HistoryRecallResponseType;

/**
 * Effect AI Tool for `history_recall`. No approval gate: the tool is
 * owner-local, read-only, and already budget-capped. Typed HistoryRecall
 * failures and parameter decode errors fold into harness `isError` results
 * through {@link resolveHostToolCall} (STREAM-07).
 */
export const HistoryRecallTool = Tool.make(HISTORY_RECALL_TOOL_NAME, {
  description: HISTORY_RECALL_TOOL_DESCRIPTION,
  parameters: HistoryRecallHostToolParams,
  success: HistoryRecallHostToolSuccess,
});

/** Toolkit containing only `history_recall` (STREAM-07 composition unit). */
export const HistoryRecallToolkit = Toolkit.make(HistoryRecallTool);

/**
 * Project the Effect Tool onto the harness wire form. Tests assert this
 * matches the registered {@link historyRecallHostToolSpec} name.
 */
export const historyRecallToolWireSpec: HarnessHostToolSpec =
  harnessHostToolSpecFromTool(HistoryRecallTool);

/** Assert the authoring Tool projects to the registered wire name. */
export const historyRecallWireNameMatchesRegistration =
  historyRecallToolWireSpec.name === historyRecallHostToolSpec.name &&
  historyRecallToolWireSpec.name === HISTORY_RECALL_TOOL_NAME;

const decodeParams = S.decodeUnknownExit(HistoryRecallHostToolParams);

/**
 * Build the HistoryRecall request from host-tool params. Always uses Scope
 * corpus input — the host never accepts an inline corpus from the model
 * (that would let a model smuggle unfiltered history through the tool).
 */
export const historyRecallRequestFromHostParams = (
  params: HistoryRecallHostToolParams,
): HistoryRecallRequest => ({
  corpus: { _tag: "Scope", scope: params.scope },
  question: params.question,
  ...(params.caps === undefined ? {} : { caps: params.caps }),
});

/** Toolkit handlers that resolve through a {@link HistoryRecallShape}. */
export const historyRecallToolkitHandlers = (recall: HistoryRecallShape) =>
  HistoryRecallToolkit.of({
    history_recall: (params) =>
      // HistoryRecallError is caught by resolveHostToolCall and becomes isError.
      recall.recall(historyRecallRequestFromHostParams(params)) as Effect.Effect<
        HistoryRecallResponseType
      >,
  });

/** Layer of handlers over a HistoryRecallShape. */
export const historyRecallToolkitLayer = (recall: HistoryRecallShape) =>
  HistoryRecallToolkit.toLayer(historyRecallToolkitHandlers(recall));

/**
 * Resolve a host-tool call through the HistoryRecall Toolkit. Unknown tools,
 * parameter failures, and typed recall errors all fold into `isError` results
 * — never defects — so the harness can always submit a result to the runtime.
 */
export const resolveHistoryRecallHostToolCall = (options: {
  readonly recall: HistoryRecallShape;
  readonly call: HarnessHostToolCall;
}): Effect.Effect<HarnessHostToolResult> => {
  if (options.call.toolName !== HISTORY_RECALL_TOOL_NAME) {
    return Effect.succeed({
      toolCallId: options.call.toolCallId,
      output: {
        error: "unknown_host_tool",
        detail: `Tool "${options.call.toolName}" is not history_recall.`,
      },
      isError: true,
    });
  }
  return resolveHostToolCall({
    toolkit: HistoryRecallToolkit,
    call: options.call,
  }).pipe(Effect.provide(historyRecallToolkitLayer(options.recall)));
};

/**
 * Same as {@link resolveHistoryRecallHostToolCall} but requires the
 * `HistoryRecall` service in the environment.
 */
export const resolveHistoryRecallHostToolCallFromService = (
  call: HarnessHostToolCall,
): Effect.Effect<HarnessHostToolResult, never, HistoryRecall> =>
  Effect.gen(function* () {
    const recall = yield* HistoryRecall;
    return yield* resolveHistoryRecallHostToolCall({ recall, call });
  });

// ---------------------------------------------------------------------------
// Neutral stream re-entry (audit §5.3)
// ---------------------------------------------------------------------------

export interface HistoryRecallStreamReentryContext {
  readonly turnId: string;
  readonly threadId: string;
  readonly source: KhalaRuntimeSource;
  /** Sequence for the tool.call event. tool.result uses sequence + 1. */
  readonly sequence: number;
  readonly toolCallId: string;
  readonly observedAt?: string;
}

/**
 * Emit the neutral `tool.call` + `tool.result` (or `tool.error`) pair for one
 * host-tool resolution. The result payload does NOT enter the neutral stream —
 * only a `resultRef`. Callers store the bounded answer under that ref for the
 * renderer and for `submitToolResult`.
 */
export const historyRecallNeutralStreamEvents = (options: {
  readonly ctx: HistoryRecallStreamReentryContext;
  readonly result: HarnessHostToolResult;
}): ReadonlyArray<HarnessStreamEvent> => {
  const { ctx, result } = options;
  const base = {
    turnId: ctx.turnId,
    threadId: ctx.threadId,
    source: ctx.source,
    toolCallId: ctx.toolCallId,
    toolName: HISTORY_RECALL_TOOL_NAME,
    ...(ctx.observedAt === undefined ? {} : { observedAt: ctx.observedAt }),
  };
  const callEvent = buildToolCall({
    ...base,
    sequence: ctx.sequence,
    inputRef: `input.host_tool.${ctx.toolCallId}`,
  });
  if (result.isError === true) {
    const messageSafe =
      typeof result.output === "object" &&
      result.output !== null &&
      "error" in result.output &&
      typeof (result.output as { error: unknown }).error === "string"
        ? String((result.output as { error: string }).error).slice(0, 200)
        : "history_recall_failed";
    return [
      callEvent,
      buildToolError({
        ...base,
        sequence: ctx.sequence + 1,
        errorRef: `error.host_tool.${ctx.toolCallId}`,
        messageSafe,
        providerExecuted: false,
      }),
    ];
  }
  return [
    callEvent,
    buildToolResult({
      ...base,
      sequence: ctx.sequence + 1,
      resultRef: `result.host_tool.${ctx.toolCallId}`,
      providerExecuted: false,
    }),
  ];
};

/**
 * Full host-tool dispatch: resolve through HistoryRecall, then build the
 * neutral stream re-entry pair. The caller appends `neutralEvents` to the
 * durable log and submits `result` back to the runtime.
 */
export const dispatchHistoryRecallHostTool = (options: {
  readonly recall: HistoryRecallShape;
  readonly call: HarnessHostToolCall;
  readonly stream: HistoryRecallStreamReentryContext;
}): Effect.Effect<{
  readonly result: HarnessHostToolResult;
  readonly neutralEvents: ReadonlyArray<HarnessStreamEvent>;
  /** Bounded answer when successful — for renderer cited-span rows. */
  readonly answer: HistoryRecallResponseType | null;
}> =>
  Effect.gen(function* () {
    const result = yield* resolveHistoryRecallHostToolCall({
      recall: options.recall,
      call: options.call,
    });
    const neutralEvents = historyRecallNeutralStreamEvents({
      ctx: { ...options.stream, toolCallId: options.call.toolCallId },
      result,
    });
    let answer: HistoryRecallResponseType | null = null;
    if (result.isError !== true) {
      const decoded = S.decodeUnknownExit(HistoryRecallResponse)(result.output);
      if (decoded._tag === "Success") answer = decoded.value;
    }
    return { result, neutralEvents, answer };
  });

/**
 * Decode untrusted host-tool input into params. Fail-closed: invalid input
 * returns null so the dispatcher can emit a typed isError result.
 */
export const decodeHistoryRecallHostToolParams = (
  input: unknown,
): HistoryRecallHostToolParams | null => {
  const decoded = decodeParams(input);
  return decoded._tag === "Success" ? decoded.value : null;
};

/**
 * Bounded public-safe summary for renderer / trace notes. Includes span count,
 * honesty truncation, and the first few citation cursors — never raw long
 * excerpts beyond a short preview.
 */
export const summarizeHistoryRecallAnswer = (
  answer: HistoryRecallResponseType,
  options?: { readonly maxCitations?: number; readonly maxExcerptChars?: number },
): string => {
  const maxCitations = options?.maxCitations ?? 5;
  const maxExcerptChars = options?.maxExcerptChars ?? 80;
  const honesty = answer.honesty.truncated
    ? `partial (caps: ${answer.honesty.capsHit.join(", ") || "unknown"})`
    : "complete";
  const scanned = `${answer.honesty.entriesScanned}/${answer.honesty.entriesTotal} entries`;
  const citations = answer.answers.slice(0, maxCitations).map((span) => {
    const excerpt =
      span.excerpt.length <= maxExcerptChars
        ? span.excerpt
        : `${span.excerpt.slice(0, maxExcerptChars)}…`;
    return `${span.turnId}#${span.sequenceStart}-${span.sequenceEnd}: ${excerpt}`;
  });
  const more =
    answer.answers.length > maxCitations
      ? ` (+${answer.answers.length - maxCitations} more)`
      : "";
  return `history_recall · ${answer.answers.length} span(s) · ${honesty} · ${scanned}${
    citations.length > 0 ? ` · ${citations.join(" | ")}` : ""
  }${more}`;
};
