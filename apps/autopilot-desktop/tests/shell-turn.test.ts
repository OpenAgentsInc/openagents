import { describe, expect, test } from "bun:test"

import {
  SHELL_SYSTEM_PROMPT,
  buildShellTurn,
  resolveShellAgentToken,
} from "../src/bun/shell-turn"

// HUD H5 (#5503): the Bun-host shell-turn that gives the zero-base shell bar a
// REAL model response via the live OpenAgents inference gateway. The agent token
// stays in the Bun host and is only ever placed in the outbound Authorization
// header; the no-token / failure paths are HONEST (never a fabricated answer).

describe("resolveShellAgentToken", () => {
  test("uses OPENAGENTS_AGENT_TOKEN (the desktop's configured token)", () => {
    expect(resolveShellAgentToken({ OPENAGENTS_AGENT_TOKEN: " tok-a " })).toBe(
      "tok-a",
    )
  })

  test("a shell-specific override wins", () => {
    expect(
      resolveShellAgentToken({
        OPENAGENTS_AGENT_TOKEN: "tok-a",
        OPENAGENTS_SHELL_AGENT_TOKEN: "tok-shell",
      }),
    ).toBe("tok-shell")
  })

  test("empty / missing token resolves to null", () => {
    expect(resolveShellAgentToken({})).toBe(null)
    expect(resolveShellAgentToken({ OPENAGENTS_AGENT_TOKEN: "   " })).toBe(null)
  })

  // #5503 live-gateway fix: a normal install sets no env var, so the shell must
  // fall back to the persisted agent credential the desktop already mints.
  test("no env token: falls back to the persisted agent credential", () => {
    expect(resolveShellAgentToken({}, () => " oa_agent_persisted ")).toBe(
      "oa_agent_persisted",
    )
  })

  test("an env token still wins over the persisted credential (no read needed)", () => {
    let read = false
    const r = resolveShellAgentToken({ OPENAGENTS_AGENT_TOKEN: "tok-env" }, () => {
      read = true
      return "oa_agent_persisted"
    })
    expect(r).toBe("tok-env")
    expect(read).toBe(false)
  })

  test("no env token and no persisted credential resolves to null", () => {
    expect(resolveShellAgentToken({}, () => null)).toBe(null)
    expect(resolveShellAgentToken({}, () => "   ")).toBe(null)
  })
})

describe("buildShellTurn — honest no-token + network behaviour", () => {
  test("NO token: no network call, ok:false, plain configure message (never fake)", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "hello",
      env: {},
      agentToken: null,
      fetchFn,
    })
    expect(called).toBe(false)
    expect(r.ok).toBe(false)
    expect(r.text).toContain("OPENAGENTS_AGENT_TOKEN")
    // Honest: it does not pretend to be a model answer.
    expect(r.text).not.toContain("hello")
  })

  test("empty prompt: no network call, honest prompt-me note", async () => {
    let called = false
    const fetchFn = (() => {
      called = true
      return Promise.resolve(new Response("{}"))
    }) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "   ",
      env: {},
      agentToken: "tok",
      fetchFn,
    })
    expect(called).toBe(false)
    expect(r.ok).toBe(false)
  })

  test("with token: POSTs to /api/v1/chat/completions with a Bearer header + Gemini default, returns the Autopilot text", async () => {
    let seenUrl: string | null = null
    let seenAuth: string | null = null
    let seenBody: unknown = null
    const fetchFn = ((url: string, init?: RequestInit) => {
      seenUrl = url
      seenAuth =
        (init?.headers as Record<string, string> | undefined)?.authorization ??
        null
      seenBody = init?.body ? JSON.parse(init.body as string) : null
      return Promise.resolve(
        new Response(
          JSON.stringify({
            choices: [{ message: { role: "assistant", content: "Hi there!" } }],
          }),
          { status: 200 },
        ),
      )
    }) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "say hi",
      env: { OPENAGENTS_COM_BASE_URL: "https://gw.test" },
      agentToken: "sk-secret",
      fetchFn,
    })
    expect(seenUrl).toBe("https://gw.test/api/v1/chat/completions")
    expect(seenAuth).toBe("Bearer sk-secret")
    expect((seenBody as { model?: string }).model).toBe("gemini-3.5-flash")
    expect((seenBody as { stream?: boolean }).stream).toBe(false)
    expect(
      (seenBody as { messages?: Array<{ role: string; content: string }> })
        .messages,
    ).toEqual([
      { role: "system", content: SHELL_SYSTEM_PROMPT },
      { role: "user", content: "say hi" },
    ])
    expect(r.ok).toBe(true)
    expect(r.text).toBe("Hi there!")
    // The raw token NEVER appears in what crosses back to the webview.
    expect(JSON.stringify(r)).not.toContain("sk-secret")
  })

  test("an explicit model override is honoured", async () => {
    let seenModel: string | null = null
    const fetchFn = ((_url: string, init?: RequestInit) => {
      seenModel = init?.body
        ? (JSON.parse(init.body as string).model as string)
        : null
      return Promise.resolve(
        new Response(
          JSON.stringify({ choices: [{ message: { content: "ok" } }] }),
          { status: 200 },
        ),
      )
    }) as unknown as typeof fetch
    await buildShellTurn({
      prompt: "x",
      env: { OPENAGENTS_SHELL_MODEL: "gemini-3.5-flash" },
      agentToken: "tok",
      fetchFn,
    })
    expect(seenModel).toBe("gemini-3.5-flash")
  })

  test("401 maps to a clean auth message (no jargon, no fake answer)", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: "unauthorized" }), {
          status: 401,
        }),
      )) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "x",
      env: {},
      agentToken: "tok",
      fetchFn,
    })
    expect(r.ok).toBe(false)
    expect(r.text.toLowerCase()).toContain("authenticate")
  })

  test("402 maps to a clean out-of-allowance message", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(JSON.stringify({ error: "insufficient_credits" }), {
          status: 402,
        }),
      )) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "x",
      env: {},
      agentToken: "tok",
      fetchFn,
    })
    expect(r.ok).toBe(false)
    expect(r.text.toLowerCase()).toContain("allowance")
  })

  test("a transport failure degrades to a clean retry message, not a crash", async () => {
    const fetchFn = (() =>
      Promise.reject(new Error("network down"))) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "x",
      env: {},
      agentToken: "tok",
      fetchFn,
    })
    expect(r.ok).toBe(false)
    expect(r.text.toLowerCase()).toContain("reach the model")
  })

  test("an empty model response degrades honestly", async () => {
    const fetchFn = (() =>
      Promise.resolve(
        new Response(JSON.stringify({ choices: [] }), { status: 200 }),
      )) as unknown as typeof fetch
    const r = await buildShellTurn({
      prompt: "x",
      env: {},
      agentToken: "tok",
      fetchFn,
    })
    expect(r.ok).toBe(false)
    expect(r.text.toLowerCase()).toContain("empty")
  })
})
