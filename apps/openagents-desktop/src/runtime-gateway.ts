import type {
  DesktopRuntimeCapabilityId,
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayRequest,
  DesktopRuntimeGatewayResponse,
} from "./runtime-gateway-contract.ts"
import { DesktopRuntimeGatewayProtocolVersion } from "./runtime-gateway-contract.ts"

type CapabilityState = Readonly<{
  id: DesktopRuntimeCapabilityId
  state: "available" | "unavailable"
  reason?: string
}>

export type DesktopRuntimeGateway = Readonly<{
  start: () => void
  request: (request: DesktopRuntimeGatewayRequest) => DesktopRuntimeGatewayResponse
  subscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  dispose: () => void
}>

const defaultCapabilities: ReadonlyArray<CapabilityState> = [
  { id: "codex-history", state: "available" },
  { id: "workspace", state: "available" },
  { id: "git-review", state: "available" },
  { id: "provider-accounts", state: "available" },
  { id: "khala-sync", state: "unavailable", reason: "OpenAgents sign-in and Sync are not connected yet." },
  { id: "conversation-stream", state: "unavailable", reason: "The durable conversation runtime is not connected yet." },
]

export const createDesktopRuntimeGateway = (
  capabilities: ReadonlyArray<CapabilityState> = defaultCapabilities,
): DesktopRuntimeGateway => {
  let phase: "idle" | "ready" | "disposed" = "idle"
  let sequence = 0
  const listeners = new Set<(event: DesktopRuntimeGatewayEvent) => void>()

  const emit = (next: "ready" | "disposed"): void => {
    const event: DesktopRuntimeGatewayEvent = {
      kind: "runtime.lifecycle",
      phase: next,
      protocolVersion: DesktopRuntimeGatewayProtocolVersion,
      sequence: ++sequence,
    }
    for (const listener of [...listeners]) listener(event)
  }

  return {
    start: () => {
      if (phase !== "idle") return
      phase = "ready"
      emit("ready")
    },
    request: request => {
      if (phase === "disposed") return { kind: "request_rejected", reason: "gateway_disposed" }
      if (request.kind === "query") {
        return {
          kind: "query_result",
          requestId: request.requestId,
          result: {
            capabilities,
            kind: "runtime.bootstrap",
            lifecycle: phase === "idle" ? "starting" : phase,
            protocolVersion: DesktopRuntimeGatewayProtocolVersion,
          },
        }
      }
      return {
        kind: "command_outcome",
        commandId: request.commandId,
        status: "unavailable",
        reason: "Conversation interrupt is unavailable until the durable runtime is connected.",
      }
    },
    subscribe: listener => {
      if (phase === "disposed") {
        listener({
          kind: "runtime.lifecycle",
          phase: "disposed",
          protocolVersion: DesktopRuntimeGatewayProtocolVersion,
          sequence,
        })
        return () => undefined
      }
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    dispose: () => {
      if (phase === "disposed") return
      phase = "disposed"
      emit("disposed")
      listeners.clear()
    },
  }
}
