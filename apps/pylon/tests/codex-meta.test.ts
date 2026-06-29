import { describe, expect, test } from "bun:test"
import { parseCodexSessionMeta } from "../src/node/codex-meta"

describe("Codex session metadata parsing (#4951)", () => {
  test("extracts metadata from session_meta payload", () => {
    const meta = parseCodexSessionMeta([
      JSON.stringify({
        timestamp: "2026-06-13T12:00:00.000Z",
        type: "session_meta",
        payload: {
          title: "Fix the bridge",
          model: "gpt-5-codex",
          cwd: "/repo",
          startedAt: "2026-06-13T11:59:59.000Z",
        },
      }),
    ])

    expect(meta).toEqual({
      title: "Fix the bridge",
      model: "gpt-5-codex",
      cwd: "/repo",
      startedAt: "2026-06-13T11:59:59.000Z",
    })
  })

  test("uses top-level session_meta fields when payload is absent", () => {
    const meta = parseCodexSessionMeta([
      JSON.stringify({
        timestamp: "2026-06-13T12:00:00.000Z",
        type: "session_meta",
        model: "gpt-5",
        cwd: "/tmp/work",
      }),
    ])

    expect(meta).toEqual({
      title: null,
      model: "gpt-5",
      cwd: "/tmp/work",
      startedAt: "2026-06-13T12:00:00.000Z",
    })
  })

  test("falls back to the first event_msg message for title", () => {
    const meta = parseCodexSessionMeta([
      JSON.stringify({ timestamp: "t1", type: "event_msg", payload: { type: "user_message", message: "Polish Codex\nmetadata parser" } }),
      JSON.stringify({ timestamp: "t2", type: "event_msg", payload: { type: "agent_message", message: "Working" } }),
    ])

    expect(meta.title).toBe("Polish Codex metadata parser")
    expect(meta.startedAt).toBe("t1")
  })

  test("prefers session_meta title over event_msg fallback", () => {
    const meta = parseCodexSessionMeta([
      JSON.stringify({ timestamp: "t1", type: "event_msg", payload: { message: "Fallback title" } }),
      JSON.stringify({ timestamp: "t2", type: "session_meta", payload: { title: "Metadata title" } }),
    ])

    expect(meta.title).toBe("Metadata title")
  })

  test("tolerates invalid JSON and unusual shapes", () => {
    const meta = parseCodexSessionMeta([
      "not json",
      JSON.stringify(null),
      JSON.stringify({ type: "session_meta", payload: "not an object" }),
      JSON.stringify({ type: "event_msg", payload: { message: 42 } }),
    ])

    expect(meta).toEqual({ title: null, model: null, cwd: null, startedAt: null })
  })

  test("supports alternate field spellings", () => {
    const meta = parseCodexSessionMeta([
      JSON.stringify({
        type: "session_meta",
        payload: {
          name: "Alternate",
          model_name: "gpt-alt",
          current_working_directory: "/alt",
          started_at: "2026-06-13T12:30:00.000Z",
        },
      }),
    ])

    expect(meta).toEqual({
      title: "Alternate",
      model: "gpt-alt",
      cwd: "/alt",
      startedAt: "2026-06-13T12:30:00.000Z",
    })
  })
})
