import { desktopCanonicalCommandRegistry } from "../desktop-command-contract.ts"

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
