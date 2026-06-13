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

const { CloudQuotaPanel } = await import("../src/cloud-quota")

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

describe("Autopilot cloud quota component", () => {
  test("renders credit balance, compute meter, and inactive failover chip", () => {
    const rendered = renderHtml(
      CloudQuotaPanel({
        creditBalance: 2750,
        compute: {
          usedRef: "usage.cloud.fixture.0001",
          meterLabel: "compute: ref-only",
        },
      }),
    )

    expect(rendered).toContain('data-autopilot-cloud-quota-panel=""')
    expect(rendered).toContain('data-autopilot-cloud-credit-balance="2750"')
    expect(rendered).toContain("2750 credits")
    expect(rendered).toContain("compute: ref-only")
    expect(rendered).toContain('data-autopilot-cloud-compute-ref="usage.cloud.fixture.0001"')
    expect(rendered).toContain("usage.cloud.fixture.0001")
    expect(rendered).toContain('data-autopilot-cloud-failover="inactive"')
    expect(rendered).toContain("failover: inactive")
  })

  test("renders active failover chip with reason ref", () => {
    const rendered = renderHtml(
      CloudQuotaPanel({
        creditBalance: 140,
        compute: {
          usedRef: "usage.cloud.fixture.0002",
          meterLabel: "compute: metered",
        },
        failover: {
          active: true,
          reasonRef: "failover.reason.fixture.quota",
        },
      }),
    )

    expect(rendered).toContain("140 credits")
    expect(rendered).toContain("compute: metered")
    expect(rendered).toContain("usage.cloud.fixture.0002")
    expect(rendered).toContain('data-autopilot-cloud-failover="active"')
    expect(rendered).toContain("failover: failover.reason.fixture.quota")
  })
})
