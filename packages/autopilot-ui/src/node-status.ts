import type { Attribute, Html } from "foldkit/html"
import { html } from "foldkit/html"
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

const className = (value: string): Attribute<AutopilotUiMessage> => h.Class(value)

const onlineLabel = (online: boolean): "online" | "offline" => (online ? "online" : "offline")

export const NodeStatusBadge = (node: NodeStatus): Html =>
  h.div(
    [
      className(
        "inline-flex min-w-0 items-center gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] px-3 py-2 text-[var(--text,#d7d8e5)]",
      ),
      h.DataAttribute("autopilot-node-ref", node.nodeRef),
    ],
    [
      h.code([className("min-w-0 truncate font-mono text-xs text-[var(--primary,#fff)]")], [
        node.nodeRef,
      ]),
      statusChip({
        label: onlineLabel(node.online),
        tone: node.online ? "success" : "danger",
        attrs: [h.DataAttribute("autopilot-node-status", onlineLabel(node.online))],
      }),
      ...(node.lastHeartbeatAt === undefined
        ? []
        : [
            h.time([className("font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
              node.lastHeartbeatAt,
            ]),
          ]),
    ],
  )

export const ProviderStatusList = (input: { providers: ReadonlyArray<ProviderHealth> }): Html =>
  h.ul(
    [className("grid gap-2"), h.DataAttribute("autopilot-provider-status-list", "")],
    input.providers.map((provider) =>
      h.li(
        [
          className(
            "grid gap-2 border border-[var(--outline,#525458)] bg-[var(--bg-secondary,#151515)] p-3 text-[var(--text,#d7d8e5)] sm:grid-cols-[minmax(0,1fr)_7rem_minmax(0,1fr)] sm:items-center",
          ),
          h.DataAttribute("autopilot-provider", provider.provider),
        ],
        [
          h.code([className("min-w-0 truncate font-mono text-sm text-[var(--primary,#fff)]")], [
            provider.provider,
          ]),
          statusChip({
            label: onlineLabel(provider.online),
            tone: provider.online ? "success" : "danger",
            attrs: [h.DataAttribute("autopilot-provider-status", onlineLabel(provider.online))],
          }),
          h.code([className("min-w-0 truncate font-mono text-xs text-[var(--text-secondary,#8a8c93)]")], [
            provider.detailRef ?? "none",
          ]),
        ],
      ),
    ),
  )
