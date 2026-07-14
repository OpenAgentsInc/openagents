import {
  decodeDesktopDeferredCommand,
  desktopCanonicalCommandRegistry,
  type DesktopDeferredCommand,
  type DesktopCommandId,
} from "@openagentsinc/openagents-desktop/desktop-command-contract"
import { resolveDesktopDeferredCommandIntent } from "@openagentsinc/openagents-desktop/renderer-command-registry"

export type NativeProductionCommandAction = "new_chat" | "workspace_chat" | "workspace_home" | "settings"

export type NativeProductionCommandBinding = Readonly<{
  action: NativeProductionCommandAction
  commandId: DesktopCommandId
  intentName: string
}>

/**
 * Native controls consume the production Desktop command IDs and intent names.
 * This is deliberately imported from the real app contract rather than copied.
 */
export const nativeProductionCommandBindings: ReadonlyArray<NativeProductionCommandBinding> = [
  { action: "new_chat", commandId: "chat.new", intentName: "DesktopNewChat" },
  { action: "workspace_chat", commandId: "chat.open", intentName: "DesktopWorkspaceSelected" },
  { action: "workspace_home", commandId: "workspace.home", intentName: "DesktopWorkspaceSelected" },
  { action: "settings", commandId: "settings.open", intentName: "DesktopSettingsToggled" },
]

const productionById = new Map(desktopCanonicalCommandRegistry.map((command) => [command.id, command] as const))

export const assertNativeProductionCommandBindings = (): void => {
  for (const binding of nativeProductionCommandBindings) {
    const production = productionById.get(binding.commandId)
    if (production === undefined || production.intentName !== binding.intentName) {
      throw new Error(`native_production_command_drift:${binding.action}`)
    }
  }
}

export const productionCommandIdForAction = (action: NativeProductionCommandAction): DesktopCommandId => {
  const binding = nativeProductionCommandBindings.find((candidate) => candidate.action === action)
  if (binding === undefined) throw new Error(`native_production_command_unknown:${action}`)
  return binding.commandId
}

export type ResolvedNativeDeferredCommand = Readonly<{
  command: DesktopDeferredCommand
  intentName: string
  payload: null | string
}>

/** Decode and resolve through the production Desktop command contracts. */
export const resolveNativeDeferredCommand = (candidate: unknown): ResolvedNativeDeferredCommand => {
  const command = decodeDesktopDeferredCommand(candidate, { onExcessProperty: "error" })
  const resolved = resolveDesktopDeferredCommandIntent(command, {
    sessionReady: true,
    workspaceReady: true,
    verifiedOwner: true,
  })
  if (resolved.state !== "ready") {
    throw new Error(`native_deferred_command_rejected:${resolved.reason}`)
  }
  return { command, intentName: resolved.intentName, payload: resolved.payload }
}

assertNativeProductionCommandBindings()
