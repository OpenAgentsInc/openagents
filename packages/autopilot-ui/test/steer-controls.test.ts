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

const { SteerControls } = await import("../src/steer-controls")

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

const actionButtonPattern = (action: string, disabled: boolean): RegExp =>
  new RegExp(`data-autopilot-action="${action}"[^>]*${disabled ? " disabled" : "(?![^>]* disabled)"}`)

const renderControls = (state: "queued" | "running" | "paused" | "completed" | "failed" | "cancelled", readOnly = false): string =>
  renderHtml(
    SteerControls({
      session: {
        sessionRef: `session.fixture.${state}`,
        state,
      },
      readOnly,
    }),
  )

describe("Steer controls", () => {
  test("running enables steer, interrupt, and pause while resume stays disabled", () => {
    const rendered = renderControls("running")

    expect(rendered).toMatch(actionButtonPattern("steer", false))
    expect(rendered).toMatch(actionButtonPattern("interrupt", false))
    expect(rendered).toMatch(actionButtonPattern("pause", false))
    expect(rendered).toMatch(actionButtonPattern("resume", true))
  })

  test("paused enables resume and disables steer, interrupt, and pause", () => {
    const rendered = renderControls("paused")

    expect(rendered).toMatch(actionButtonPattern("steer", true))
    expect(rendered).toMatch(actionButtonPattern("interrupt", true))
    expect(rendered).toMatch(actionButtonPattern("pause", true))
    expect(rendered).toMatch(actionButtonPattern("resume", false))
  })

  test("readOnly disables all actions", () => {
    const rendered = renderControls("running", true)

    expect(rendered).toMatch(actionButtonPattern("steer", true))
    expect(rendered).toMatch(actionButtonPattern("interrupt", true))
    expect(rendered).toMatch(actionButtonPattern("pause", true))
    expect(rendered).toMatch(actionButtonPattern("resume", true))
  })

  test("completed disables all actions", () => {
    const rendered = renderControls("completed")

    expect(rendered).toMatch(actionButtonPattern("steer", true))
    expect(rendered).toMatch(actionButtonPattern("interrupt", true))
    expect(rendered).toMatch(actionButtonPattern("pause", true))
    expect(rendered).toMatch(actionButtonPattern("resume", true))
  })
})
