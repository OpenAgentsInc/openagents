import { classAttrs } from "@openagentsinc/ui/class-foldkit"
import type { Html } from "foldkit/html"
import { html } from "foldkit/html"
import { domainStyles } from "./domain-styles.js"
import type { AutopilotUiMessage } from "./view.js"
import { statusChip } from "./view.js"

export type NodeStatus = Readonly<{
  nodeRef: string
  online: boolean
  lastHeartbeatAt?: string
}>

export type ProviderHealth = Readonly<{
  provider: string
  online: boolean
  detailRef?: string
}>

const h = html<AutopilotUiMessage>()

const onlineLabel = (online: boolean): "online" | "offline" => (online ? "online" : "offline")

export const NodeStatusBadge = (node: NodeStatus): Html =>
  h.div(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.inlinePanel),
      h.DataAttribute("autopilot-node-ref", node.nodeRef),
    ],
    [
      h.code(classAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [node.nodeRef]),
      statusChip({
        label: onlineLabel(node.online),
        tone: node.online ? "success" : "danger",
        attrs: [h.DataAttribute("autopilot-node-status", onlineLabel(node.online))],
      }),
      ...(node.lastHeartbeatAt === undefined
        ? []
        : [
            h.time(classAttrs<AutopilotUiMessage>(domainStyles.muted), [
              node.lastHeartbeatAt,
            ]),
          ]),
    ],
  )

export const ProviderStatusList = (input: { providers: ReadonlyArray<ProviderHealth> }): Html =>
  h.ul(
    [
      ...classAttrs<AutopilotUiMessage>(domainStyles.list),
      h.DataAttribute("autopilot-provider-status-list", ""),
    ],
    input.providers.map((provider) =>
      h.li(
        [
          ...classAttrs<AutopilotUiMessage>(domainStyles.providerRow),
          h.DataAttribute("autopilot-provider", provider.provider),
        ],
        [
          h.code(classAttrs<AutopilotUiMessage>(domainStyles.codePrimary), [
            provider.provider,
          ]),
          statusChip({
            label: onlineLabel(provider.online),
            tone: provider.online ? "success" : "danger",
            attrs: [h.DataAttribute("autopilot-provider-status", onlineLabel(provider.online))],
          }),
          h.code(classAttrs<AutopilotUiMessage>(domainStyles.codeMuted), [
            provider.detailRef ?? "none",
          ]),
        ],
      ),
    ),
  )
