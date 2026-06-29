import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import type { DecisionView } from "../src/decision-actions"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const { DecisionActions, decisionActionState } = await import("../src/decision-actions")

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

const pendingDecision: DecisionView = {
  requestId: "decision.fixture.pending",
  state: "pending",
}

const resolvedDecision: DecisionView = {
  requestId: "decision.fixture.resolved",
  state: "resolved",
  resolvedVerb: "approve",
}

describe("Decision actions", () => {
  test("pending and not readOnly enables all decision verbs", () => {
    const state = decisionActionState(pendingDecision, { readOnly: false })
    const rendered = renderHtml(DecisionActions({ decision: pendingDecision, readOnly: false }))

    expect(state.approve.enabled).toBe(true)
    expect(state.deny.enabled).toBe(true)
    expect(state.answer.enabled).toBe(true)
    expect(rendered).toContain('data-autopilot-decision-action="approve"')
    expect(rendered).toContain('data-autopilot-decision-action="deny"')
    expect(rendered).toContain('data-autopilot-decision-action="answer"')
    expect(rendered).not.toContain('data-autopilot-decision-action="approve" type="button" disabled')
    expect(rendered).not.toContain('data-autopilot-decision-action="deny" type="button" disabled')
    expect(rendered).not.toContain('data-autopilot-decision-action="answer" type="button" disabled')
  })

  test("readOnly disables all decision verbs", () => {
    const state = decisionActionState(pendingDecision, { readOnly: true })
    const rendered = renderHtml(DecisionActions({ decision: pendingDecision, readOnly: true }))

    expect(state.approve.enabled).toBe(false)
    expect(state.deny.enabled).toBe(false)
    expect(state.answer.enabled).toBe(false)
    expect(rendered).toContain('data-autopilot-decision-action="approve" type="button" disabled')
    expect(rendered).toContain('data-autopilot-decision-action="deny" type="button" disabled')
    expect(rendered).toContain('data-autopilot-decision-action="answer" type="button" disabled')
  })

  test("resolved disables all decision verbs and shows the resolved elsewhere note", () => {
    const state = decisionActionState(resolvedDecision, { readOnly: false })
    const rendered = renderHtml(DecisionActions({ decision: resolvedDecision, readOnly: false }))

    expect(state.approve.enabled).toBe(false)
    expect(state.deny.enabled).toBe(false)
    expect(state.answer.enabled).toBe(false)
    expect(rendered).toContain('data-autopilot-decision-action="approve" type="button" disabled')
    expect(rendered).toContain('data-autopilot-decision-action="deny" type="button" disabled')
    expect(rendered).toContain('data-autopilot-decision-action="answer" type="button" disabled')
    expect(rendered).toContain("resolved elsewhere")
    expect(rendered).toContain('data-autopilot-decision-resolved-note="resolved"')
  })
})
