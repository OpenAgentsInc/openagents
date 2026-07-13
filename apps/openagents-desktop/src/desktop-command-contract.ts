import { Schema } from "effect"

export const DesktopCommandId = Schema.Literals([
  "chat.new",
  "chat.send",
  "chat.stop",
  "window.fullscreen_toggle",
  "chat.open",
  "palette.toggle",
  "settings.open",
  "workspace.choose",
  "workspace.files",
  "workspace.fleet",
  "workspace.home",
  "workspace.inbox",
  "workspace.review",
  "workspace.terminal",
])
export type DesktopCommandId = typeof DesktopCommandId.Type

export const DesktopCommandChord = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(80),
  Schema.isPattern(/^(Meta|Control|Alt|Shift)(\+(Meta|Control|Alt|Shift))*\+[A-Za-z0-9,./;'][A-Za-z0-9,./;'_-]*$/),
)
export type DesktopCommandChord = typeof DesktopCommandChord.Type

export const DesktopCommandArguments = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("none") }),
  Schema.Struct({ kind: Schema.Literal("workspace"), workspace: Schema.Literals(["chat", "files", "fleet", "home", "inbox", "review", "terminal"]) }),
])
export type DesktopCommandArguments = typeof DesktopCommandArguments.Type

export type DesktopCommandDefinition = Readonly<{
  id: DesktopCommandId
  label: string
  intentName: string
  arguments: DesktopCommandArguments["kind"]
  defaultArguments: DesktopCommandArguments
  result: "dispatched" | "workspace_selected" | "workspace_picker_requested"
  scope: "global" | "session" | "workspace"
  availability: "always" | "session_ready" | "workspace_ready"
  authorization: "local_user" | "verified_owner"
  defaultBindings: ReadonlyArray<DesktopCommandChord>
  palette: boolean
}>

export const desktopCanonicalCommandRegistry: ReadonlyArray<DesktopCommandDefinition> = [
  { id: "palette.toggle", label: "Commands", intentName: "DesktopCommandPaletteToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "global", availability: "always", authorization: "local_user", defaultBindings: ["Meta+K", "Control+K"], palette: false },
  { id: "chat.new", label: "New chat", intentName: "DesktopNewChat", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "always", authorization: "local_user", defaultBindings: ["Meta+N", "Control+N"], palette: true },
  { id: "chat.send", label: "Send message", intentName: "DesktopNoteSubmitted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+Enter", "Control+Enter"], palette: true },
  { id: "chat.stop", label: "Stop turn", intentName: "DesktopTurnInterrupted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+.", "Control+."], palette: true },
  { id: "window.fullscreen_toggle", label: "Toggle full screen", intentName: "DesktopFullscreenToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "always", authorization: "local_user", defaultBindings: ["Meta+F", "Control+F"], palette: true },
  { id: "workspace.fleet", label: "Open fleet", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "fleet" }, result: "workspace_selected", scope: "workspace", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "chat.open", label: "Open chat", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "chat" }, result: "workspace_selected", scope: "session", availability: "always", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.files", label: "Open Files", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "files" }, result: "workspace_selected", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.home", label: "Open Project Home", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "home" }, result: "workspace_selected", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.terminal", label: "Open Terminal", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "terminal" }, result: "workspace_selected", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.inbox", label: "Open Inbox", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "inbox" }, result: "workspace_selected", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "workspace.review", label: "Review changes", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "review" }, result: "workspace_selected", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.choose", label: "Choose workspace folder", intentName: "DesktopWorkspacePickerRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "workspace_picker_requested", scope: "global", availability: "always", authorization: "local_user", defaultBindings: ["Meta+O", "Control+O"], palette: true },
  { id: "settings.open", label: "Open Settings", intentName: "DesktopSettingsToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "global", availability: "always", authorization: "local_user", defaultBindings: ["Meta+,", "Control+,"], palette: true },
]

export type DesktopCommandBinding = Readonly<{
  commandId: DesktopCommandId
  chord: DesktopCommandChord
}>

export type DesktopCommandBindingResolution = Readonly<{
  bindings: ReadonlyArray<DesktopCommandBinding>
  conflicts: ReadonlyArray<Readonly<{
    chord: DesktopCommandChord
    commandIds: ReadonlyArray<DesktopCommandId>
  }>>
}>

export const normalizeDesktopCommandChord = (raw: string): DesktopCommandChord => {
  const parts = raw.split("+").map(value => value.trim()).filter(Boolean)
  const key = parts.at(-1) ?? ""
  const modifiers = new Set(parts.slice(0, -1).map(value =>
    value === "Cmd" ? "Meta" : value === "Ctrl" ? "Control" : value))
  const ordered = ["Meta", "Control", "Alt", "Shift"].filter(value => modifiers.has(value))
  return Schema.decodeUnknownSync(DesktopCommandChord)([...ordered, key.length === 1 ? key.toUpperCase() : key].join("+"))
}

export const resolveDesktopCommandBindings = (
  overrides: ReadonlyArray<Readonly<{ commandId: string; chord: string }>>,
): DesktopCommandBindingResolution => {
  const known = new Set(desktopCanonicalCommandRegistry.map(value => value.id))
  const validOverrides = new Map<DesktopCommandId, DesktopCommandChord>()
  for (const override of overrides) {
    if (!known.has(override.commandId as DesktopCommandId)) continue
    try {
      validOverrides.set(
        Schema.decodeUnknownSync(DesktopCommandId)(override.commandId),
        normalizeDesktopCommandChord(override.chord),
      )
    } catch {
      // Malformed user bindings are omitted and remain recoverable in settings.
    }
  }
  const bindings: DesktopCommandBinding[] = desktopCanonicalCommandRegistry.flatMap(command => {
    const override = validOverrides.get(command.id)
    return override === undefined
      ? command.defaultBindings.map(chord => ({ commandId: command.id, chord }))
      : [{ commandId: command.id, chord: override }]
  })
  const byChord = new Map<string, Set<DesktopCommandId>>()
  for (const binding of bindings) {
    const commands = byChord.get(binding.chord) ?? new Set<DesktopCommandId>()
    commands.add(binding.commandId)
    byChord.set(binding.chord, commands)
  }
  const conflicts = [...byChord.entries()]
    .filter(([, commands]) => commands.size > 1)
    .map(([chord, commands]) => ({
      chord: Schema.decodeUnknownSync(DesktopCommandChord)(chord),
      commandIds: [...commands].sort(),
    }))
    .sort((left, right) => left.chord.localeCompare(right.chord))
  const conflicted = new Set(conflicts.map(value => value.chord))
  return {
    bindings: bindings.filter(value => !conflicted.has(value.chord)),
    conflicts,
  }
}

export const DesktopDeferredCommand = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.deferred_command.v1"),
  requestRef: Schema.String.check(Schema.isPattern(/^command\.[A-Za-z0-9._:-]+$/)),
  commandId: DesktopCommandId,
  arguments: DesktopCommandArguments,
  source: Schema.Literals(["deep_link", "native_menu", "second_instance", "restore"]),
  delivery: Schema.Literals(["dispatch", "duplicate_rejected"]),
})
export type DesktopDeferredCommand = typeof DesktopDeferredCommand.Type
export const decodeDesktopDeferredCommand = Schema.decodeUnknownSync(DesktopDeferredCommand)
export const DesktopCommandEventChannel = "openagents:desktop-command:event"
export const DesktopCommandReadyChannel = "openagents:desktop-command:ready"
export const DesktopCommandBindingsChannel = "openagents:desktop-command:bindings"
export const DesktopCommandBindingSaveChannel = "openagents:desktop-command:binding-save"
export const DesktopCommandBindingsResetChannel = "openagents:desktop-command:bindings-reset"

export const DesktopCommandBindingUpdate = Schema.Struct({
  commandId: DesktopCommandId,
  chord: Schema.NullOr(DesktopCommandChord),
})
export type DesktopCommandBindingUpdate = typeof DesktopCommandBindingUpdate.Type
export const decodeDesktopCommandBindingUpdateOrNull = (value: unknown): DesktopCommandBindingUpdate | null => {
  const decoded = Schema.decodeUnknownExit(DesktopCommandBindingUpdate)(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const DesktopCommandBindingProjection = Schema.Struct({
  schema: Schema.Literal("openagents.desktop.command_bindings.v1"),
  rows: Schema.Array(Schema.Struct({
    commandId: DesktopCommandId,
    label: Schema.String,
    defaultBindings: Schema.Array(DesktopCommandChord),
    overrideBinding: Schema.NullOr(DesktopCommandChord),
    effectiveBindings: Schema.Array(DesktopCommandChord),
    conflict: Schema.Boolean,
  })),
  conflicts: Schema.Array(Schema.Struct({
    chord: DesktopCommandChord,
    commandIds: Schema.Array(DesktopCommandId),
  })),
})
export type DesktopCommandBindingProjection = typeof DesktopCommandBindingProjection.Type
export const decodeDesktopCommandBindingProjectionOrNull = (value: unknown): DesktopCommandBindingProjection | null => {
  const decoded = Schema.decodeUnknownExit(DesktopCommandBindingProjection)(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const decodeDesktopDeferredCommandOrNull = (value: unknown): DesktopDeferredCommand | null => {
  const decoded = Schema.decodeUnknownExit(DesktopDeferredCommand)(value)
  return decoded._tag === "Success" ? decoded.value : null
}

export const desktopCommandIsAvailable = (
  command: DesktopCommandDefinition,
  state: Readonly<{ sessionReady: boolean; workspaceReady: boolean; verifiedOwner: boolean }>,
): boolean =>
  (command.availability === "always" ||
    (command.availability === "session_ready" && state.sessionReady) ||
    (command.availability === "workspace_ready" && state.workspaceReady)) &&
  (command.authorization === "local_user" || state.verifiedOwner)
