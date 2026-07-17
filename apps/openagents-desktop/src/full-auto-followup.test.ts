import { describe, expect, test } from "vite-plus/test"

import { makeFullAutoFollowupHandoff } from "./full-auto-followup.ts"

const promoted = {
  kind: "followup_promoted" as const,
  queueRef: "queue-1",
  intentRef: "intent-1",
  clientUserMessageId: "user-1",
  message: "Do this next",
}

describe("Full Auto background follow-up handoff", () => {
  test("hands a background promotion to exactly one next dispatch", () => {
    const handoff = makeFullAutoFollowupHandoff()
    handoff.observe({ threadRef: "thread-1", background: true, fullAuto: true, event: promoted })
    expect(handoff.take("thread-1")).toEqual({
      queueRef: "queue-1",
      clientUserMessageId: "user-1",
      message: "Do this next",
    })
    expect(handoff.take("thread-1")).toBeNull()
  })

  test("leaves foreground and ordinary-turn promotion ownership unchanged", () => {
    const handoff = makeFullAutoFollowupHandoff()
    handoff.observe({ threadRef: "thread-1", background: false, fullAuto: true, event: promoted })
    handoff.observe({ threadRef: "thread-1", background: true, fullAuto: false, event: promoted })
    expect(handoff.take("thread-1")).toBeNull()
  })
})
