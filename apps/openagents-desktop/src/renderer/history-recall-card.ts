/**
 * Renderer projection for `history_recall` tool rows (RLM-03).
 *
 * Cited spans are already corpus-filtered (visibility / redactionClass).
 * This module only formats them for the transcript tool card — it does not
 * dereference result refs or re-read raw history.
 *
 * Renderer boundary: this file must not import main-process packages
 * (`@openagentsinc/agent-harness-contract`, `@openagentsinc/history-corpus`).
 * The wire tool name is duplicated as a string constant so the projection
 * stays renderer-safe; main owns the registry authority.
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

export type HistoryRecallCardStatus = "running" | "ok" | "failed"

export interface HistoryRecallToolCardModel {
  readonly key: string
  readonly toolCallId: string
  readonly toolName: typeof HISTORY_RECALL_TOOL_NAME
  readonly status: HistoryRecallCardStatus
  /** Bounded summary line (honesty + span count). */
  readonly summary: string
  /** Cited spans for the expanded row — empty while running or on failure. */
  readonly citedSpans: ReadonlyArray<HistoryRecallCitedSpanRow>
  /** Single-line citation preview for the default (collapsed) view. */
  readonly citedSpansLine: string
}

const MAX_PREVIEW_SPANS = 4
const MAX_EXCERPT = 72

/** Format cited spans as a single public-safe preview line. */
export const formatHistoryRecallCitedSpans = (
  spans: ReadonlyArray<HistoryRecallCitedSpanRow>,
  options?: { readonly maxSpans?: number; readonly maxExcerptChars?: number },
): string => {
  const maxSpans = options?.maxSpans ?? MAX_PREVIEW_SPANS
  const maxExcerpt = options?.maxExcerptChars ?? MAX_EXCERPT
  if (spans.length === 0) return "no citations"
  const parts = spans.slice(0, maxSpans).map((span) => {
    const excerpt =
      span.excerpt.length <= maxExcerpt
        ? span.excerpt
        : `${span.excerpt.slice(0, maxExcerpt)}…`
    return `${span.turnId}#${span.sequenceStart}-${span.sequenceEnd}: ${excerpt}`
  })
  const more =
    spans.length > maxSpans ? ` (+${spans.length - maxSpans} more)` : ""
  return `${parts.join(" · ")}${more}`
}

/** Whether a tool-card / trace toolName is history_recall. */
export const isHistoryRecallToolCard = (toolName: string): boolean =>
  toolName === HISTORY_RECALL_TOOL_NAME

/**
 * Project a completed (or running) history_recall invocation into a
 * renderer card model. `citedSpans` should come from the main-process
 * dispatch result (already redacted).
 */
export const projectHistoryRecallToolCard = (input: {
  readonly toolCallId: string
  readonly phase: "started" | "ok" | "failed"
  readonly summary: string
  readonly citedSpans?: ReadonlyArray<HistoryRecallCitedSpanRow>
  readonly key?: string
}): HistoryRecallToolCardModel => {
  const citedSpans = input.citedSpans ?? []
  const status: HistoryRecallCardStatus =
    input.phase === "started" ? "running" : input.phase === "ok" ? "ok" : "failed"
  return {
    key: input.key ?? `history-recall.${input.toolCallId}`,
    toolCallId: input.toolCallId,
    toolName: HISTORY_RECALL_TOOL_NAME,
    status,
    summary: input.summary.slice(0, 400),
    citedSpans,
    citedSpansLine:
      status === "ok" ? formatHistoryRecallCitedSpans(citedSpans) : status === "running"
        ? "recalling…"
        : "recall failed",
  }
}
