import { pendingDecision } from "@openagentsinc/autopilot-control-protocol"
import {
  decisionRequestFixture,
  sessionEventStreamFixture,
} from "@openagentsinc/autopilot-control-protocol/fixtures"
import { describe, expect, test } from "bun:test"
import type { Html } from "foldkit/html"
import {
  ArtifactList,
  AssignmentList,
  CloudQuotaPanel,
  DecisionActions,
  DecisionCard,
  EarningsPanel,
  EventTimeline,
  NodeStatusBadge,
  ProviderStatusList,
  ReceiptList,
  SessionActions,
  SessionDetail,
  VerifyStatus,
} from "../src/index"

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

describe("Autopilot UI StyleX migration coverage", () => {
  test("selected domain components route through shared StyleX fallback classes", () => {
    const session = {
      sessionRef: "session.stylex.fixture",
      adapter: "codex",
      state: "running",
    }
    const decision = pendingDecision(decisionRequestFixture)
    const rendered = [
      NodeStatusBadge({ nodeRef: "node.stylex.fixture", online: true }),
      ProviderStatusList({
        providers: [{ provider: "provider.stylex.fixture", online: false }],
      }),
      CloudQuotaPanel({
        creditBalance: 42,
        compute: { usedRef: "quota.stylex.fixture", meterLabel: "compute" },
      }),
      EarningsPanel({
        balanceSats: 21,
        entries: [{ ref: "earning.stylex.fixture", amountSats: 21, at: "2026-06-22T00:00:00Z" }],
      }),
      DecisionCard({ decision }),
      DecisionActions({
        decision: { requestId: "decision.stylex.fixture", state: "pending" },
        readOnly: false,
      }),
      SessionDetail(session, { events: [1] }),
      SessionActions({ session, readOnly: false }),
      AssignmentList({
        assignments: [{ ref: "assignment.stylex.fixture", state: "available", progress: 50 }],
      }),
      ArtifactList({
        artifacts: [{ name: "artifact.txt", digestRef: "digest.stylex.artifact.fixture" }],
      }),
      ReceiptList({
        receipts: [{ kind: "verify", digestRef: "digest.stylex.receipt.fixture", status: "ok" }],
      }),
      VerifyStatus({
        command: ["bun", "test"],
        status: "pending",
        requiredArtifacts: [{ ref: "artifact.stylex.required", present: true }],
      }),
      EventTimeline({ events: sessionEventStreamFixture.slice(0, 1) }),
    ].map(renderHtml).join("")

    expect(rendered).toContain("oa-autopilot-domain-panel")
    expect(rendered).toContain("oa-autopilot-domain-inline-panel")
    expect(rendered).toContain("oa-autopilot-domain-provider-row")
    expect(rendered).toContain("oa-autopilot-domain-earnings-row")
    expect(rendered).toContain("oa-autopilot-domain-assignment-row")
    expect(rendered).toContain("oa-autopilot-domain-artifact-row")
    expect(rendered).toContain("oa-autopilot-domain-receipt-row")
    expect(rendered).toContain("oa-autopilot-domain-two-column-row")
    expect(rendered).toContain("oa-autopilot-domain-event-row")
    expect(rendered).toContain("oa-autopilot-domain-action-button")
    expect(rendered).toContain("oa-autopilot-domain-chip")
    expect(rendered).toContain('data-autopilot-decision-id="decision.fixture.req01"')
    expect(rendered).toContain('data-autopilot-session-ref="session.stylex.fixture"')
    expect(rendered).toContain('data-autopilot-verify-command=""')
  })
})
