import {
  isReadOnlyCapabilitySet,
  verbAllowedByCapabilities,
  type BridgeRequestVerb,
  type Capability,
} from "./bridge.js"

export type ClientSurface = "mobile" | "desktop" | "web"

export type ClientCapabilityMatrix = {
  client: string
  verbs: string[]
  canSpawn: boolean
  canCancel: boolean
  canApprove: boolean
  readOnlyOnly: boolean
}

const READ_ONLY_CAPABILITIES = [
  "observe_public",
  "read_artifact",
] satisfies Capability[]

const READ_ONLY_BRIDGE_VERBS = [
  "bridge.clients.list",
  "session.list",
  "session.subscribe",
  "session.snapshot",
  "session.history",
  "artifact.read",
  "capability.list",
] satisfies BridgeRequestVerb[]

type ClientCapabilityConfig = {
  client: ClientSurface
  capabilities: readonly Capability[]
  verbs: readonly string[]
  canSpawn: boolean
}

const CLIENT_CAPABILITY_MATRIX: Record<ClientSurface, ClientCapabilityConfig> = {
  mobile: {
    client: "mobile",
    capabilities: READ_ONLY_CAPABILITIES,
    verbs: READ_ONLY_BRIDGE_VERBS,
    canSpawn: false,
  },
  desktop: {
    client: "desktop",
    capabilities: READ_ONLY_CAPABILITIES,
    verbs: READ_ONLY_BRIDGE_VERBS,
    canSpawn: false,
  },
  web: {
    client: "web",
    capabilities: READ_ONLY_CAPABILITIES,
    verbs: READ_ONLY_BRIDGE_VERBS,
    canSpawn: false,
  },
}

export function clientCapabilities(client: ClientSurface): ClientCapabilityMatrix {
  const config = CLIENT_CAPABILITY_MATRIX[client]

  return {
    client: config.client,
    verbs: [...config.verbs],
    canSpawn: config.canSpawn,
    canCancel: verbAllowedByCapabilities("session.cancel", config.capabilities),
    canApprove: verbAllowedByCapabilities("decision.resolve", config.capabilities),
    readOnlyOnly: isReadOnlyCapabilitySet(config.capabilities),
  }
}
