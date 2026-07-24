import { describe, expect, test } from "vite-plus/test"

import {
  applyFullAutoRunControlIntent,
} from "./full-auto-run-control-intent.ts"
import type { FullAutoRunActionContext } from "./full-auto-run-actions.ts"

describe("full-auto-run-control-intent", () => {
  test("rejects unknown runs with typed outcome (never silent)", () => {
    const ctx = {
      capabilities: {
        runRegistry: {
          get: () => null,
        },
      },
      now: () => new Date("2026-07-24T00:00:00.000Z"),
      actor: "mobile",
      callerLabel: "mobile control intent",
    } as unknown as FullAutoRunActionContext

    const outcome = applyFullAutoRunControlIntent(ctx, {
      intentId: "intent.1",
      runRef: "run.missing",
      action: "pause",
    })
    expect(outcome).toEqual({
      intentId: "intent.1",
      status: "rejected",
      rejectionReason: "run_not_found",
    })
  })
})
