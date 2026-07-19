import {
  decodeIdeManagedSandboxCommandResult,
  decodeIdeManagedSandboxSnapshot,
  emptyIdeManagedSandboxSnapshot,
  type IdeManagedSandboxCommand,
  type IdeManagedSandboxCommandResult,
  type IdeManagedSandboxSnapshot,
} from "../../ide/managed-sandbox-contract.ts"

export type IdeManagedSandboxRendererHost = Readonly<{
  snapshot: () => Promise<unknown>
  command: (command: IdeManagedSandboxCommand) => Promise<unknown>
}>

export const unavailableIdeManagedSandboxRendererHost: IdeManagedSandboxRendererHost = {
  snapshot: async () => emptyIdeManagedSandboxSnapshot(),
  command: async () => ({
    _tag: "Refused",
    reason: "not_configured",
    message: "OpenAgents-managed placement is unavailable in this Desktop host.",
    snapshot: emptyIdeManagedSandboxSnapshot(),
  }),
}

export const loadIdeManagedSandboxRendererSnapshot = async (
  host: IdeManagedSandboxRendererHost,
): Promise<IdeManagedSandboxSnapshot> =>
  decodeIdeManagedSandboxSnapshot(await host.snapshot()) ?? emptyIdeManagedSandboxSnapshot()

export const executeIdeManagedSandboxRendererCommand = async (
  host: IdeManagedSandboxRendererHost,
  command: IdeManagedSandboxCommand,
): Promise<IdeManagedSandboxCommandResult> =>
  decodeIdeManagedSandboxCommandResult(await host.command(command)) ?? {
    _tag: "Refused",
    reason: "invalid_response",
    message: "The managed-placement host returned an invalid response.",
    snapshot: emptyIdeManagedSandboxSnapshot(),
  }
