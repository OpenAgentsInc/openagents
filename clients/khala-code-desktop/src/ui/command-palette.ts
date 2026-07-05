import type {
  KhalaCodeCommandPaletteRecord,
  KhalaCodeCommandPaletteResult,
  KhalaCodeCommandRegistry,
} from "./command-registry"

export type KhalaCodeCommandPaletteHandle = Readonly<{
  close: () => void
  destroy: () => void
  isOpen: () => boolean
  open: (query?: string) => void
  selectedResultId: () => string | null
  setQuery: (query: string) => void
}>

export type KhalaCodeCommandPaletteOptions = Readonly<{
  getLoading?: () => boolean
  getRecords?: () => readonly KhalaCodeCommandPaletteRecord[]
  onClose?: () => void
  onExecute?: (result: KhalaCodeCommandPaletteResult) => void | Promise<void>
  registry: KhalaCodeCommandRegistry
}>

const groupLabel = (group: string): string => {
  switch (group) {
    case "composer":
      return "Composer"
    case "file":
      return "Files"
    case "navigation":
      return "Navigation"
    case "model":
      return "Models"
    case "project":
      return "Projects"
    case "provider":
      return "Providers"
    case "server":
      return "Server"
    case "session":
      return "Session"
    case "settings":
      return "Settings"
    case "workbench":
      return "Workbench"
    default:
      return group
  }
}

export const mountKhalaCodeCommandPalette = (
  container: HTMLElement,
  options: KhalaCodeCommandPaletteOptions,
): KhalaCodeCommandPaletteHandle => {
  let open = false
  let query = ""
  let selectedIndex = 0
  let results: readonly KhalaCodeCommandPaletteResult[] = []

  const root = document.createElement("div")
  root.className = "khala-code-command-palette"
  root.dataset.khalaCodeCommandPalette = ""
  root.hidden = true
  root.setAttribute("role", "dialog")
  root.setAttribute("aria-modal", "true")
  root.setAttribute("aria-label", "Command palette")

  const panel = document.createElement("div")
  panel.className = "khala-code-command-palette-panel"

  const input = document.createElement("input")
  input.type = "search"
  input.className = "khala-code-command-palette-input"
  input.placeholder = "Run command"
  input.autocomplete = "off"
  input.spellcheck = false
  input.setAttribute("aria-label", "Search commands")
  input.setAttribute("role", "combobox")
  input.setAttribute("aria-expanded", "true")

  const list = document.createElement("div")
  list.className = "khala-code-command-palette-list"
  list.setAttribute("role", "listbox")

  panel.append(input, list)
  root.append(panel)
  container.replaceChildren(root)

  const close = (): void => {
    if (!open) return
    open = false
    root.hidden = true
    options.onClose?.()
  }

  const executeSelected = (): void => {
    const result = results[selectedIndex]
    if (result === undefined || result.disabled === true) return
    close()
    void options.onExecute?.(result)
  }

  const resultButton = (
    result: KhalaCodeCommandPaletteResult,
    index: number,
  ): HTMLButtonElement => {
    const button = document.createElement("button")
    button.type = "button"
    button.className = "khala-code-command-palette-result"
    button.dataset.commandPaletteResult = result.id
    button.dataset.kind = result.kind
    button.dataset.selected = index === selectedIndex ? "true" : "false"
    button.disabled = result.disabled === true
    button.setAttribute("role", "option")
    button.setAttribute("aria-selected", index === selectedIndex ? "true" : "false")
    if (result.disabled === true) button.setAttribute("aria-disabled", "true")
    button.addEventListener("mouseenter", () => {
      selectedIndex = index
      render()
    })
    button.addEventListener("click", executeSelected)

    const title = document.createElement("span")
    title.className = "khala-code-command-palette-result-title"
    title.textContent = result.title

    const meta = document.createElement("span")
    meta.className = "khala-code-command-palette-result-meta"
    meta.textContent =
      result.disabled === true
        ? result.disabledReason ?? "Unavailable"
        : result.subtitle ?? result.kind

    const shortcut = document.createElement("span")
    shortcut.className = "khala-code-command-palette-result-shortcut"
    shortcut.textContent = result.keybindingLabel ?? ""
    shortcut.hidden = result.keybindingLabel === undefined

    button.replaceChildren(title, meta, shortcut)
    return button
  }

  const emptyState = (message: string): HTMLElement => {
    const empty = document.createElement("div")
    empty.className = "khala-code-command-palette-empty"
    empty.setAttribute("role", "status")
    empty.textContent = message
    return empty
  }

  const render = (): void => {
    root.hidden = !open
    if (!open) return
    const loading = options.getLoading?.() ?? false
    results = loading
      ? []
      : options.registry.search({
          includeDisabled: true,
          limit: 18,
          query,
          records: options.getRecords?.() ?? [],
        })
    selectedIndex = Math.max(0, Math.min(selectedIndex, Math.max(0, results.length - 1)))
    input.value = query
    input.setAttribute("aria-activedescendant", results[selectedIndex]?.id ?? "")
    if (loading) {
      list.replaceChildren(emptyState("Loading commands"))
      return
    }
    if (results.length === 0) {
      list.replaceChildren(emptyState(query.trim() === "" ? "No commands" : "No matching commands"))
      return
    }

    const groups = new Map<string, KhalaCodeCommandPaletteResult[]>()
    for (const result of results) {
      groups.set(result.group, [...(groups.get(result.group) ?? []), result])
    }
    const nodes: HTMLElement[] = []
    for (const [group, groupResults] of groups) {
      const heading = document.createElement("div")
      heading.className = "khala-code-command-palette-group"
      heading.textContent = groupLabel(group)
      nodes.push(heading)
      for (const result of groupResults) {
        nodes.push(resultButton(result, results.indexOf(result)))
      }
    }
    list.replaceChildren(...nodes)
  }

  input.addEventListener("input", () => {
    query = input.value
    selectedIndex = 0
    render()
  })

  input.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      event.preventDefault()
      close()
      return
    }
    if (event.key === "ArrowDown") {
      event.preventDefault()
      selectedIndex = Math.min(results.length - 1, selectedIndex + 1)
      render()
      return
    }
    if (event.key === "ArrowUp") {
      event.preventDefault()
      selectedIndex = Math.max(0, selectedIndex - 1)
      render()
      return
    }
    if (event.key === "Enter") {
      event.preventDefault()
      executeSelected()
    }
  })

  root.addEventListener("click", event => {
    if (event.target === root) close()
  })

  return {
    close,
    destroy: () => {
      root.remove()
    },
    isOpen: () => open,
    open: initialQuery => {
      open = true
      query = initialQuery ?? ""
      selectedIndex = 0
      render()
      requestAnimationFrame(() => input.focus({ preventScroll: true }))
    },
    selectedResultId: () => results[selectedIndex]?.id ?? null,
    setQuery: value => {
      query = value
      selectedIndex = 0
      render()
    },
  }
}
