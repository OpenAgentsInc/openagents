import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import { createSessionNotifier } from "../src/bun/notifier.ts"
import { notificationsHtml } from "../src/shared/notification-html.ts"

function session(input: Partial<SessionSummary> & { sessionRef: string; state: SessionSummary["state"] }): SessionSummary {
  return {
    adapter: "codex",
    accountRefHash: null,
    updatedAt: "2026-06-13T00:00:00.000Z",
    ...input,
  } as SessionSummary
}

describe("desktop session notifier (CL-30)", () => {
  test("raises an OS notification only when a session newly becomes notify-worthy", () => {
    const raised: Array<{ title: string; body: string; priority: string }> = []
    const notifier = createSessionNotifier({
      raise: (n) => raised.push(n),
      now: () => "2026-06-13T00:00:00.000Z",
    })

    // First poll: a failed session and a running one. Only failed notifies.
    const first = notifier.ingest([
      session({ sessionRef: "sess-a", state: "failed", latestActivity: "verify failed" }),
      session({ sessionRef: "sess-b", state: "running", latestActivity: "working" }),
    ])
    expect(raised).toHaveLength(1)
    expect(raised[0].priority).toBe("high")
    expect(raised[0].body).toContain("sess-a")
    expect(first.unread).toBe(1)
    expect(first.hasHigh).toBe(true)

    // Same sessions again: nothing new, no duplicate notification.
    notifier.ingest([
      session({ sessionRef: "sess-a", state: "failed", latestActivity: "verify failed" }),
      session({ sessionRef: "sess-b", state: "running", latestActivity: "working" }),
    ])
    expect(raised).toHaveLength(1)

    // sess-b transitions to completed: a new, normal-priority notification.
    const third = notifier.ingest([
      session({ sessionRef: "sess-a", state: "failed", latestActivity: "verify failed" }),
      session({ sessionRef: "sess-b", state: "completed", latestActivity: "done" }),
    ])
    expect(raised).toHaveLength(2)
    expect(raised[1].priority).toBe("normal")
    expect(third.unread).toBe(2)
  })

  test("never raises for non-notify-worthy states", () => {
    const raised: unknown[] = []
    const notifier = createSessionNotifier({ raise: () => raised.push(true) })
    const view = notifier.ingest([
      session({ sessionRef: "sess-q", state: "queued" }),
      session({ sessionRef: "sess-r", state: "running" }),
      session({ sessionRef: "sess-c", state: "cancelled" }),
    ])
    expect(raised).toHaveLength(0)
    expect(view.unread).toBe(0)
    expect(view.items).toHaveLength(0)
  })

  test("a throwing OS-notification raise does not break the feed", () => {
    const notifier = createSessionNotifier({
      raise: () => {
        throw new Error("osascript unavailable")
      },
    })
    const view = notifier.ingest([session({ sessionRef: "sess-x", state: "failed" })])
    expect(view.unread).toBe(1)
  })

  test("notification panel HTML reflects the center view and escapes content", () => {
    const notifier = createSessionNotifier({ raise: () => {} })
    const view = notifier.ingest([
      session({ sessionRef: "sess-<x>", state: "failed", latestActivity: "boom & crash" }),
    ])
    const html = notificationsHtml(view)
    expect(html).toContain("Notifications · 1")
    expect(html).toContain("notif-has-high")
    expect(html).not.toContain("<x>")
    expect(html).toContain("&amp;")
  })

  test("empty view renders the empty state", () => {
    const notifier = createSessionNotifier({ raise: () => {} })
    expect(notificationsHtml(notifier.view())).toContain("No notifications yet.")
  })
})
