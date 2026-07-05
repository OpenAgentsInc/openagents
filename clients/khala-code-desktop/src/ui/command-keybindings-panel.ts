import {
  formatKhalaCodeCommandKeybindingConfig,
  khalaCodeCommandKeybindingConfigForKeyboardEvent,
  khalaCodeCommandKeybindingSignatures,
  parseKhalaCodeCommandKeybindingConfig,
  type KhalaCodeCommandId,
  type KhalaCodeCommandKeybindingOverrideMap,
  type KhalaCodeCommandRegistry,
} from "./command-registry"

export const KHALA_CODE_COMMAND_KEYBINDINGS_STORAGE_KEY =
  "khala-code-desktop.command-keybindings.v1"

export type KhalaCodeCommandKeybindingsSectionHandle = Readonly<{
  render: () => HTMLElement
  stopCapture: () => void
}>

export type KhalaCodeCommandKeybindingsSectionOptions = Readonly<{
  getOverrides: () => KhalaCodeCommandKeybindingOverrideMap
  onChanged?: () => void
  registry: () => KhalaCodeCommandRegistry | null
  resetAll: () => void
  setOverride: (id: KhalaCodeCommandId, value: string | null) => void
}>

const el = <Tag extends keyof HTMLElementTagNameMap>(
  tag: Tag,
  className?: string,
  text?: string,
): HTMLElementTagNameMap[Tag] => {
  const node = document.createElement(tag)
  if (className !== undefined) node.className = className
  if (text !== undefined) node.textContent = text
  return node
}

export const readKhalaCodeCommandKeybindingOverrides = (
  storage: Storage,
): KhalaCodeCommandKeybindingOverrideMap => {
  const raw = storage.getItem(KHALA_CODE_COMMAND_KEYBINDINGS_STORAGE_KEY)
  if (raw === null) return {}
  try {
    const parsed = JSON.parse(raw) as unknown
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return {}
    return Object.fromEntries(
      Object.entries(parsed as Record<string, unknown>)
        .filter(([, value]) => typeof value === "string")
        .map(([key, value]) => [key, value]),
    ) as KhalaCodeCommandKeybindingOverrideMap
  } catch {
    return {}
  }
}

export const writeKhalaCodeCommandKeybindingOverrides = (
  storage: Storage,
  overrides: KhalaCodeCommandKeybindingOverrideMap,
): void => {
  storage.setItem(
    KHALA_CODE_COMMAND_KEYBINDINGS_STORAGE_KEY,
    JSON.stringify(overrides, null, 2),
  )
}

const hasOwn = (
  overrides: KhalaCodeCommandKeybindingOverrideMap,
  id: KhalaCodeCommandId,
): boolean => Object.prototype.hasOwnProperty.call(overrides, id)

const categoryLabel = (value: string): string => {
  switch (value) {
    case "composer":
      return "Composer"
    case "navigation":
      return "Navigation"
    case "session":
      return "Session"
    case "settings":
      return "Settings"
    case "workbench":
      return "Workbench"
    default:
      return value
  }
}

export const createKhalaCodeCommandKeybindingsSection = (
  options: KhalaCodeCommandKeybindingsSectionOptions,
): KhalaCodeCommandKeybindingsSectionHandle => {
  let activeCommandId: KhalaCodeCommandId | null = null
  let filter = ""
  let conflictIds = new Set<KhalaCodeCommandId>()
  let sectionNode: HTMLElement | null = null
  let statusNode: HTMLElement | null = null
  let listNode: HTMLElement | null = null

  const titleFor = (registry: KhalaCodeCommandRegistry, id: KhalaCodeCommandId): string =>
    registry.command(id)?.title ?? id

  const syncStatus = (message: string): void => {
    if (statusNode === null) return
    statusNode.textContent = message
    statusNode.hidden = message.length === 0
  }

  const matchingCommands = (
    registry: KhalaCodeCommandRegistry,
  ): ReturnType<KhalaCodeCommandRegistry["commands"]> => {
    const query = filter.trim().toLowerCase()
    return registry.commands()
      .filter(command => {
        if (query.length === 0) return true
        const keybinding = registry.keybindingLabel(command.id).toLowerCase()
        return (
          command.title.toLowerCase().includes(query) ||
          command.id.toLowerCase().includes(query) ||
          command.category.toLowerCase().includes(query) ||
          keybinding.includes(query)
        )
      })
      .sort((left, right) =>
        left.category.localeCompare(right.category) ||
        left.title.localeCompare(right.title) ||
        left.id.localeCompare(right.id)
      )
  }

  const conflictsFor = (
    registry: KhalaCodeCommandRegistry,
    id: KhalaCodeCommandId,
    config: string,
  ): readonly KhalaCodeCommandId[] => {
    const signatures = new Set(khalaCodeCommandKeybindingSignatures(
      parseKhalaCodeCommandKeybindingConfig(config),
    ))
    if (signatures.size === 0) return []
    const conflicts: KhalaCodeCommandId[] = []
    for (const command of registry.commands()) {
      if (command.id === id) continue
      const overlap = registry.effectiveKeybindings(command.id)
        .some(binding => signatures.has(khalaCodeCommandKeybindingSignatures([binding])[0] ?? ""))
      if (overlap) conflicts.push(command.id)
    }
    return conflicts
  }

  const setOverride = (
    registry: KhalaCodeCommandRegistry,
    id: KhalaCodeCommandId,
    value: string | null,
  ): void => {
    options.setOverride(id, value)
    conflictIds = new Set()
    activeCommandId = null
    options.onChanged?.()
    renderRows(registry)
  }

  const capture = (
    registry: KhalaCodeCommandRegistry,
    id: KhalaCodeCommandId,
    event: KeyboardEvent,
  ): void => {
    event.preventDefault()
    event.stopPropagation()
    event.stopImmediatePropagation()
    if (event.key === "Escape") {
      activeCommandId = null
      syncStatus("")
      renderRows(registry)
      return
    }
    if (
      (event.key === "Backspace" || event.key === "Delete") &&
      !event.altKey &&
      !event.ctrlKey &&
      !event.metaKey &&
      !event.shiftKey
    ) {
      setOverride(registry, id, "none")
      syncStatus(`${titleFor(registry, id)} is unassigned.`)
      return
    }
    const next = khalaCodeCommandKeybindingConfigForKeyboardEvent(event)
    if (next === null) return
    const conflicts = conflictsFor(registry, id, next)
    if (conflicts.length > 0) {
      conflictIds = new Set([id, ...conflicts])
      syncStatus(
        `${titleFor(registry, id)} conflicts with ${conflicts.map(conflict => titleFor(registry, conflict)).join(", ")} for ${formatKhalaCodeCommandKeybindingConfig(next)}.`,
      )
      renderRows(registry)
      return
    }
    setOverride(registry, id, next)
    syncStatus(`${titleFor(registry, id)} set to ${formatKhalaCodeCommandKeybindingConfig(next)}.`)
  }

  const commandRow = (
    registry: KhalaCodeCommandRegistry,
    id: KhalaCodeCommandId,
  ): HTMLElement => {
    const command = registry.command(id)
    const overrides = options.getOverrides()
    const row = el("div", "khala-keybindings-row")
    row.dataset.commandId = id
    row.dataset.conflict = conflictIds.has(id) ? "true" : "false"

    const label = el("div", "khala-keybindings-label")
    label.append(
      el("span", "khala-keybindings-title", command?.title ?? id),
      el("span", "khala-keybindings-meta", `${categoryLabel(command?.category ?? "workbench")} / ${id}`),
    )

    const keyButton = el("button", "khala-keybindings-capture")
    keyButton.type = "button"
    keyButton.dataset.commandKeybindingCapture = id
    keyButton.textContent = activeCommandId === id
      ? "Press keys"
      : registry.keybindingLabel(id) || "Unassigned"
    keyButton.title = "Click, then press the new keybinding"
    keyButton.addEventListener("click", () => {
      activeCommandId = activeCommandId === id ? null : id
      conflictIds = new Set()
      syncStatus(activeCommandId === null ? "" : `Recording ${command?.title ?? id}. Press Escape to cancel.`)
      renderRows(registry)
      requestAnimationFrame(() => {
        sectionNode
          ?.querySelector<HTMLButtonElement>(`[data-command-keybinding-capture="${id}"]`)
          ?.focus()
      })
    })
    keyButton.addEventListener("keydown", event => {
      if (activeCommandId !== id) return
      capture(registry, id, event)
    })

    const clear = el("button", "khala-keybindings-action", "Clear")
    clear.type = "button"
    clear.disabled = registry.effectiveKeybindings(id).length === 0
    clear.addEventListener("click", () => {
      setOverride(registry, id, "none")
      syncStatus(`${command?.title ?? id} is unassigned.`)
    })

    const reset = el("button", "khala-keybindings-action", "Reset")
    reset.type = "button"
    reset.disabled = !hasOwn(overrides, id)
    reset.addEventListener("click", () => {
      setOverride(registry, id, null)
      syncStatus(`${command?.title ?? id} restored to default.`)
    })

    row.append(label, keyButton, clear, reset)
    return row
  }

  function renderRows(registry: KhalaCodeCommandRegistry): void {
    if (listNode === null) return
    const commands = matchingCommands(registry)
    if (commands.length === 0) {
      listNode.replaceChildren(el("div", "khala-settings-empty", "No matching keybindings"))
      return
    }
    const groups = new Map<string, HTMLElement[]>()
    for (const command of commands) {
      groups.set(command.category, [...(groups.get(command.category) ?? []), commandRow(registry, command.id)])
    }
    const nodes: HTMLElement[] = []
    for (const [group, rows] of groups) {
      nodes.push(el("h4", "khala-keybindings-group", categoryLabel(group)))
      nodes.push(...rows)
    }
    listNode.replaceChildren(...nodes)
  }

  return {
    render() {
      const registry = options.registry()
      const section = el("section", "khala-settings-section khala-keybindings-section")
      sectionNode = section
      section.append(el("h3", "khala-settings-section-title", "Keybindings"))
      if (registry === null) {
        section.append(el("p", "khala-settings-empty", "Command registry is not ready."))
        return section
      }

      const toolbar = el("div", "khala-keybindings-toolbar")
      const search = el("input", "khala-settings-select khala-keybindings-search")
      search.type = "search"
      search.name = "khala-code-keybinding-search"
      search.placeholder = "Search commands"
      search.setAttribute("aria-label", "Search keybindings")
      search.value = filter
      search.addEventListener("input", () => {
        filter = search.value
        conflictIds = new Set()
        renderRows(registry)
      })
      const resetAll = el("button", "khala-settings-refresh", "Reset all")
      resetAll.type = "button"
      resetAll.disabled = Object.keys(options.getOverrides()).length === 0
      resetAll.addEventListener("click", () => {
        options.resetAll()
        activeCommandId = null
        conflictIds = new Set()
        options.onChanged?.()
        syncStatus("All keybindings restored to defaults.")
        renderRows(registry)
        resetAll.disabled = true
      })
      toolbar.append(search, resetAll)

      const status = el("div", "khala-settings-status khala-keybindings-status")
      status.hidden = true
      statusNode = status
      const list = el("div", "khala-keybindings-list")
      listNode = list
      section.append(toolbar, status, list)
      renderRows(registry)
      return section
    },
    stopCapture() {
      activeCommandId = null
      conflictIds = new Set()
    },
  }
}
