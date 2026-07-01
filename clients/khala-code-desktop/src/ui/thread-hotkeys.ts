import type { KhalaCodeDesktopCodexThreadSummary } from "../shared/codex-threads"

export const KHALA_CODE_RECENT_THREAD_HOTKEY_LIMIT = 10

export const recentThreadIndexForDigitKey = (key: string): number | null => {
  if (/^[1-9]$/u.test(key)) return Number(key) - 1
  if (key === "0") return 9
  return null
}

const threadRecencyValue = (
  thread: KhalaCodeDesktopCodexThreadSummary,
): number =>
  thread.recencyAt ??
  thread.updatedAt ??
  thread.createdAt ??
  0

export const recentThreadsForHotkeys = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): readonly KhalaCodeDesktopCodexThreadSummary[] =>
  [...threads]
    .sort((left, right) => {
      const recencyDelta = threadRecencyValue(right) - threadRecencyValue(left)
      if (recencyDelta !== 0) return recencyDelta
      const titleDelta = left.title.localeCompare(right.title)
      if (titleDelta !== 0) return titleDelta
      return left.id.localeCompare(right.id)
    })
    .slice(0, KHALA_CODE_RECENT_THREAD_HOTKEY_LIMIT)
