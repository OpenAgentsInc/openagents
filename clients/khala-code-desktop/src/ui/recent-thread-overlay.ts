import type { KhalaCodeDesktopCodexThreadSummary } from "../shared/codex-threads"
import { recentThreadsForHotkeys } from "./thread-hotkeys"

export const KHALA_CODE_RECENT_THREAD_OVERLAY_LIMIT = 9
export const KHALA_CODE_RECENT_THREAD_OVERLAY_HOLD_MS = 250

export type RecentThreadOverlayEntry = {
  readonly digit: number
  readonly threadId: string
  readonly title: string
}

export const recentThreadOverlayEntries = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): readonly RecentThreadOverlayEntry[] =>
  recentThreadsForHotkeys(threads)
    .slice(0, KHALA_CODE_RECENT_THREAD_OVERLAY_LIMIT)
    .map((thread, index) => ({
      digit: index + 1,
      threadId: thread.id,
      title: thread.title,
    }))

export type RecentThreadOverlayOptions = {
  readonly activeThreadId: () => string | null
  readonly holdDelayMs?: number
  readonly ownerDocument?: Document
  readonly recentThreads: () => readonly KhalaCodeDesktopCodexThreadSummary[]
  readonly onSelect: (index: number) => void
}

export type RecentThreadOverlayHandle = {
  readonly destroy: () => void
  readonly hide: () => void
  readonly isVisible: () => boolean
  readonly notifyMetaKeyDown: () => void
  readonly notifyMetaKeyUp: () => void
  readonly refresh: () => void
  readonly show: () => void
}

export const mountRecentThreadOverlay = (
  options: RecentThreadOverlayOptions,
): RecentThreadOverlayHandle => {
  const ownerDocument = options.ownerDocument ?? document
  const holdDelayMs = options.holdDelayMs ?? KHALA_CODE_RECENT_THREAD_OVERLAY_HOLD_MS
  let holdTimer: ReturnType<typeof setTimeout> | null = null
  let visible = false

  const root = ownerDocument.createElement("div")
  root.className = "khala-recent-thread-overlay"
  root.hidden = true
  root.setAttribute("role", "menu")
  root.setAttribute("aria-label", "Recent chats")
  ownerDocument.body.append(root)

  const cancelHoldTimer = (): void => {
    if (holdTimer === null) return
    clearTimeout(holdTimer)
    holdTimer = null
  }

  const render = (): void => {
    const entries = recentThreadOverlayEntries(options.recentThreads())
    const activeThreadId = options.activeThreadId()
    const heading = ownerDocument.createElement("p")
    heading.className = "khala-recent-thread-overlay-title"
    heading.textContent = "Recent chats"

    const list = ownerDocument.createElement("div")
    list.className = "khala-recent-thread-overlay-list"
    if (entries.length === 0) {
      const empty = ownerDocument.createElement("p")
      empty.className = "khala-recent-thread-overlay-empty"
      empty.textContent = "No recent chats"
      list.append(empty)
    }
    for (const entry of entries) {
      const row = ownerDocument.createElement("button")
      row.type = "button"
      row.className = "khala-recent-thread-overlay-item"
      row.dataset.threadId = entry.threadId
      row.dataset.digit = String(entry.digit)
      row.dataset.active = activeThreadId === entry.threadId ? "true" : "false"
      row.setAttribute("role", "menuitem")

      const digit = ownerDocument.createElement("span")
      digit.className = "khala-recent-thread-overlay-digit"
      digit.textContent = String(entry.digit)

      const title = ownerDocument.createElement("span")
      title.className = "khala-recent-thread-overlay-item-title"
      title.textContent = entry.title

      row.append(digit, title)
      row.addEventListener("click", () => {
        hide()
        options.onSelect(entry.digit - 1)
      })
      list.append(row)
    }

    const hint = ownerDocument.createElement("p")
    hint.className = "khala-recent-thread-overlay-hint"
    hint.textContent = "⌘ held — press 1–9 to jump"

    root.replaceChildren(heading, list, hint)
  }

  const show = (): void => {
    cancelHoldTimer()
    render()
    visible = true
    root.hidden = false
  }

  const hide = (): void => {
    cancelHoldTimer()
    if (!visible) {
      root.hidden = true
      return
    }
    visible = false
    root.hidden = true
  }

  return {
    destroy: () => {
      cancelHoldTimer()
      root.remove()
    },
    hide,
    isVisible: () => visible,
    notifyMetaKeyDown: () => {
      if (visible || holdTimer !== null) return
      if (holdDelayMs <= 0) {
        show()
        return
      }
      holdTimer = setTimeout(show, holdDelayMs)
    },
    notifyMetaKeyUp: () => {
      hide()
    },
    refresh: () => {
      if (visible) render()
    },
    show,
  }
}
