import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { NodeStatusBadge, ProviderStatusList } = await import("../src/node-status")

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ")
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [["class", classes] as const]),
  ]

  return pairs
    .filter(([, value]) => value !== false && value !== undefined && value !== null)
    .map(([name, value]) => (value === true ? ` ${name}` : ` ${name}="${String(value)}"`))
    .join("")
}

const renderHtml = (html: Html): string => {
  if (html === null) return ""
  if (!isVNodeLike(html)) return ""
  const tag = html.sel ?? "node"
  const children = (html.children ?? [])
    .map((child) => (typeof child === "string" ? child : renderHtml(child)))
    .join("")
  const text = html.text ?? ""

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe("Autopilot node status components", () => {
  test("renders node and provider status from inline fixtures", () => {
    const node = {
      nodeRef: "node.fixture.online",
      online: true,
      lastHeartbeatAt: "2026-06-13T12:00:00.000Z",
    }
    const providers = [
      {
        provider: "provider.fixture.offline",
        online: false,
        detailRef: "heartbeat.fixture.missing",
      },
    ]

    const rendered = [
      renderHtml(NodeStatusBadge(node)),
      renderHtml(ProviderStatusList({ providers })),
    ].join("")

    expect(rendered).toContain('data-autopilot-node-ref="node.fixture.online"')
    expect(rendered).toContain('data-autopilot-node-status="online"')
    expect(rendered).toContain("node.fixture.online")
    expect(rendered).toContain("online")
    expect(rendered).toContain('data-autopilot-provider-status-list=""')
    expect(rendered).toContain('data-autopilot-provider="provider.fixture.offline"')
    expect(rendered).toContain('data-autopilot-provider-status="offline"')
    expect(rendered).toContain("provider.fixture.offline")
    expect(rendered).toContain("heartbeat.fixture.missing")
    expect(rendered).toContain("offline")
  })
})
