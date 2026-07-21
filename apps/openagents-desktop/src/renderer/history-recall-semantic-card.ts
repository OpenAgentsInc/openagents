/**
 * Renderer projection for Tier S (semantic) `history_recall` results (RLM-05).
 *
 * Renderer boundary: this file must not import main-process packages. The
 * bounded terminal summary shape is duplicated from
 * `../history-recall-semantic.ts` (`SemanticRecallTerminalSummary`) as plain
 * data, the same pattern as the Tier D card.
 *
 * Presentation rules (issue #9141):
 * - completed, partial, refused, and typed failure states render distinctly;
 * - recall output is labelled a CITED CANDIDATE — never "verified";
 * - usage honesty is explicit: exact token totals when known, otherwise the
 *   words "tokens unavailable" — never a fabricated zero;
 * - only validated citations render as navigable spans.
 */

/** Wire name — must match `HISTORY_RECALL_TOOL_NAME` in the harness contract. */
export const HISTORY_RECALL_TOOL_NAME = "history_recall" as const

/** Cited span row shape (mirrors main-process projection, no package import). */
export interface HistoryRecallCitedSpanRow {
  readonly turnId: string
  readonly sequenceStart: number
  readonly sequenceEnd: number
  readonly excerpt: string
  readonly kind: string
  readonly scopeRef: string
}

/** Plain-data mirror of the main-process bounded terminal summary. */
export interface SemanticRecallTerminalSummary {
  readonly state: "completed" | "partial" | "refused"
  readonly reason: string | null
  readonly modelCalls: number
  readonly subcalls: number
  readonly totalTokens: number | null
  readonly usageCompleteness: "complete" | "partial" | "unavailable"
  readonly citationValidated: number
  readonly citationInvalid: number
  readonly strategyRef: string | null
}

export type SemanticRecallCardState =
  | "running"
  | "completed"
  | "partial"
  | "refused"
  | "failed"

export interface SemanticRecallToolCardModel {
  readonly key: string
  readonly toolCallId: string
  readonly toolName: typeof HISTORY_RECALL_TOOL_NAME
  readonly tier: "semantic"
  readonly state: SemanticRecallCardState
  /** Bounded headline: state + candidate labelling. */
  readonly headline: string
  /** Exact-usage honesty line. */
  readonly usageLine: string
  readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly citedSpansLine: string
}

const MAX_PREVIEW_SPANS = 4
const MAX_EXCERPT = 72

const formatSpans = (
  spans: ReadonlyArray<HistoryRecallCitedSpanRow>,
): string => {
  if (spans.length === 0) return "no citations"
  const parts = spans.slice(0, MAX_PREVIEW_SPANS).map((span) => {
    const excerpt =
      span.excerpt.length <= MAX_EXCERPT
        ? span.excerpt
        : `${span.excerpt.slice(0, MAX_EXCERPT)}…`
    return `${span.turnId}#${span.sequenceStart}-${span.sequenceEnd}: ${excerpt}`
  })
  const more =
    spans.length > MAX_PREVIEW_SPANS
      ? ` (+${spans.length - MAX_PREVIEW_SPANS} more)`
      : ""
  return `${parts.join(" · ")}${more}`
}

/** Exact-usage honesty line. Unknown token totals stay "unavailable". */
export const formatSemanticRecallUsageLine = (
  summary: SemanticRecallTerminalSummary,
): string => {
  const tokens =
    summary.totalTokens === null
      ? "tokens unavailable"
      : `${summary.totalTokens} tokens (${summary.usageCompleteness})`
  return `${summary.modelCalls} model calls · ${summary.subcalls} subcalls · ${tokens}`
}

const headlineFor = (
  state: SemanticRecallCardState,
  reason: string | null,
): string => {
  switch (state) {
    case "running":
      return "semantic recall running…"
    case "completed":
      return "semantic recall complete · cited candidate (not verified)"
    case "partial":
      return `semantic recall partial (${reason ?? "capped"}) · cited candidate (not verified)`
    case "refused":
      return `semantic recall refused (${reason ?? "not admitted"})`
    case "failed":
      return `semantic recall failed (${reason ?? "error"})`
  }
}

/**
 * Project a semantic recall invocation into a renderer card model. Replay
 * needs only the bounded terminal summary and cited spans — no transient
 * progress rows are consulted.
 */
export const projectSemanticRecallToolCard = (input: {
  readonly toolCallId: string
  readonly phase: "started" | "terminal" | "failed"
  readonly terminal?: SemanticRecallTerminalSummary
  readonly failureReason?: string
  readonly citedSpans?: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly key?: string
}): SemanticRecallToolCardModel => {
  const citedSpans = input.citedSpans ?? []
  const state: SemanticRecallCardState =
    input.phase === "started"
      ? "running"
      : input.phase === "failed"
        ? "failed"
        : (input.terminal?.state ?? "failed")
  const reason =
    input.phase === "failed"
      ? (input.failureReason ?? null)
      : (input.terminal?.reason ?? null)
  return {
    key: input.key ?? `history-recall-semantic.${input.toolCallId}`,
    toolCallId: input.toolCallId,
    toolName: HISTORY_RECALL_TOOL_NAME,
    tier: "semantic",
    state,
    headline: headlineFor(state, reason),
    usageLine:
      state === "running"
        ? "usage pending"
        : input.terminal === undefined
          ? "usage unavailable"
          : formatSemanticRecallUsageLine(input.terminal),
    citedSpans: state === "completed" || state === "partial" ? citedSpans : [],
    citedSpansLine:
      state === "completed" || state === "partial"
        ? formatSpans(citedSpans)
        : state === "running"
          ? "recalling…"
          : state === "refused"
            ? "semantic recall refused"
            : "recall failed",
  }
}
