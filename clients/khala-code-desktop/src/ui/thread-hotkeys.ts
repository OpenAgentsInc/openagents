import type { KhalaCodeDesktopCodexThreadSummary } from "../shared/codex-threads"

export const KHALA_CODE_RECENT_THREAD_HOTKEY_LIMIT = 10

export type RecentThreadCycleDirection = "newer" | "older"

export type RecentThreadHotkeyEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "defaultPrevented" | "key" | "metaKey" | "shiftKey"
>

export const recentThreadIndexForDigitKey = (key: string): number | null => {
  if (/^[1-9]$/u.test(key)) return Number(key) - 1
  if (key === "0") return 9
  return null
}

export const recentThreadHotkeyIndexForEvent = (
  event: RecentThreadHotkeyEvent,
): number | null => {
  if (
    event.defaultPrevented ||
    event.altKey ||
    event.ctrlKey ||
    !event.metaKey ||
    event.shiftKey
  ) {
    return null
  }
  return recentThreadIndexForDigitKey(event.key)
}

export const recentThreadCycleDirectionForEvent = (
  event: RecentThreadHotkeyEvent,
): RecentThreadCycleDirection | null => {
  if (
    event.defaultPrevented ||
    event.altKey ||
    event.ctrlKey ||
    !event.metaKey ||
    event.shiftKey
  ) {
    return null
  }
  if (event.key === "ArrowUp") return "newer"
  if (event.key === "ArrowDown") return "older"
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
    .filter(thread => thread.resumable !== false)
    .sort((left, right) => {
      const recencyDelta = threadRecencyValue(right) - threadRecencyValue(left)
      if (recencyDelta !== 0) return recencyDelta
      const titleDelta = left.title.localeCompare(right.title)
      if (titleDelta !== 0) return titleDelta
      return left.id.localeCompare(right.id)
    })
    .slice(0, KHALA_CODE_RECENT_THREAD_HOTKEY_LIMIT)

export const recentThreadCycleIndex = (
  input: {
    readonly activeThreadId: string | null
    readonly direction: RecentThreadCycleDirection
    readonly threads: readonly KhalaCodeDesktopCodexThreadSummary[]
  },
): number | null => {
  const threads = recentThreadsForHotkeys(input.threads)
  if (threads.length === 0) return null
  const activeIndex = input.activeThreadId === null
    ? -1
    : threads.findIndex(thread => thread.id === input.activeThreadId)
  if (activeIndex < 0) return 0
  const delta = input.direction === "newer" ? -1 : 1
  return (activeIndex + delta + threads.length) % threads.length
}

export const KHALA_CODE_RECENT_THREAD_HOTKEY_HINT_HOLD_MS = 250
export const KHALA_CODE_RECENT_THREAD_HOTKEY_HINT_LIMIT = 9

export type KeyHoldTracker = {
  readonly isRevealed: () => boolean
  readonly keyDown: () => void
  readonly keyUp: () => void
}

export const createKeyHoldTracker = (input: {
  readonly holdDelayMs?: number
  readonly onHide: () => void
  readonly onReveal: () => void
}): KeyHoldTracker => {
  const holdDelayMs = input.holdDelayMs ?? KHALA_CODE_RECENT_THREAD_HOTKEY_HINT_HOLD_MS
  let holdTimer: ReturnType<typeof setTimeout> | null = null
  let revealed = false

  const reveal = (): void => {
    holdTimer = null
    revealed = true
    input.onReveal()
  }

  return {
    isRevealed: () => revealed,
    keyDown: () => {
      if (revealed || holdTimer !== null) return
      if (holdDelayMs <= 0) {
        reveal()
        return
      }
      holdTimer = setTimeout(reveal, holdDelayMs)
    },
    keyUp: () => {
      if (holdTimer !== null) {
        clearTimeout(holdTimer)
        holdTimer = null
      }
      if (!revealed) return
      revealed = false
      input.onHide()
    },
  }
}

export const recentThreadHotkeyHintDigits = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): ReadonlyMap<string, number> =>
  new Map(
    recentThreadsForHotkeys(threads)
      .slice(0, KHALA_CODE_RECENT_THREAD_HOTKEY_HINT_LIMIT)
      .map((thread, index) => [thread.id, index + 1]),
  )
