import { describe, expect, test } from "bun:test"
import { pendingDecision } from "@openagentsinc/autopilot-control-protocol"
import {
  decisionRequestFixture,
  sessionEventStreamFixture,
  sessionListFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"
import type { Html } from "foldkit/html"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { DecisionCard, EventTimeline, SessionList } = await import("../src/index")

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

describe("Autopilot UI components", () => {
  test("renders a session list from shared protocol fixtures", () => {
    const rendered = renderHtml(SessionList({ sessions: sessionListFixture }))

    expect(rendered).toContain('data-autopilot-session-list=""')
    expect(rendered).toContain("session.pylon.codex_composer.fixture0001")
    expect(rendered).toContain("session.pylon.claude_composer.fixture0002")
    expect(rendered).toContain("codex")
    expect(rendered).toContain("claude_agent")
    expect(rendered).toContain('data-autopilot-state="running"')
    expect(rendered).toContain("progress.fixture.0001")
    expect(rendered).toContain("none")
  })

  test("renders a pending decision card from the shared decision fixture", () => {
    const decision = pendingDecision(decisionRequestFixture)
    const rendered = renderHtml(DecisionCard({ decision }))

    expect(rendered).toContain('data-autopilot-decision-id="decision.fixture.req01"')
    expect(rendered).toContain('data-autopilot-decision-state="pending"')
    expect(rendered).toContain("action.fixture.approve_pr")
    expect(rendered).toContain('data-autopilot-decision-action="approve"')
    expect(rendered).toContain('data-autopilot-decision-action="deny"')
    expect(rendered).toContain('data-autopilot-decision-action="answer"')
    expect(rendered).not.toContain(" disabled>")
  })

  test("renders a session event timeline from shared fixtures", () => {
    const rendered = renderHtml(EventTimeline({ events: sessionEventStreamFixture }))

    expect(rendered).toContain('data-autopilot-event-timeline=""')
    expect(rendered).toContain('data-autopilot-event-id="evt.0003"')
    expect(rendered).toContain("decision_requested")
    expect(rendered).toContain("decision.fixture.req01")
    expect(rendered).toContain("#5")
    expect(rendered).toContain("completed")
  })
})
