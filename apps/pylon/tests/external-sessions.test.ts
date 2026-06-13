import { describe, expect, test } from "bun:test"
import { normalizeClaudeLine, buildClaudeSession } from "../src/node/external-sessions"

const ev = (o: any) => normalizeClaudeLine(o)

describe("external Claude session normalization (#4951)", () => {
  test("normalizes the salient block of each event concisely", () => {
    expect(ev({ type: "user", timestamp: "t", message: { role: "user", content: "fix the readme" } }))
      .toMatchObject({ phase: "user", messageText: "you: fix the readme" })
    expect(ev({ type: "assistant", message: { content: [{ type: "text", text: "On it." }] } }))
      .toMatchObject({ phase: "agent_message", messageText: "agent: On it." })
    expect(ev({ type: "assistant", message: { content: [{ type: "tool_use", name: "Bash", input: { command: "ls -la" } }] } }))
      .toMatchObject({ phase: "tool_use", messageText: "Bash: ls -la" })
    expect(ev({ type: "assistant", message: { content: [{ type: "tool_use", name: "Edit", input: { file_path: "src/x.ts" } }] } }))
      .toMatchObject({ messageText: "Edit src/x.ts" })
    expect(ev({ type: "assistant", message: { content: [{ type: "tool_use", name: "Task", input: { description: "audit auth" } }] } }))
      .toMatchObject({ messageText: "→ sub-agent: audit auth" })
    expect(ev({ type: "user", message: { content: [{ type: "tool_result", content: "exit 0\nok" }] } }))
      .toMatchObject({ phase: "tool_result" })
    expect(ev({ type: "ai-title", aiTitle: "Fix the docs" })).toEqual({ title: "Fix the docs" })
    expect(ev({ type: "file-history-snapshot", snapshot: {} })).toBeNull()
  })

  test("builds a session with title, tail, latestActivity, and running state", () => {
    const lines = [
      JSON.stringify({ type: "ai-title", aiTitle: "My Task" }),
      JSON.stringify({ type: "user", message: { role: "user", content: "do the thing" } }),
      JSON.stringify({ type: "assistant", message: { content: [{ type: "tool_use", name: "Bash", input: { command: "rg foo" } }] } }),
    ]
    const s = buildClaudeSession({ sessionId: "abc123", lines, mtimeMs: 1000, nowMs: 1000, parentRef: null })
    expect(s.sessionRef).toBe("claude:abc123")
    expect(s.agentKind).toBe("claude")
    expect(s.title).toBe("My Task")
    expect(s.latestActivity).toBe("Bash: rg foo")
    expect(s.state).toBe("running")
    expect(s.events.length).toBe(2)
  })

  test("a sub-agent session carries its parentRef and goes idle when stale", () => {
    const s = buildClaudeSession({ sessionId: "kid", lines: [], mtimeMs: 0, nowMs: 10_000_000, parentRef: "claude:parent" })
    expect(s.parentRef).toBe("claude:parent")
    expect(s.state).toBe("idle")
  })
})
