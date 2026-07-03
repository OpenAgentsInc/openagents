export const KHALA_CODE_OVERLAY_MENU_HOLD_MS = 250

export type OverlayMenuEntry = {
  readonly active?: boolean
  readonly id: string
  readonly key: string
  readonly label: string
}

export type OverlayMenuOptions = {
  readonly emptyLabel?: string
  readonly entries: () => readonly OverlayMenuEntry[]
  readonly hint?: string
  readonly holdDelayMs?: number
  readonly onSelect: (id: string) => void
  readonly ownerDocument?: Document
  readonly title: string
}

export type OverlayMenuHandle = {
  readonly destroy: () => void
  readonly hide: () => void
  readonly isVisible: () => boolean
  readonly notifyHoldKeyDown: () => void
  readonly notifyHoldKeyUp: () => void
  readonly refresh: () => void
  readonly show: () => void
}

/**
 * Generalized centered overlay menu with keyboard-hold reveal semantics.
 * Not mounted for recent-chat hotkeys (contract
 * khala_code.chat.recent_thread_cmd_hotkeys.v2 renders hints in the sidebar
 * rows instead); kept as the shared primitive for future dialog menus.
 */
export const mountOverlayMenu = (
  options: OverlayMenuOptions,
): OverlayMenuHandle => {
  const ownerDocument = options.ownerDocument ?? document
  const holdDelayMs = options.holdDelayMs ?? KHALA_CODE_OVERLAY_MENU_HOLD_MS
  let holdTimer: ReturnType<typeof setTimeout> | null = null
  let visible = false

  const root = ownerDocument.createElement("div")
  root.className = "khala-overlay-menu"
  root.hidden = true
  root.setAttribute("role", "menu")
  root.setAttribute("aria-label", options.title)
  ownerDocument.body.append(root)

  const cancelHoldTimer = (): void => {
    if (holdTimer === null) return
    clearTimeout(holdTimer)
    holdTimer = null
  }

  const render = (): void => {
    const entries = options.entries()
    const heading = ownerDocument.createElement("p")
    heading.className = "khala-overlay-menu-title"
    heading.textContent = options.title

    const list = ownerDocument.createElement("div")
    list.className = "khala-overlay-menu-list"
    if (entries.length === 0) {
      const empty = ownerDocument.createElement("p")
      empty.className = "khala-overlay-menu-empty"
      empty.textContent = options.emptyLabel ?? "Nothing here"
      list.append(empty)
    }
    for (const entry of entries) {
      const row = ownerDocument.createElement("button")
      row.type = "button"
      row.className = "khala-overlay-menu-item"
      row.dataset.entryId = entry.id
      row.dataset.entryKey = entry.key
      row.dataset.active = entry.active === true ? "true" : "false"
      row.setAttribute("role", "menuitem")

      const key = ownerDocument.createElement("span")
      key.className = "khala-overlay-menu-key"
      key.textContent = entry.key

      const label = ownerDocument.createElement("span")
      label.className = "khala-overlay-menu-item-label"
      label.textContent = entry.label

      row.append(key, label)
      row.addEventListener("click", () => {
        hide()
        options.onSelect(entry.id)
      })
      list.append(row)
    }

    root.replaceChildren(heading, list)
    if (options.hint !== undefined) {
      const hint = ownerDocument.createElement("p")
      hint.className = "khala-overlay-menu-hint"
      hint.textContent = options.hint
      root.append(hint)
    }
  }

  const show = (): void => {
    cancelHoldTimer()
    render()
    visible = true
    root.hidden = false
  }

  const hide = (): void => {
    cancelHoldTimer()
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
    notifyHoldKeyDown: () => {
      if (visible || holdTimer !== null) return
      if (holdDelayMs <= 0) {
        show()
        return
      }
      holdTimer = setTimeout(show, holdDelayMs)
    },
    notifyHoldKeyUp: () => {
      hide()
    },
    refresh: () => {
      if (visible) render()
    },
    show,
  }
}
