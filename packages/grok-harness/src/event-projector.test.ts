import { describe, expect, test } from "bun:test"

import { createGrokAcpEventProjector } from "./event-projector.ts"

describe("createGrokAcpEventProjector", () => {
  test("streams message_start, deltas, then done", () => {
    const p = createGrokAcpEventProjector({
      threadId: "t1",
      turnId: "turn1",
      messageId: "m1",
    })

    const e1 = p.onUpdate({
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "hel" },
    })
    expect(e1.map((e) => e.type)).toEqual(["message_start", "message_delta"])

    const e2 = p.onUpdate({
      sessionUpdate: "agent_message_chunk",
      content: { type: "text", text: "lo" },
    })
    expect(e2.map((e) => e.type)).toEqual(["message_delta"])
    expect(p.text()).toBe("hello")

    const fin = p.finish()
    expect(fin).toEqual([
      { type: "message_done", turnId: "turn1", messageId: "m1" },
    ])
  })
})
