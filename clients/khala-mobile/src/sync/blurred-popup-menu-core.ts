import type { TranscriptPart } from "./khala-runtime-transcript-core"

/** Pure builders for the long-press "Blurred Popup" context menu's copy
 * actions (`app/thread/[threadId].tsx`, wired via `TouchablePopupHandler`,
 * see issue #8395). Both return `undefined` for part kinds with nothing
 * meaningful to copy (`usage`, `turn-status`) — those rows aren't wrapped in
 * `TouchablePopupHandler` at all, but these stay total functions over
 * `TranscriptPart["kind"]` so callers don't have to duplicate the check. */

/** Plain-text clipboard payload — no markdown decoration, just the part's
 * own readable content. */
export const buildCopyText = (part: TranscriptPart): string | undefined => {
  switch (part.kind) {
    case "text":
      return part.text
    case "reasoning":
      return part.text
    case "tool":
      return part.errorMessageSafe === undefined
        ? `${part.toolName} (${part.status})`
        : `${part.toolName} (${part.status}): ${part.errorMessageSafe}`
    case "usage":
    case "turn-status":
      return undefined
  }
}

/** Markdown-flavored clipboard payload — reasoning gets its blockquote
 * treatment and tool calls get an inline-code tool name, matching the same
 * markdown conventions `buildQuoteSnippet` (`./swipe-quote-core.ts`) already
 * established for the swipe-to-quote action, so "Copy as Markdown" and
 * "Quote" read consistently if a user does both. */
export const buildCopyMarkdown = (part: TranscriptPart): string | undefined => {
  switch (part.kind) {
    case "text":
      return part.text
    case "reasoning":
      return `> ${part.text}`
    case "tool":
      return part.errorMessageSafe === undefined
        ? `\`${part.toolName}\` — ${part.status}`
        : `\`${part.toolName}\` — ${part.status}: ${part.errorMessageSafe}`
    case "usage":
    case "turn-status":
      return undefined
  }
}
