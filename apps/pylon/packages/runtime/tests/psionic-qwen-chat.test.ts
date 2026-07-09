import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  makePsionicQwenClient,
  PsionicQwenClientError,
} from "../src/backends/psionic-qwen/client.js"

describe("archived Psionic Qwen client", () => {
  test("never contacts the retired backend and fails completion with the archive receipt", async () => {
    let fetchCalls = 0
    const client = await Effect.runPromise(makePsionicQwenClient({
      explicitBaseUrl: "http://127.0.0.1:18080",
      fetch: async () => {
        fetchCalls += 1
        return Response.json({ ready: true })
      },
    }))

    const readiness = await Effect.runPromise(client.doctor())
    expect(readiness).toMatchObject({
      ready: false,
      status: "archived",
      blockerRefs: ["blocker.psionic_qwen35.archived_to_backroom"],
      receipt: {
        archivedTo: "backroom.openagents_prune_20260708_tassadar_psionic",
      },
    })
    expect(readiness.profile.baseUrlSource).toBe("archived")

    const error = await captureError(client.complete({
      prompt: "private prompt that must never reach a retired backend",
    }))
    expect(error).toBeInstanceOf(PsionicQwenClientError)
    expect(error).toMatchObject({
      failureClass: "archived",
      reason: "Psionic Qwen backend is archived.",
      receipt: {
        archivedTo: "backroom.openagents_prune_20260708_tassadar_psionic",
      },
    })
    expect(JSON.stringify(error)).not.toContain("private prompt")
    expect(fetchCalls).toBe(0)
  })
})

async function captureError(effect: Effect.Effect<unknown, unknown>): Promise<unknown> {
  try {
    await Effect.runPromise(effect)
  } catch (error) {
    return error
  }

  throw new Error("expected effect to fail")
}
