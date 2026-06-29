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

const {
  DecisionCard,
  EventTimeline,
  PublicActivityStrip,
  SessionList,
  publicActivityHrefForRef,
} = await import("../src/index")

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

  test("renders public activity strip with source lag warnings and public refs", () => {
    const rendered = renderHtml(
      PublicActivityStrip({
        sourceUrl: "https://openagents.test/api/public/activity-timeline?limit=20",
        envelope: {
          generatedAt: "2026-06-18T00:00:00.000Z",
          nextCursor: null,
          sourceLag: [
            {
              sourceKind: "forum",
              status: "stale",
              latestSourceEventAt: "2026-06-17T23:59:00.000Z",
              observedAt: "2026-06-18T00:00:00.000Z",
              lagSeconds: 60,
              maxStalenessSeconds: 30,
              sourceRefs: ["forum.activity.public.1"],
              blockerRefs: ["blocker.public.activity_timeline.source_lag.forum"],
              caveatRefs: ["caveat.public.activity_timeline.source_lag"],
            },
          ],
          events: [
            {
              eventRef: "activity.training.settlement.1",
              cursor:
                "2026-06-18T00:00:01.000Z:settlement_receipt:activity.training.settlement.1",
              ts: "2026-06-18T00:00:01.000Z",
              kind: "real_bitcoin_moved",
              sourceKind: "settlement_receipt",
              runRef: "run.cs336.a1.demo",
              refs: ["receipt.public.real.1"],
              sourceRefs: ["receipt.public.real.1"],
              blockerRefs: [],
              caveatRefs: [],
              amountSats: 2100,
              realBitcoinMoved: true,
              state: "settled",
              text: "Receipt-backed real Bitcoin movement confirmed.",
            },
          ],
        },
      }),
    )

    expect(rendered).toContain('data-public-activity-strip=""')
    expect(rendered).toContain('data-public-activity-category="settle"')
    expect(rendered).toContain("Receipt-backed real Bitcoin movement confirmed.")
    expect(rendered).toContain("2,100 sats")
    expect(rendered).toContain('data-public-activity-source-lag="forum"')
    expect(rendered).toContain(
      'href="/api/public/nexus-pylon/receipts/receipt.public.real.1"',
    )
    expect(rendered).toContain("blocker.public.activity_timeline.source_lag.forum")
  })

  test("derives bounded public activity ref hrefs", () => {
    expect(
      publicActivityHrefForRef("route:/api/public/activity-timeline", {}),
    ).toBe("/api/public/activity-timeline")
    expect(
      publicActivityHrefForRef("trace.public.demo", {
        runRef: "run.cs336.a1.demo",
      }),
    ).toBe(
      "/api/public/training/runs/run.cs336.a1.demo?focusRef=trace.public.demo",
    )
    expect(publicActivityHrefForRef("/Users/private/path", {})).toBe(null)
  })
})
