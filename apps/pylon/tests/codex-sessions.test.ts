import { mkdir, mkdtemp, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { stableExternalSessionRef } from "../src/node/external-sessions"
import { buildCodexSession, normalizeCodexLine, scanCodexSessions } from "../src/node/codex-sessions"

const ev = (o: any) => normalizeCodexLine(o)

describe("external Codex session normalization (#4951)", () => {
  test("normalizes event_msg agent text concisely", () => {
    expect(ev({ timestamp: "t", type: "event_msg", payload: { type: "agent_message", message: "Working on it.\nNow." } }))
      .toMatchObject({ observedAt: "t", phase: "agent_message", messageText: "agent: Working on it. Now." })
    expect(ev({ timestamp: "t", type: "event_msg", payload: { type: "token_count", info: {} } })).toBeNull()
  })

  test("normalizes response_item tool calls and outputs", () => {
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "function_call", name: "exec_command", arguments: JSON.stringify({ cmd: "rg foo" }) } }))
      .toMatchObject({ phase: "tool_use", messageText: "exec_command: rg foo" })
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "function_call_output", output: "exit 0\nok" } }))
      .toMatchObject({ phase: "tool_result", messageText: "result: exit 0 ok" })
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "reasoning", summary: [{ text: "checking files" }] } }))
      .toMatchObject({ phase: "reasoning", messageText: "thinking: checking files" })
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "reasoning", text: "checking live logs" } }))
      .toMatchObject({
        phase: "reasoning",
        messageText: "thinking: checking live logs",
        messageFull: "thinking: checking live logs",
      })
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "reasoning", summary: [] } }))
      .toMatchObject({ phase: "reasoning", messageText: "thinking…" })
    expect(ev({ timestamp: "t", type: "response_item", payload: { type: "reasoning", summary: [], encrypted_content: "sealed" } }))
      .toBeNull()
  })

  test("normalizes Codex task status and thinking token counts", () => {
    expect(ev({ timestamp: "t1", type: "event_msg", payload: { type: "task_started" } }))
      .toMatchObject({ phase: "started", messageText: "task started" })
    expect(ev({
      timestamp: "t2",
      type: "event_msg",
      payload: {
        type: "token_count",
        info: {
          last_token_usage: {
            output_tokens: 12,
            reasoning_output_tokens: 5,
          },
        },
      },
    })).toMatchObject({ phase: "reasoning", messageText: "thinking tokens: 5; output tokens: 12" })
    expect(ev({ timestamp: "t3", type: "event_msg", payload: { type: "task_complete" } }))
      .toMatchObject({ phase: "completed", messageText: "task complete" })
  })

  test("builds a Codex external session shape with latest activity", () => {
    const lines = [
      JSON.stringify({ timestamp: "t0", type: "session_meta", payload: { id: "019ee2ec-b15d-7442-8bfe-2fd3f63d57ac" } }),
      JSON.stringify({ timestamp: "t1", type: "event_msg", payload: { type: "agent_message", message: "Reading files" } }),
      JSON.stringify({ timestamp: "t2", type: "response_item", payload: { type: "function_call", name: "apply_patch", input: "*** Begin Patch\n" } }),
    ]
    const s = buildCodexSession({ sessionId: "abc123", lines, mtimeMs: 1000, nowMs: 1000, parentRef: "codex:parent" })
    expect(s.sessionRef).toBe(stableExternalSessionRef("session.pylon.codex_external", "abc123"))
    expect(s.aliasSessionRefs).toEqual([
      "codex:abc123",
      "session.pylon.codex_composer.be4d2b8c1eb3512e70bf59be",
    ])
    expect(s.agentKind).toBe("codex")
    expect(s.parentRef).toBe("codex:parent")
    expect(s.title).toBe("Codex session abc123")
    expect(s.latestActivity).toBe("apply_patch: *** Begin Patch")
    expect(s.events).toHaveLength(2)
  })

  test("uses running for fresh mtimes and idle for stale mtimes", () => {
    expect(buildCodexSession({ sessionId: "fresh", lines: [], mtimeMs: 10_000, nowMs: 99_999 }).state).toBe("running")
    expect(buildCodexSession({ sessionId: "stale", lines: [], mtimeMs: 10_000, nowMs: 100_000 }).state).toBe("idle")
  })

  test("derives the stable composer alias from rollout filenames when session_meta is outside the tail", () => {
    const s = buildCodexSession({
      sessionId: "rollout-2026-06-19T21-47-03-019ee2ec-b15d-7442-8bfe-2fd3f63d57ac",
      lines: [],
      mtimeMs: 1000,
      nowMs: 1000,
    })
    expect(s.aliasSessionRefs).toEqual([
      "codex:rollout-2026-06-19T21-47-03-019ee2ec-b15d-7442-8bfe-2fd3f63d57ac",
      "session.pylon.codex_composer.be4d2b8c1eb3512e70bf59be",
    ])
  })

  test("scans recent rollout files under YYYY/MM/DD", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-codex-sessions-"))
    const day = join(root, "2026", "06", "13")
    await mkdir(day, { recursive: true })
    await writeFile(
      join(day, "rollout-2026-06-13T12-00-00-test.jsonl"),
      `${JSON.stringify({ timestamp: "t", type: "event_msg", payload: { type: "agent_message", message: "hello" } })}\n`,
    )

    const sessions = scanCodexSessions({ sessionsRoot: root, nowMs: Date.now(), maxAgeMs: 60_000, maxSessions: 10 })
    expect(sessions).toHaveLength(1)
    expect(sessions[0]?.sessionRef).toBe(
      stableExternalSessionRef(
        "session.pylon.codex_external",
        join(day, "rollout-2026-06-13T12-00-00-test.jsonl"),
      ),
    )
    expect(sessions[0]?.aliasSessionRefs).toContain("codex:rollout-2026-06-13T12-00-00-test")
    expect(sessions[0]?.latestActivity).toBe("agent: hello")
  })
})
