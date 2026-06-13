export type NodeStatus = Readonly<{
  nodeRef: string
  online: boolean
  lastHeartbeatAt?: string
}>

export type ProviderHealth = Readonly<{
  provider: string
  online: boolean
}>

export type NodeStatusRowTone = "success" | "warning" | "danger" | "info" | "neutral"

export type NodeStatusRowViewModel = {
  nodeRef: string
  label: string
  statusLabel: "online" | "offline"
  tone: NodeStatusRowTone
  lastHeartbeatAt: string
}

export type ProviderHealthRowViewModel = {
  provider: string
  label: string
  statusLabel: "online" | "offline"
  tone: NodeStatusRowTone
}

const onlineLabel = (online: boolean): "online" | "offline" => (online ? "online" : "offline")

const onlineTone = (online: boolean): NodeStatusRowTone => (online ? "success" : "danger")

export function nodeStatusRowsViewModel(nodes: NodeStatus[]): NodeStatusRowViewModel[] {
  return nodes.map((node) => ({
    nodeRef: node.nodeRef,
    label: node.nodeRef,
    statusLabel: onlineLabel(node.online),
    tone: onlineTone(node.online),
    lastHeartbeatAt: node.lastHeartbeatAt ?? "none",
  }))
}

export function providerHealthRowsViewModel(providers: ProviderHealth[]): ProviderHealthRowViewModel[] {
  return providers.map((provider) => ({
    provider: provider.provider,
    label: provider.provider,
    statusLabel: onlineLabel(provider.online),
    tone: onlineTone(provider.online),
  }))
}
