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

const { SessionActions, SessionDetail } = await import("../src/session-actions")

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

const runningSession = {
  sessionRef: "session.fixture.running",
  adapter: "codex",
  state: "running",
}

const completedSession = {
  sessionRef: "session.fixture.completed",
  adapter: "claude_agent",
  state: "completed",
}

describe("Session actions", () => {
  test("enables cancel for a running session", () => {
    const rendered = renderHtml(SessionActions({ session: runningSession, readOnly: false }))

    expect(rendered).toContain('data-autopilot-action="cancel"')
    expect(rendered).not.toContain('data-autopilot-action="cancel" type="button" disabled')
  })

  test("disables cancel for a completed session", () => {
    const rendered = renderHtml(SessionActions({ session: completedSession, readOnly: false }))

    expect(rendered).toContain('data-autopilot-action="cancel"')
    expect(rendered).toContain('data-autopilot-action="cancel" type="button" disabled')
  })

  test("readOnly disables all actions", () => {
    const rendered = renderHtml(SessionActions({ session: runningSession, readOnly: true }))

    expect(rendered).toContain('data-autopilot-action="spawn" type="button" disabled')
    expect(rendered).toContain('data-autopilot-action="cancel" type="button" disabled')
  })

  test("session detail shows the ref and state", () => {
    const rendered = renderHtml(SessionDetail(runningSession, { events: [{ eventId: "evt.fixture" }] }))

    expect(rendered).toContain("session.fixture.running")
    expect(rendered).toContain('data-autopilot-state="running"')
    expect(rendered).toContain("running")
  })
})
