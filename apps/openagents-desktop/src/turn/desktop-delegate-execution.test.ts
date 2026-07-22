import { describe, expect, test } from "vite-plus/test"

import type { ClaudeLocalStartRequest } from "../claude-local-contract.ts"
import type { ProviderLaneRunInput } from "../provider-lane.ts"
import { executeOrdinaryDelegateTurn } from "./desktop-delegate-execution.ts"

describe("ordinary delegated lane execution", () => {
  test("background lifecycle does not add Full Auto intent or prompt text", async () => {
    let admitted: ClaudeLocalStartRequest | null = null
    let run: ProviderLaneRunInput<null> | null = null
    const result = await executeOrdinaryDelegateTurn({
      lane: {
        admit: (request) => {
          admitted = request
          return { ok: true, model: "fixture-model", context: null }
        },
        runTurn: async (input) => {
          run = input
          return { ok: true, text: "Implemented issue #9159.", totalTokens: 10 }
        },
      },
      requestRef: "request.codex.9159",
      threadRef: "thread.9159",
      message: "Implement issue #9159.",
      history: [{ role: "assistant", text: "Prior context." }],
      emit: () => undefined,
    })

    expect(result).toEqual({ ok: true, text: "Implemented issue #9159." })
    expect(admitted).not.toBeNull()
    expect(run).not.toBeNull()
    const capturedAdmission = admitted as unknown as ClaudeLocalStartRequest
    const capturedRun = run as unknown as ProviderLaneRunInput<null>
    expect(capturedAdmission.fullAuto).toBe(false)
    expect(capturedAdmission.message).toBe("Implement issue #9159.")
    expect(capturedRun.background).toBe(true)
    expect(capturedRun.request.fullAuto).toBe(false)
    expect(capturedRun.message).toBe("Implement issue #9159.")
    expect(capturedRun.message).not.toContain("Full Auto is on for this turn")
  })
})
