import { describe, expect, test } from "vite-plus/test"

import { APPLE_FM_PROMPT_MAX_CHARS, buildOpenAgentsAppleFmPrompt } from "./apple-fm-prompt.ts"

/**
 * AFS-03 (#9081): these are the honesty + history-window behavior contracts for
 * the Apple FM prompt, moved out of the renderer and behind the shared turn
 * kernel's host-owned Apple FM provider. The renderer no longer builds this
 * prompt; the guarantees are unchanged, now asserted on the host module.
 */
describe("buildOpenAgentsAppleFmPrompt (host-owned)", () => {
  test("the frozen renderer-prepared bound is preserved on the host module", () => {
    expect(APPLE_FM_PROMPT_MAX_CHARS).toBe(3900)
  })

  test("keeps the newest turns within the window, always the last message, and cues the assistant", () => {
    const turns = Array.from({ length: 40 }, (_, i) => ({
      role: (i % 2 === 0 ? "user" : "assistant") as "user" | "assistant",
      text: `line ${i} ${"x".repeat(200)}`,
    }))
    const prompt = buildOpenAgentsAppleFmPrompt(turns, 1000)
    expect(prompt.length).toBeLessThanOrEqual(1000)
    expect(prompt).toContain("line 39")
    expect(prompt).not.toContain("line 0 ")
    expect(prompt.endsWith("Assistant:")).toBe(true)
  })

  test("flattens history into User/Assistant lines including the newest turn", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([
      { role: "user", text: "Hi" },
      { role: "assistant", text: "Hello there." },
      { role: "user", text: "Are you there?" },
    ])
    expect(prompt).toContain("Assistant: Hello there.")
    expect(prompt).toContain("User: Are you there?")
  })

  test("stays helpful while forbidding claimed actions or invented facts (owner directive 2026-07-20)", () => {
    const prompt = buildOpenAgentsAppleFmPrompt([
      { role: "user", text: "dispatch a subagent to set a reminder" },
    ])
    // Honesty limit is preserved: no tools, never claim to have acted, no made-up facts.
    expect(prompt).toContain("no tools")
    expect(prompt).toContain("you cannot run commands")
    expect(prompt).toContain("never claim you did, are doing, or will do any such action")
    expect(prompt).toContain("Do not make up facts")
    // Positive-first framing so the small model does not fall into a refusal
    // spiral on benign questions ("what can you do").
    expect(prompt).toContain("helpful, friendly assistant")
    expect(prompt).toContain("always try to be helpful and give a real answer")
    expect(prompt).not.toContain("CANNOT take any action")
  })
})
