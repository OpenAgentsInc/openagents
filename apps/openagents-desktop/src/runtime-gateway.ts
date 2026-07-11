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
  request: (request: DesktopRuntimeGatewayRequest) => DesktopRuntimeGatewayResponse | Promise<DesktopRuntimeGatewayResponse>
  subscribe: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  dispose: () => void
}>

export const desktopRuntimeCapabilities = (input: Readonly<{
  sessionLocalState: "signed_out" | "credential_present_unverified" | "session_ready" | "denied" | "unavailable"
  syncLocalState: "ready" | "unavailable"
  syncNetworkPhase: "idle" | "bootstrapping" | "catching_up" | "live" | "must_refetch" | "denied" | "closed"
}>): ReadonlyArray<CapabilityState> => [
  { id: "codex-history", state: "available" },
  { id: "workspace", state: "available" },
  { id: "git-review", state: "available" },
  { id: "provider-accounts", state: "available" },
  {
    id: "openagents-session",
    state: input.sessionLocalState === "session_ready" ? "available" : "unavailable",
    reason: input.sessionLocalState === "session_ready"
      ? undefined
      : input.sessionLocalState === "signed_out"
      ? "OpenAgents sign-in is required."
      : input.sessionLocalState === "credential_present_unverified"
        ? "Stored OpenAgents credentials require server verification."
        : input.sessionLocalState === "denied"
          ? "OpenAgents session access was denied."
          : "OS-encrypted OpenAgents session custody is unavailable.",
  },
  {
    id: "khala-sync",
    state: input.syncNetworkPhase === "live" ? "available" : "unavailable",
    reason: input.syncNetworkPhase === "live"
      ? undefined
      : input.syncLocalState === "unavailable"
        ? "Local Sync persistence is unavailable."
        : input.syncNetworkPhase === "idle"
          ? "Authenticated Sync is idle."
          : input.syncNetworkPhase === "closed"
            ? "Authenticated Sync is closed."
            : `Authenticated Sync is ${input.syncNetworkPhase}.`,
  },
  { id: "conversation-stream", state: "unavailable", reason: "The durable conversation runtime is not connected yet." },
]

export const createDesktopRuntimeGateway = (
  capabilities: ReadonlyArray<CapabilityState> | (() => ReadonlyArray<CapabilityState>) =
    () => desktopRuntimeCapabilities({ sessionLocalState: "unavailable", syncLocalState: "unavailable", syncNetworkPhase: "closed" }),
  sessionActions?: Readonly<{
    signIn: () => Promise<Readonly<{ state: "verified" | "cancelled" | "unavailable" }>>
    signOut: () => Promise<Readonly<{ state: "signed_out" | "unavailable" }>>
  }>,
  sessionPhase: () => "signed_out" | "unverified" | "session_ready" | "denied" | "unavailable" = () => "unavailable",
): DesktopRuntimeGateway => {
  let phase: "idle" | "ready" | "disposed" = "idle"
  let sequence = 0
  let sessionActionInFlight = false
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
            capabilities: typeof capabilities === "function" ? capabilities() : capabilities,
            kind: "runtime.bootstrap",
            lifecycle: phase === "idle" ? "starting" : phase,
            protocolVersion: DesktopRuntimeGatewayProtocolVersion,
            sessionPhase: sessionPhase(),
          },
        }
      }
      if (request.command.id === "session.sign_in") {
        if (sessionActions === undefined || sessionActionInFlight) {
          return Promise.resolve({
            kind: "session_outcome",
            commandId: request.commandId,
            status: "unavailable",
            phase: "unavailable",
          })
        }
        sessionActionInFlight = true
        return sessionActions.signIn()
          .then(result => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: result.state === "verified" ? "completed" as const : result.state,
            phase: result.state === "verified" ? "session_ready" as const : result.state === "cancelled" ? "signed_out" as const : "unavailable" as const,
          }))
          .catch(() => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: "unavailable" as const,
            phase: "unavailable" as const,
          }))
          .finally(() => { sessionActionInFlight = false })
      }
      if (request.command.id === "session.sign_out") {
        if (sessionActions === undefined || sessionActionInFlight) {
          return Promise.resolve({
            kind: "session_outcome",
            commandId: request.commandId,
            status: "unavailable",
            phase: "unavailable",
          })
        }
        sessionActionInFlight = true
        return sessionActions.signOut()
          .then(result => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: result.state === "signed_out" ? "completed" as const : "unavailable" as const,
            phase: result.state,
          }))
          .catch(() => ({
            kind: "session_outcome" as const,
            commandId: request.commandId,
            status: "unavailable" as const,
            phase: "unavailable" as const,
          }))
          .finally(() => { sessionActionInFlight = false })
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
