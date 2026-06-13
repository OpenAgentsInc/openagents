import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { EarningsSummary } from "../src/earnings"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { EarningsPanel } = await import("../src/earnings")

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

describe("Autopilot earnings panel", () => {
  test("renders balance and recent earnings without actions", () => {
    const summary = {
      balanceSats: 12345,
      entries: [
        {
          ref: "earning.autopilot.fixture0001",
          amountSats: 2100,
          at: "2026-06-13T14:30:00.000Z",
        },
        {
          ref: "earning.autopilot.fixture0002",
          amountSats: 89,
          at: "2026-06-13T15:00:00.000Z",
        },
      ],
    } satisfies EarningsSummary

    const rendered = renderHtml(EarningsPanel(summary))

    expect(rendered).toContain('data-autopilot-earnings-panel=""')
    expect(rendered).toContain("12345 sats")
    expect(rendered).toContain("earning.autopilot.fixture0001")
    expect(rendered).toContain("earning.autopilot.fixture0002")
    expect(rendered).toContain("2100 sats")
    expect(rendered).toContain("89 sats")
    expect(rendered).not.toContain("<button")
  })
})
