import {
  desktopCanonicalCommandRegistry,
  desktopCommandIsAvailable,
  type DesktopDeferredCommand,
} from "../desktop-command-contract.ts"

/**
 * Closed Desktop command registry. Entries name existing typed intents only;
 * it never carries a callback, host API, shell command, or user-entered route.
 * Palette buttons therefore dispatch the exact same intent as the visible UI.
 */
export const desktopCommandRegistry = desktopCanonicalCommandRegistry
  .filter(command => command.palette)
  .map(command => ({
    id: command.id,
    label: command.label,
    intentName: command.intentName,
    payload: command.defaultArguments.kind === "none"
      ? null
      : command.defaultArguments.workspace,
  }))

export type DesktopCommand = (typeof desktopCommandRegistry)[number]

export type DesktopDeferredCommandIntent =
  | Readonly<{ state: "ready"; intentName: string; payload: null | string }>
  | Readonly<{ state: "rejected"; reason: "argument_mismatch" | "duplicate" | "unavailable" | "unknown_command" }>

export const resolveDesktopDeferredCommandIntent = (
  command: DesktopDeferredCommand,
  state: Readonly<{ sessionReady: boolean; workspaceReady: boolean; verifiedOwner: boolean }>,
): DesktopDeferredCommandIntent => {
  if (command.delivery === "duplicate_rejected") return { state: "rejected", reason: "duplicate" }
  const definition = desktopCanonicalCommandRegistry.find(value => value.id === command.commandId)
  if (definition === undefined) return { state: "rejected", reason: "unknown_command" }
  if (definition.arguments !== command.arguments.kind) {
    return { state: "rejected", reason: "argument_mismatch" }
  }
  if (!desktopCommandIsAvailable(definition, state)) {
    return { state: "rejected", reason: "unavailable" }
  }
  return {
    state: "ready",
    intentName: definition.intentName,
    payload: command.arguments.kind === "none" ? null : command.arguments.workspace,
  }
}
