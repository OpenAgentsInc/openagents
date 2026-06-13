import { describe, expect, test } from "bun:test"
import { CONTROL_SCHEMA_TAG } from "@openagentsinc/autopilot-control-protocol"
import { sessionListFixture } from "@openagentsinc/autopilot-control-protocol/fixtures"
import { renderSessions, sessionsHtml } from "../src/ui/session-render"
import type { NodeStateMessage } from "../src/shared/rpc"

describe("session rendering", () => {
  test("renders fixture sessions and the connected status line", () => {
    const message: NodeStateMessage = {
      ok: true,
      schema: CONTROL_SCHEMA_TAG,
      sessions: sessionListFixture,
    }

    const html = sessionsHtml(message)

    expect(html).toContain("connected · 2 sessions")
    for (const session of sessionListFixture) {
      expect(html).toContain(session.sessionRef)
      expect(html).toContain(session.adapter)
      expect(html).toContain(session.state)
    }
  })

  test("renders a live session-detail timeline from recentEvents (CL-5)", () => {
    const ref = sessionListFixture[0].sessionRef
    const html = sessionsHtml({
      ok: true,
      schema: CONTROL_SCHEMA_TAG,
      sessions: sessionListFixture,
      events: {
        [ref]: [
          { eventIndex: 0, phase: "started", state: "running", observedAt: "2026-06-13T17:00:00.000Z", detail: "" },
          { eventIndex: 1, phase: "composer_event", state: "running", observedAt: "2026-06-13T17:00:05.000Z", detail: "agent: hello world" },
        ],
      },
    })

    expect(html).toContain("session-timeline")
    expect(html).toContain("agent: hello world")
    expect(html).toContain("17:00:05")
  })

  test("renders node-status breakdown, verify line, and accounts (CL-18/19/20)", () => {
    const html = sessionsHtml({
      ok: true,
      schema: CONTROL_SCHEMA_TAG,
      sessions: sessionListFixture,
      accounts: [
        { provider: "codex", homeState: "present", ready: true },
        { provider: "claude_agent", homeState: "missing", ready: false },
      ],
    })

    // CL-20 node-status breakdown (counts by state) + CL-18 accounts panel
    expect(html).toContain("Accounts")
    expect(html).toContain("codex · present · ready")
    expect(html).toContain("claude_agent · missing · blocked")
  })

  test("renders offline empty state when no sessions are present", () => {
    const html = sessionsHtml({
      ok: false,
      schema: CONTROL_SCHEMA_TAG,
      sessions: [],
    })

    expect(html).toContain("offline · 0 sessions")
    expect(html).toContain("No sessions yet.")
  })

  test("renders into a container", () => {
    const container = { innerHTML: "" } as HTMLElement

    renderSessions(container, {
      ok: true,
      schema: CONTROL_SCHEMA_TAG,
      sessions: sessionListFixture,
    })

    expect(container.innerHTML).toContain("connected · 2 sessions")
    expect(container.innerHTML).toContain(sessionListFixture[0].sessionRef)
  })
})
