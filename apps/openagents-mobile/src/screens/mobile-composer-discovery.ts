import {
  Combobox,
  ComponentValueBinding,
  IntentRef,
  type ComposerAutocomplete,
} from "@effect-native/core"
import type { MobileComposerPathEntry } from "../coding/mobile-composer-path-context"

export const mobileSlashCommandIds = [
  "mobile.command.new_chat",
  "mobile.command.choose_target",
  "mobile.command.attach",
  "mobile.command.stop_turn",
] as const

export type MobileSlashCommandId = (typeof mobileSlashCommandIds)[number]

export type MobileSlashCommandContext = Readonly<{
  composerAvailable: boolean
  targetCatalogAvailable: boolean
  attachmentPickerAvailable: boolean
  activeTurnRef: string | null
  activeTurnCancelable: boolean
  pendingAction: boolean
}>

export type MobileSlashCommand = Readonly<{
  id: MobileSlashCommandId
  invocation: string
  label: string
  description: string
  available: boolean
  unavailableReason?: string
}>

const command = (
  input: Omit<MobileSlashCommand, "available" | "unavailableReason">,
  available: boolean,
  unavailableReason?: string,
): MobileSlashCommand => ({
  ...input,
  available,
  ...(available || unavailableReason === undefined ? {} : { unavailableReason }),
})

export const mobileSlashCommands = (
  context: MobileSlashCommandContext,
): ReadonlyArray<MobileSlashCommand> => [
  command({
    id: "mobile.command.new_chat",
    invocation: "new",
    label: "New chat",
    description: "Start a new conversation",
  }, !context.pendingAction, "Wait for the current action to settle"),
  command({
    id: "mobile.command.choose_target",
    invocation: "target",
    label: "Choose target",
    description: "Select an authenticated execution target and model",
  }, context.composerAvailable && context.targetCatalogAvailable && !context.pendingAction,
  context.composerAvailable ? "Execution targets are unavailable" : "Open a coding session first"),
  command({
    id: "mobile.command.attach",
    invocation: "attach",
    label: "Add attachment",
    description: "Choose a file or image for this draft",
  }, context.composerAvailable && context.attachmentPickerAvailable && !context.pendingAction,
  context.composerAvailable ? "Attachment picker is busy or unavailable" : "Open a coding session first"),
  command({
    id: "mobile.command.stop_turn",
    invocation: "stop",
    label: "Stop turn",
    description: "Interrupt the exact active runtime turn",
  }, context.activeTurnRef !== null && context.activeTurnCancelable && !context.pendingAction,
  context.activeTurnRef === null ? "No active turn" : "This turn cannot be stopped right now"),
]

export type MobileComposerSlashTrigger = Readonly<{
  query: string
  replaceFrom: number
}>

export type MobileComposerPathTrigger = Readonly<{
  query: string
  replaceFrom: number
}>

export type MobileComposerPathDiscoveryState =
  | Readonly<{ state: "idle" }>
  | Readonly<{ state: "loading"; query: string }>
  | Readonly<{ state: "ready"; query: string; entries: ReadonlyArray<MobileComposerPathEntry> }>
  | Readonly<{ state: "unavailable"; query: string; message: string }>
  | Readonly<{ state: "failed"; query: string; message: string }>

/** Deterministic parsing begins only after the explicit slash trigger. */
export const mobileComposerSlashTrigger = (text: string): MobileComposerSlashTrigger | null => {
  const match = /(?:^|\s)\/([A-Za-z0-9_-]*)$/u.exec(text)
  if (match === null) return null
  const slashOffset = match.index + (match[0].startsWith("/") ? 0 : 1)
  return { query: match[1] ?? "", replaceFrom: slashOffset }
}

export const mobileComposerPathTrigger = (text: string): MobileComposerPathTrigger | null => {
  const match = /(?:^|\s)@([^\s@]*)$/u.exec(text)
  if (match === null) return null
  const atOffset = match.index + (match[0].startsWith("@") ? 0 : 1)
  return { query: match[1] ?? "", replaceFrom: atOffset }
}

export const projectMobileSlashCommands = (
  text: string,
  context: MobileSlashCommandContext,
): Readonly<{ trigger: MobileComposerSlashTrigger; commands: ReadonlyArray<MobileSlashCommand> }> | null => {
  const trigger = mobileComposerSlashTrigger(text)
  if (trigger === null) return null
  const query = trigger.query.toLocaleLowerCase()
  return {
    trigger,
    commands: mobileSlashCommands(context).filter(value =>
      query === "" || `${value.invocation} ${value.label}`.toLocaleLowerCase().includes(query)),
  }
}

export const renderMobileSlashCommandAutocomplete = (
  text: string,
  context: MobileSlashCommandContext,
): ComposerAutocomplete | undefined => {
  const projected = projectMobileSlashCommands(text, context)
  if (projected === null) return undefined
  return {
    trigger: "slash",
    query: projected.trigger.query,
    combobox: Combobox({
      key: "khala-coding-composer-slash-commands",
      query: projected.trigger.query,
      placeholder: "Search commands",
      options: projected.commands.map(value => ({
        id: value.id,
        label: `/${value.invocation} · ${value.label}`,
        subtitle: value.available ? value.description : value.unavailableReason,
        group: "Commands",
        disabled: !value.available,
        ...(value.unavailableReason === undefined ? {} : { disabledReason: value.unavailableReason }),
      })),
      emptyLabel: "No commands match this slash token.",
      onQueryChange: IntentRef("CodingComposerSlashQueryChanged", ComponentValueBinding()),
      onSelect: IntentRef("CodingComposerSlashCommandSelected", ComponentValueBinding()),
      style: { width: "full" },
      a11y: { role: "listbox", label: "Composer commands" },
    }),
  }
}

export const renderMobilePathAutocomplete = (
  text: string,
  state: MobileComposerPathDiscoveryState,
): ComposerAutocomplete | undefined => {
  const trigger = mobileComposerPathTrigger(text)
  if (trigger === null) return undefined
  const current = state.state !== "idle" && state.query === trigger.query ? state : { state: "loading" as const, query: trigger.query }
  const entries = current.state === "ready" ? current.entries : []
  const emptyLabel = current.state === "unavailable" || current.state === "failed"
    ? current.message
    : current.state === "loading"
      ? "Searching this worktree…"
      : trigger.query === ""
        ? "Type a file or folder name."
        : "No matching files or folders."
  return {
    trigger: "mention",
    query: trigger.query,
    combobox: Combobox({
      key: "khala-coding-composer-path-context",
      query: trigger.query,
      placeholder: "Search this worktree",
      options: entries.map(entry => ({
        id: entry.pathRef,
        label: entry.pathRef.split("/").at(-1) ?? entry.pathRef,
        subtitle: `${entry.pathRef} · ${entry.kind}`,
        group: "Worktree",
        icon: entry.kind === "directory" ? "Folder" : "File",
      })),
      loading: current.state === "loading",
      emptyLabel,
      onQueryChange: IntentRef("CodingComposerPathQueryChanged", ComponentValueBinding()),
      onSelect: IntentRef("CodingComposerPathSelected", ComponentValueBinding()),
      style: { width: "full" },
      a11y: { role: "listbox", label: "Repository files and folders" },
    }),
  }
}
