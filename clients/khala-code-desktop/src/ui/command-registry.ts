export type KhalaCodeCommandCategory =
  | "composer"
  | "help"
  | "navigation"
  | "session"
  | "settings"
  | "workbench"

export type KhalaCodeCommandPaletteKind =
  | "command"
  | "file"
  | "model"
  | "project"
  | "provider"
  | "server"
  | "session"

export type KhalaCodeCommandId =
  | "composer.attach_file"
  | "composer.focus"
  | "composer.stop_turn"
  | "help.bug_report"
  | "help.copy_issue_metadata"
  | "help.docs"
  | "help.export_diagnostics"
  | "help.feedback"
  | "help.release_notes"
  | "help.support"
  | "palette.open"
  | "message.next"
  | "message.previous"
  | "session.archive"
  | "session.fork"
  | "session.new_chat"
  | "session.next"
  | "session.previous"
  | "session.refresh"
  | "session.restore_closed"
  | "session.share"
  | "session.unarchive"
  | "session.unshare"
  | "view.chat"
  | "view.editor"
  | "view.fleet"
  | "view.forum"
  | "view.home"
  | "view.inbox"
  | "view.review"
  | "view.settings"

export type KhalaCodeCommandKeybinding = Readonly<{
  key: string
  alt?: boolean
  ctrl?: boolean
  meta?: boolean
  shift?: boolean
}>

export type KhalaCodeCommandKeybindingOverrideMap = Readonly<
  Partial<Record<KhalaCodeCommandId, string>>
>

export type KhalaCodeCommandDefinition = Readonly<{
  analyticsRef: string
  available?: () => boolean
  category: KhalaCodeCommandCategory
  defaultKeybindings?: readonly KhalaCodeCommandKeybinding[]
  disabledReason?: () => string
  execute: () => void | Promise<void>
  id: KhalaCodeCommandId
  keywords?: readonly string[]
  subtitle?: string
  title: string
}>

export type KhalaCodeCommandPaletteRecord = Readonly<{
  disabled?: boolean
  disabledReason?: string
  group: string
  id: string
  keybindingLabel?: string
  kind: KhalaCodeCommandPaletteKind
  metadataRef: string
  scoreHints?: readonly string[]
  subtitle?: string
  title: string
}>

export type KhalaCodeCommandPaletteResult = KhalaCodeCommandPaletteRecord & Readonly<{
  score: number
}>

export type KhalaCodeCommandRegistry = Readonly<{
  command: (id: KhalaCodeCommandId) => KhalaCodeCommandDefinition | null
  commands: () => readonly KhalaCodeCommandDefinition[]
  effectiveKeybindings: (id: KhalaCodeCommandId) => readonly KhalaCodeCommandKeybinding[]
  execute: (id: KhalaCodeCommandId) => Promise<boolean>
  keybindingLabel: (id: KhalaCodeCommandId) => string
  recordForCommand: (id: KhalaCodeCommandId) => KhalaCodeCommandPaletteRecord | null
  search: (input: {
    includeDisabled?: boolean
    limit?: number
    query: string
    records?: readonly KhalaCodeCommandPaletteRecord[]
  }) => readonly KhalaCodeCommandPaletteResult[]
}>

export type KhalaCodeCommandRegistryOptions = Readonly<{
  getKeybindingOverrides?: () => KhalaCodeCommandKeybindingOverrideMap
}>

const normalizeSearchText = (value: string): string =>
  value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, " ")
    .trim()

const normalizeKeybindingKey = (value: string): string => {
  const key = value.trim()
  if (key === "") return ""
  switch (key.toLowerCase()) {
    case " ":
    case "space":
    case "spacebar":
      return "space"
    case ",":
    case "comma":
      return "comma"
    case "+":
    case "plus":
      return "plus"
    case "arrowup":
    case "up":
      return "arrowup"
    case "arrowdown":
    case "down":
      return "arrowdown"
    case "arrowleft":
    case "left":
      return "arrowleft"
    case "arrowright":
    case "right":
      return "arrowright"
    case "esc":
    case "escape":
      return "escape"
    default:
      return key.length === 1 ? key.toLowerCase() : key.toLowerCase()
  }
}

const keybindingKeyLabel = (key: string): string => {
  switch (normalizeKeybindingKey(key)) {
    case "space":
      return "Space"
    case "comma":
      return ","
    case "plus":
      return "+"
    case "arrowup":
      return "Up"
    case "arrowdown":
      return "Down"
    case "arrowleft":
      return "Left"
    case "arrowright":
      return "Right"
    case "escape":
      return "Esc"
    default:
      return key.length === 1 ? key.toUpperCase() : key
  }
}

export const formatKhalaCodeCommandKeybinding = (
  binding: KhalaCodeCommandKeybinding,
): string => {
  const parts: string[] = []
  if (binding.meta) parts.push("Cmd")
  if (binding.ctrl) parts.push("Ctrl")
  if (binding.alt) parts.push("Alt")
  if (binding.shift) parts.push("Shift")
  parts.push(keybindingKeyLabel(binding.key))
  return parts.join("+")
}

export const serializeKhalaCodeCommandKeybinding = (
  binding: KhalaCodeCommandKeybinding,
): string => {
  const parts: string[] = []
  if (binding.meta) parts.push("meta")
  if (binding.ctrl) parts.push("ctrl")
  if (binding.alt) parts.push("alt")
  if (binding.shift) parts.push("shift")
  parts.push(normalizeKeybindingKey(binding.key))
  return parts.join("+")
}

export const parseKhalaCodeCommandKeybindingConfig = (
  config: string | undefined,
): readonly KhalaCodeCommandKeybinding[] => {
  if (config === undefined) return []
  const trimmed = config.trim()
  if (trimmed === "" || trimmed.toLowerCase() === "none") return []
  return trimmed
    .split(",")
    .map(combo => combo.trim())
    .filter(Boolean)
    .map(combo => {
      const binding: {
        alt?: boolean
        ctrl?: boolean
        key?: string
        meta?: boolean
        shift?: boolean
      } = {}
      for (const rawPart of combo.split("+").map(part => part.trim()).filter(Boolean)) {
        const part = rawPart.toLowerCase()
        if (part === "alt" || part === "option") binding.alt = true
        else if (part === "ctrl" || part === "control") binding.ctrl = true
        else if (part === "cmd" || part === "command" || part === "meta") binding.meta = true
        else if (part === "shift") binding.shift = true
        else binding.key = normalizeKeybindingKey(rawPart)
      }
      return binding.key === undefined || binding.key.length === 0
        ? null
        : {
            key: binding.key,
            ...(binding.alt === true ? { alt: true } : {}),
            ...(binding.ctrl === true ? { ctrl: true } : {}),
            ...(binding.meta === true ? { meta: true } : {}),
            ...(binding.shift === true ? { shift: true } : {}),
          }
    })
    .filter((binding): binding is KhalaCodeCommandKeybinding => binding !== null)
}

export const formatKhalaCodeCommandKeybindingConfig = (
  config: string | undefined,
): string =>
  parseKhalaCodeCommandKeybindingConfig(config)
    .map(formatKhalaCodeCommandKeybinding)
    .join(", ")

export const khalaCodeCommandKeybindingConfigForKeyboardEvent = (
  event: KeyboardEvent,
): string | null => {
  if (
    event.key === "Alt" ||
    event.key === "Control" ||
    event.key === "Meta" ||
    event.key === "Shift"
  ) {
    return null
  }
  const key = normalizeKeybindingKey(event.key)
  if (key.length === 0) return null
  return serializeKhalaCodeCommandKeybinding({
    key,
    ...(event.altKey ? { alt: true } : {}),
    ...(event.ctrlKey ? { ctrl: true } : {}),
    ...(event.metaKey ? { meta: true } : {}),
    ...(event.shiftKey ? { shift: true } : {}),
  })
}

export const khalaCodeCommandKeybindingSignature = (
  binding: KhalaCodeCommandKeybinding,
): string => serializeKhalaCodeCommandKeybinding(binding)

export const khalaCodeCommandKeybindingSignatures = (
  bindings: readonly KhalaCodeCommandKeybinding[],
): readonly string[] => bindings.map(khalaCodeCommandKeybindingSignature)

const keybindingMatches = (
  binding: KhalaCodeCommandKeybinding,
  event: KeyboardEvent,
): boolean =>
  normalizeKeybindingKey(event.key) === normalizeKeybindingKey(binding.key) &&
  event.altKey === (binding.alt ?? false) &&
  event.ctrlKey === (binding.ctrl ?? false) &&
  event.metaKey === (binding.meta ?? false) &&
  event.shiftKey === (binding.shift ?? false)

const commandRecord = (
  command: KhalaCodeCommandDefinition,
  keybindings: readonly KhalaCodeCommandKeybinding[],
): KhalaCodeCommandPaletteRecord => {
  const available = command.available?.() ?? true
  const binding = keybindings[0]
  return {
    group: command.category,
    id: command.id,
    kind: "command",
    metadataRef: command.analyticsRef,
    title: command.title,
    ...(available ? {} : {
      disabled: true,
      disabledReason: command.disabledReason?.() ?? "Unavailable",
    }),
    ...(binding === undefined ? {} : { keybindingLabel: formatKhalaCodeCommandKeybinding(binding) }),
    ...(command.keywords === undefined ? {} : { scoreHints: command.keywords }),
    ...(command.subtitle === undefined ? {} : { subtitle: command.subtitle }),
  }
}

const scoreRecord = (
  record: KhalaCodeCommandPaletteRecord,
  query: string,
): number | null => {
  const normalizedQuery = normalizeSearchText(query)
  if (normalizedQuery === "") return 1
  const title = normalizeSearchText(record.title)
  const subtitle = normalizeSearchText(record.subtitle ?? "")
  const hints = normalizeSearchText((record.scoreHints ?? []).join(" "))
  if (title === normalizedQuery) return 100
  if (title.startsWith(normalizedQuery)) return 80
  if (title.includes(normalizedQuery)) return 60
  if (subtitle.includes(normalizedQuery)) return 40
  if (hints.includes(normalizedQuery)) return 30
  const queryParts = normalizedQuery.split(" ").filter(Boolean)
  if (queryParts.length > 1 && queryParts.every(part => `${title} ${subtitle} ${hints}`.includes(part))) {
    return 20
  }
  return null
}

export const createKhalaCodeCommandRegistry = (
  commands: readonly KhalaCodeCommandDefinition[],
  options: KhalaCodeCommandRegistryOptions = {},
): KhalaCodeCommandRegistry => {
  const byId = new Map(commands.map(command => [command.id, command]))
  const effectiveKeybindingsFor = (
    command: KhalaCodeCommandDefinition,
  ): readonly KhalaCodeCommandKeybinding[] => {
    const overrides = options.getKeybindingOverrides?.() ?? {}
    if (Object.prototype.hasOwnProperty.call(overrides, command.id)) {
      return parseKhalaCodeCommandKeybindingConfig(overrides[command.id])
    }
    return command.defaultKeybindings ?? []
  }
  return {
    command: id => byId.get(id) ?? null,
    commands: () => [...commands],
    effectiveKeybindings: id => {
      const command = byId.get(id)
      return command === undefined ? [] : effectiveKeybindingsFor(command)
    },
    execute: async id => {
      const command = byId.get(id)
      if (command === undefined || command.available?.() === false) return false
      await command.execute()
      return true
    },
    keybindingLabel: id => {
      const command = byId.get(id)
      const binding = command === undefined ? undefined : effectiveKeybindingsFor(command)[0]
      return binding === undefined ? "" : formatKhalaCodeCommandKeybinding(binding)
    },
    recordForCommand: id => {
      const command = byId.get(id)
      return command === undefined ? null : commandRecord(command, effectiveKeybindingsFor(command))
    },
    search: input => {
      const limit = Math.max(1, Math.trunc(input.limit ?? 12))
      return [
        ...commands.map(command => commandRecord(command, effectiveKeybindingsFor(command))),
        ...(input.records ?? []),
      ].filter(record => input.includeDisabled === true || record.disabled !== true)
        .map(record => {
          const score = scoreRecord(record, input.query)
          return score === null ? null : { ...record, score }
        })
        .filter((record): record is KhalaCodeCommandPaletteResult => record !== null)
        .sort((left, right) =>
          right.score - left.score ||
          Number(left.disabled === true) - Number(right.disabled === true) ||
          left.group.localeCompare(right.group) ||
          left.title.localeCompare(right.title) ||
          left.id.localeCompare(right.id)
        )
        .slice(0, limit)
    },
  }
}

export const khalaCodeCommandForKeyboardEvent = (
  registry: KhalaCodeCommandRegistry,
  event: KeyboardEvent,
): KhalaCodeCommandId | null => {
  if (event.defaultPrevented) return null
  for (const command of registry.commands()) {
    if (registry.effectiveKeybindings(command.id).some(binding => keybindingMatches(binding, event))) {
      return command.id
    }
  }
  return null
}
