/**
 * Pure H2 history -> local-thread seed projection (#8712).
 *
 * Main re-reads a bounded provider-history page and calls this projector. The
 * renderer supplies refs/cutoff only. The resulting seed deliberately matches
 * fable-local-runtime's existing first-turn history seam: at most 12 user /
 * assistant messages, each at most 2,000 characters.
 */
import type { DesktopMessage } from "./chat-contract.ts"
import type { CodexHistoryItem } from "./codex-history-contract.ts"
import {
  FABLE_LOCAL_HISTORY_MESSAGES,
  FABLE_LOCAL_HISTORY_MESSAGE_LIMIT,
} from "./fable-local-runtime.ts"

export const HISTORY_FORK_READ_LIMIT = 500

export type HistoryForkFetchPlan = Readonly<{
  offset: number
  limit: number
  throughSequence: number
}>

export const historyForkFetchPlan = (
  totalItems: number,
  requestedThroughSequence: number | null,
): HistoryForkFetchPlan | null => {
  if (!Number.isInteger(totalItems) || totalItems <= 0) return null
  const throughSequence = Math.min(
    totalItems - 1,
    requestedThroughSequence === null ? totalItems - 1 : Math.max(0, requestedThroughSequence),
  )
  const limit = Math.min(HISTORY_FORK_READ_LIMIT, throughSequence + 1)
  return { offset: throughSequence + 1 - limit, limit, throughSequence }
}

const historyRole = (item: CodexHistoryItem): DesktopMessage["role"] | null =>
  item.kind === "user_message" ? "user" : item.kind === "assistant_message" ? "assistant" : null

export const historyForkSeed = (
  items: ReadonlyArray<CodexHistoryItem>,
  throughSequence: number,
): ReadonlyArray<DesktopMessage> =>
  items
    .filter(item => item.sequence <= throughSequence && historyRole(item) !== null && item.summary.trim() !== "")
    .slice(-FABLE_LOCAL_HISTORY_MESSAGES)
    .map(item => ({
      key: `fork.${item.itemRef}`.slice(0, 320),
      role: historyRole(item)!,
      text: item.summary.slice(0, FABLE_LOCAL_HISTORY_MESSAGE_LIMIT),
      timestamp: item.timestamp.slice(0, 64),
    }))
