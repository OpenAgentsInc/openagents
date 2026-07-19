import { Schema } from "effect"

import { IdeExplorerCommandSchema } from "./ide/path-index-contract.ts"
import { DesktopWorkspacePathRefSchema } from "./workspace-contract.ts"

export const DesktopCommandId = Schema.Literals([
  "chat.new",
  "chat.send",
  "chat.steer_current",
  "chat.stop",
  "chat.queue_next",
  "window.fullscreen_toggle",
  "navigation.back",
  "navigation.forward",
  "chat.open",
  "palette.toggle",
  "interaction.question.submit",
  "interaction.approval.approve",
  "interaction.approval.deny",
  "interaction.plan.accept",
  "interaction.plan.request_changes",
  "interaction.plan.replan",
  "settings.open",
  "workspace.choose",
  "workspace.files",
  "workspace.open_file",
  "workspace.review",
  "workspace.explorer.refresh",
  "workspace.explorer.retry",
  "workspace.explorer.rescan",
  "editor.save",
  "editor.save_all",
  "editor.split.toggle",
  "editor.vim.toggle",
  "full-auto.launch",
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
  Schema.Struct({ kind: Schema.Literal("workspace"), workspace: Schema.Literals(["chat", "files", "home", "review"]) }),
  Schema.Struct({ kind: Schema.Literal("path"), pathRef: DesktopWorkspacePathRefSchema }),
  Schema.Struct({ kind: Schema.Literal("explorer"), command: IdeExplorerCommandSchema }),
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
  { id: "interaction.question.submit", label: "Submit selected question answer", intentName: "DesktopQuestionSubmitted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+Shift+Q", "Control+Shift+Q"], palette: true },
  { id: "interaction.approval.approve", label: "Approve pending tool request", intentName: "DesktopApprovalApproved", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "interaction.approval.deny", label: "Deny pending tool request", intentName: "DesktopApprovalDenied", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "interaction.plan.accept", label: "Accept pending plan", intentName: "DesktopPlanAccepted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "interaction.plan.request_changes", label: "Request plan changes", intentName: "DesktopPlanChangesRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "interaction.plan.replan", label: "Request a new plan", intentName: "DesktopPlanReplanRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "verified_owner", defaultBindings: [], palette: true },
  { id: "chat.new", label: "New chat", intentName: "DesktopNewChat", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "always", authorization: "local_user", defaultBindings: ["Meta+N", "Control+N"], palette: true },
  { id: "chat.send", label: "Send message", intentName: "DesktopNoteSubmitted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "chat.steer_current", label: "Steer current turn", intentName: "DesktopSteerCurrentRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+Shift+Enter", "Control+Shift+Enter"], palette: true },
  { id: "chat.stop", label: "Stop turn", intentName: "DesktopTurnInterrupted", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+.", "Control+."], palette: true },
  { id: "chat.queue_next", label: "Queue next turn", intentName: "DesktopQueueNextRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "session_ready", authorization: "local_user", defaultBindings: ["Meta+Alt+Enter", "Control+Alt+Enter"], palette: true },
  { id: "window.fullscreen_toggle", label: "Toggle full screen", intentName: "DesktopFullscreenToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "always", authorization: "local_user", defaultBindings: ["Meta+F", "Control+F"], palette: true },
  // Collision review (#8825): arrows collide with editable/native movement,
  // brackets are reserved by common tab/menu conventions, and Electron menu
  // accelerators cannot honor the renderer's editable guard. Keep these typed,
  // discoverable, and rebindable without claiming an unsafe default chord.
  { id: "navigation.back", label: "Back", intentName: "DesktopNavigationBackRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "global", availability: "always", authorization: "local_user", defaultBindings: [], palette: false },
  { id: "navigation.forward", label: "Forward", intentName: "DesktopNavigationForwardRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "global", availability: "always", authorization: "local_user", defaultBindings: [], palette: false },
  { id: "chat.open", label: "Open chat", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "chat" }, result: "workspace_selected", scope: "session", availability: "always", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.files", label: "Toggle Files", intentName: "DesktopFilesModeToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: ["Meta+E", "Control+E"], palette: true },
  // Host-only macOS Open With delivery. Main replaces the workspace authority
  // before dispatch and supplies the selected relative path; the placeholder
  // default is never registered as a menu, palette, restore, or deep-link action.
  { id: "workspace.open_file", label: "Open system file", intentName: "DesktopSystemDocumentOpened", arguments: "path", defaultArguments: { kind: "path", pathRef: "README.md" }, result: "workspace_selected", scope: "workspace", availability: "always", authorization: "local_user", defaultBindings: [], palette: false },
  { id: "workspace.review", label: "Review changes", intentName: "DesktopWorkspaceSelected", arguments: "workspace", defaultArguments: { kind: "workspace", workspace: "review" }, result: "workspace_selected", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: false },
  { id: "workspace.explorer.refresh", label: "Refresh Explorer", intentName: "WorkspaceBrowserExplorerCommandRequested", arguments: "explorer", defaultArguments: { kind: "explorer", command: { _tag: "Refresh" } }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.explorer.retry", label: "Retry Explorer", intentName: "WorkspaceBrowserExplorerCommandRequested", arguments: "explorer", defaultArguments: { kind: "explorer", command: { _tag: "Retry" } }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.explorer.rescan", label: "Rescan Explorer", intentName: "WorkspaceBrowserExplorerCommandRequested", arguments: "explorer", defaultArguments: { kind: "explorer", command: { _tag: "Rescan" } }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "editor.save", label: "Save active editor", intentName: "WorkspaceEditorSaveRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "editor.save_all", label: "Save all editors", intentName: "WorkspaceEditorSaveAllRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: ["Meta+Alt+S", "Control+Alt+S"], palette: true },
  { id: "editor.split.toggle", label: "Toggle editor split", intentName: "WorkspaceEditorSplitToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "editor.vim.toggle", label: "Toggle Vim mode", intentName: "WorkspaceEditorVimToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "workspace", availability: "workspace_ready", authorization: "local_user", defaultBindings: [], palette: true },
  { id: "workspace.choose", label: "Choose workspace folder", intentName: "DesktopWorkspacePickerRequested", arguments: "none", defaultArguments: { kind: "none" }, result: "workspace_picker_requested", scope: "global", availability: "always", authorization: "local_user", defaultBindings: ["Meta+O", "Control+O"], palette: true },
  { id: "settings.open", label: "Open Settings", intentName: "DesktopSettingsToggled", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "global", availability: "always", authorization: "local_user", defaultBindings: ["Meta+,", "Control+,"], palette: true },
  // FA-AC-54 (#8974): the dedicated left-rail Full Auto launcher entry,
  // distinct from the retired composer toggle -- opens the mission-contract
  // form rather than applying to the next message.
  { id: "full-auto.launch", label: "Full Auto", intentName: "DesktopFullAutoLauncherOpened", arguments: "none", defaultArguments: { kind: "none" }, result: "dispatched", scope: "session", availability: "always", authorization: "local_user", defaultBindings: [], palette: true },
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
  source: Schema.Literals(["deep_link", "native_menu", "second_instance", "restore", "open_file"]),
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
