import { describe, expect, test } from "bun:test"

import { stableExternalSessionRef, toSessionListEntry } from "./external-sessions.js"
import { buildCodexSession } from "./codex-sessions.js"

const uuid = "11111111-2222-3333-4444-555555555555"

describe("Codex external session projection", () => {
  test("uses a stable mirror ref, aliases the Codex UUID, and loads a readable title", () => {
    const session = buildCodexSession({
      sessionId: `rollout-${uuid}`,
      sourceRef: "/tmp/codex/rollout.jsonl",
      lines: [
        JSON.stringify({ type: "session_meta", payload: { id: uuid } }),
        JSON.stringify({
          timestamp: "2026-07-03T12:00:00.000Z",
          type: "event_msg",
          payload: {
            type: "user_message",
            message: "Fix Codex account failover and explain the real error.",
          },
        }),
      ],
      mtimeMs: 1000,
      nowMs: 2000,
    })

    expect(session.sessionRef).toBe(
      stableExternalSessionRef("session.pylon.codex_external", "/tmp/codex/rollout.jsonl"),
    )
    expect(session.sessionRef.startsWith("codex:")).toBe(false)
    expect(session.aliasSessionRefs).toContain(`codex:rollout-${uuid}`)
    expect(session.aliasSessionRefs).toContain(
      stableExternalSessionRef("session.pylon.codex_composer", uuid),
    )
    expect(session.title).toBe("Fix Codex account failover and explain the real error.")
    expect(toSessionListEntry(session, "2026-07-03T12:00:01.000Z")).toMatchObject({
      title: "Fix Codex account failover and explain the real error.",
    })
  })

  test("falls back to a bounded title without treating rollout names as Codex resume ids", () => {
    const session = buildCodexSession({
      sessionId: "rollout-not-a-uuid",
      lines: [],
      mtimeMs: 1000,
      nowMs: 2000,
    })

    expect(session.sessionRef).toBe(
      stableExternalSessionRef("session.pylon.codex_external", "rollout-not-a-uuid"),
    )
    expect(session.aliasSessionRefs).toEqual(["codex:rollout-not-a-uuid"])
    expect(session.title).toBe("Codex session not-a-uuid")
  })
})
