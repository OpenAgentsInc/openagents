export type KhalaCodeCommandCategory =
  | "composer"
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
  | "palette.open"
  | "session.new_chat"
  | "session.refresh"
  | "view.chat"
  | "view.editor"
  | "view.fleet"
  | "view.forum"
  | "view.inbox"
  | "view.settings"

export type KhalaCodeCommandKeybinding = Readonly<{
  key: string
  alt?: boolean
  ctrl?: boolean
  meta?: boolean
  shift?: boolean
}>

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
  execute: (id: KhalaCodeCommandId) => Promise<boolean>
  recordForCommand: (id: KhalaCodeCommandId) => KhalaCodeCommandPaletteRecord | null
  search: (input: {
    includeDisabled?: boolean
    limit?: number
    query: string
    records?: readonly KhalaCodeCommandPaletteRecord[]
  }) => readonly KhalaCodeCommandPaletteResult[]
}>

const normalizeSearchText = (value: string): string =>
  value
    .normalize("NFKD")
    .toLowerCase()
    .replace(/[^a-z0-9]+/gu, " ")
    .trim()

const keybindingLabel = (binding: KhalaCodeCommandKeybinding): string => {
  const parts: string[] = []
  if (binding.meta) parts.push("Cmd")
  if (binding.ctrl) parts.push("Ctrl")
  if (binding.alt) parts.push("Alt")
  if (binding.shift) parts.push("Shift")
  parts.push(binding.key.length === 1 ? binding.key.toUpperCase() : binding.key)
  return parts.join("+")
}

const keybindingMatches = (
  binding: KhalaCodeCommandKeybinding,
  event: KeyboardEvent,
): boolean =>
  event.key.toLowerCase() === binding.key.toLowerCase() &&
  event.altKey === (binding.alt ?? false) &&
  event.ctrlKey === (binding.ctrl ?? false) &&
  event.metaKey === (binding.meta ?? false) &&
  event.shiftKey === (binding.shift ?? false)

const commandRecord = (
  command: KhalaCodeCommandDefinition,
): KhalaCodeCommandPaletteRecord => {
  const available = command.available?.() ?? true
  const binding = command.defaultKeybindings?.[0]
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
    ...(binding === undefined ? {} : { keybindingLabel: keybindingLabel(binding) }),
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
): KhalaCodeCommandRegistry => {
  const byId = new Map(commands.map(command => [command.id, command]))
  return {
    command: id => byId.get(id) ?? null,
    commands: () => [...commands],
    execute: async id => {
      const command = byId.get(id)
      if (command === undefined || command.available?.() === false) return false
      await command.execute()
      return true
    },
    recordForCommand: id => {
      const command = byId.get(id)
      return command === undefined ? null : commandRecord(command)
    },
    search: input => {
      const limit = Math.max(1, Math.trunc(input.limit ?? 12))
      return [
        ...commands.map(commandRecord),
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
    if ((command.defaultKeybindings ?? []).some(binding => keybindingMatches(binding, event))) {
      return command.id
    }
  }
  return null
}
