import type { TranscriptPart } from "./khala-runtime-transcript-core"

const QUOTE_SNIPPET_MAX_LENGTH = 80

const truncate = (text: string, maxLength: number): string => {
  const collapsed = text.replace(/\s+/g, " ").trim()
  if (collapsed.length <= maxLength) return collapsed
  return `${collapsed.slice(0, maxLength - 1).trimEnd()}…`
}

/** Builds a short quoted-reference snippet for the swipe-to-quote action in
 * the thread transcript list (`app/thread/[threadId].tsx`, wired via
 * `SwipeableItem`, see issue #8393). Returns `undefined` for part kinds with
 * no meaningful quotable content (`usage`, `turn-status`) — those rows aren't
 * wrapped in `SwipeableItem` at all, but this stays a total function over
 * `TranscriptPart["kind"]` so callers don't have to duplicate the kind
 * check. */
export const buildQuoteSnippet = (
  part: TranscriptPart,
  maxLength: number = QUOTE_SNIPPET_MAX_LENGTH
): string | undefined => {
  switch (part.kind) {
    case "text":
      return `> ${truncate(part.text, maxLength)}`
    case "reasoning":
      return `> (reasoning) ${truncate(part.text, maxLength)}`
    case "tool":
      return `> re: ${part.toolName} (${part.status})`
    case "usage":
    case "turn-status":
      return undefined
  }
}

/** Merges a quote snippet into the composer's current draft text. If the
 * draft is empty, the snippet becomes the start of the message with a
 * trailing space so the user can keep typing immediately; otherwise the
 * snippet is prepended above the existing draft on its own line, preserving
 * whatever the user had already typed rather than clobbering it. */
export const buildComposerTextWithQuote = (currentText: string, quoteSnippet: string): string => {
  if (currentText.trim().length === 0) return `${quoteSnippet} `
  return `${quoteSnippet}\n${currentText}`
}
