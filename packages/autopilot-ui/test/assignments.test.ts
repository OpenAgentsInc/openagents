import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { Assignment } from "../src/assignments"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { AssignmentList } = await import("../src/assignments")

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    style?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === "object" && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const style = Object.entries(node.data?.style ?? {})
    .map(([name, value]) => `${name}: ${String(value)};`)
    .join(" ")
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(" ")
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(style.length === 0 ? [] : [["style", style] as const]),
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

const assignments = [
  {
    ref: "assignment.fixture.available",
    state: "available",
    progress: 10,
  },
  {
    ref: "assignment.fixture.in_progress",
    state: "in_progress",
    progress: 65,
  },
] satisfies ReadonlyArray<Assignment>

describe("Assignment list", () => {
  test("renders refs, state chips, progress, and available accept action", () => {
    const rendered = renderHtml(AssignmentList({ assignments, readOnly: false }))

    expect(rendered).toContain("assignment.fixture.available")
    expect(rendered).toContain("assignment.fixture.in_progress")
    expect(rendered).toContain('data-autopilot-assignment-state="available"')
    expect(rendered).toContain('data-autopilot-assignment-state="in_progress"')
    expect(rendered).toContain('data-autopilot-assignment-progress="10"')
    expect(rendered).toContain('data-autopilot-assignment-progress="65"')
    expect(rendered).toContain("10%")
    expect(rendered).toContain("65%")
    expect(rendered).toContain('data-autopilot-assignment-action="accept"')
    expect(rendered.match(/data-autopilot-assignment-action="accept"/g)).toHaveLength(1)
  })

  test("omits accept action when readOnly", () => {
    const rendered = renderHtml(AssignmentList({ assignments, readOnly: true }))

    expect(rendered).not.toContain('data-autopilot-assignment-action="accept"')
  })
})
