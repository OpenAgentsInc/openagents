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
