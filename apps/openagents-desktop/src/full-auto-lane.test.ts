import { describe, expect, test } from "vite-plus/test"

import { fullAutoLanePolicy, fullAutoPrompt } from "./full-auto-lane.ts"

describe("Full Auto ACP lane policy", () => {
  test.each(["acp:grok-cli", "acp:cursor-agent"])(
    "admits %s only with its provider-specific bounded instruction",
    laneRef => {
      expect(fullAutoLanePolicy(laneRef)).toMatchObject({ autoResolveQuestions: true })
      expect(fullAutoPrompt(laneRef, "continue")).toContain("pinned")
      expect(fullAutoPrompt(laneRef, "continue")).toContain("continue")
    },
  )

  test("keeps unknown lanes fail-closed", () => {
    expect(fullAutoLanePolicy("acp:unknown")).toBeNull()
    expect(fullAutoPrompt("acp:unknown", "continue")).toBe("continue")
  })
})
