import { describe, expect, test } from "bun:test"
import { expandClaudeMessage } from "../src/node/claude-blocks"

describe("Claude content block expansion (#4951)", () => {
  test("expands a multi-block assistant message into multiple events", () => {
    const events = expandClaudeMessage({
      type: "assistant",
      timestamp: "t1",
      message: {
        content: [
          { type: "text", text: "Reading the code." },
          { type: "thinking", thinking: "Need to inspect parser behavior." },
          { type: "text", text: "I found it." },
        ],
      },
    })

    expect(events).toHaveLength(3)
    expect(events.map((event) => event.phase)).toEqual(["agent_message", "reasoning", "agent_message"])
    expect(events.map((event) => event.messageText)).toEqual(["agent: Reading the code.", "thinking…", "agent: I found it."])
  })

  test("represents text and tool_use blocks from the same message", () => {
    const events = expandClaudeMessage({
      type: "assistant",
      timestamp: "t2",
      message: {
        content: [
          { type: "text", text: "I will run tests." },
          { type: "tool_use", name: "Bash", input: { command: "bun test apps/pylon/tests/claude-blocks.test.ts" } },
        ],
      },
    })

    expect(events).toHaveLength(2)
    expect(events[0]).toMatchObject({ observedAt: "t2", phase: "agent_message", messageText: "agent: I will run tests." })
    expect(events[1]).toMatchObject({ observedAt: "t2", phase: "tool_use", messageText: "Bash: bun test apps/pylon/tests/claude-blocks.test.ts" })
  })

  test("sets messageFull to the full JSON input for tool_use blocks", () => {
    const input = { file_path: "apps/pylon/src/node/claude-blocks.ts", old_string: "short", new_string: "long" }
    const events = expandClaudeMessage({
      type: "assistant",
      message: { content: [{ type: "tool_use", name: "Edit", input }] },
    })

    expect(events).toHaveLength(1)
    expect(events[0]?.messageText).toBe("Edit apps/pylon/src/node/claude-blocks.ts")
    expect(events[0]?.messageFull).toBe(JSON.stringify(input, null, 2))
  })

  test("expands user string content as you text", () => {
    expect(expandClaudeMessage({ type: "user", timestamp: "t3", message: { content: "please fix it\nnow" } })).toEqual([
      { observedAt: "t3", phase: "user", messageText: "you: please fix it now", messageFull: "please fix it\nnow" },
    ])
  })

  test("represents tool_result content with concise result and full content", () => {
    const events = expandClaudeMessage({
      type: "user",
      timestamp: "t4",
      message: { content: [{ type: "tool_result", content: "exit 0\nall good" }] },
    })

    expect(events).toEqual([{ observedAt: "t4", phase: "tool_result", messageText: "result: exit 0 all good", messageFull: "exit 0\nall good" }])
  })

  test("represents array tool_result content with full JSON content", () => {
    const content = [
      { type: "text", text: "first line" },
      { type: "image", source: { type: "base64", media_type: "image/png", data: "abc" } },
      { type: "text", text: "second line" },
    ]
    const events = expandClaudeMessage({
      type: "user",
      message: { content: [{ type: "tool_result", content }] },
    })

    expect(events).toHaveLength(1)
    expect(events[0]).toMatchObject({ phase: "tool_result", messageText: "result: first line second line" })
    expect(events[0]?.messageFull).toBe(JSON.stringify(content, null, 2))
  })

  test("represents thinking blocks as reasoning with full thinking text", () => {
    const events = expandClaudeMessage({
      type: "assistant",
      message: { content: [{ type: "thinking", thinking: "Compare every content block, not just the first salient one." }] },
    })

    expect(events).toEqual([
      {
        observedAt: "",
        phase: "reasoning",
        messageText: "thinking…",
        messageFull: "Compare every content block, not just the first salient one.",
      },
    ])
  })

  test("returns no events for empty content, unsupported blocks, and noise lines", () => {
    expect(expandClaudeMessage({ type: "assistant", message: { content: "   " } })).toEqual([])
    expect(expandClaudeMessage({ type: "assistant", message: { content: [{ type: "text", text: "" }, { type: "unknown" }] } })).toEqual([])
    expect(expandClaudeMessage({ type: "summary", summary: "title" })).toEqual([])
    expect(expandClaudeMessage(null)).toEqual([])
  })
})
