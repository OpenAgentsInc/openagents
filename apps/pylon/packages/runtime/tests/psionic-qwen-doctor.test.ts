import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import { runProbeCli } from "../src/index.js"

describe("archived Psionic Qwen CLI", () => {
  test("doctor returns the typed archive boundary without probing the network", async () => {
    let fetchCalls = 0
    const result = await Effect.runPromise(runProbeCli([
      "backend",
      "psionic",
      "doctor",
      "--base-url",
      "http://127.0.0.1:18080",
      "--json",
    ], {
      fetch: async () => {
        fetchCalls += 1
        return Response.json({ ready: true })
      },
    }))
    const payload = JSON.parse(result.stdout)

    expect(result.exitCode).toBe(1)
    expect(payload).toMatchObject({
      ready: false,
      status: "archived",
      blockerRefs: ["blocker.psionic_qwen35.archived_to_backroom"],
      receipt: {
        archivedTo: "backroom.openagents_prune_20260708_tassadar_psionic",
      },
    })
    expect(fetchCalls).toBe(0)
  })

  test("smoke remains blocked and never serializes the supplied prompt", async () => {
    let fetchCalls = 0
    const result = await Effect.runPromise(runProbeCli([
      "backend",
      "psionic",
      "smoke",
      "--prompt",
      "private smoke prompt",
      "--json",
    ], {
      fetch: async () => {
        fetchCalls += 1
        return Response.json({ ready: true })
      },
    }))
    const payload = JSON.parse(result.stdout)

    expect(result.exitCode).toBe(1)
    expect(payload.state).toBe("blocked")
    expect(payload.readiness.status).toBe("archived")
    expect(payload.admissionBlockerRefs).toContain(
      "blocker.psionic_qwen35.archived_to_backroom",
    )
    expect(result.stdout).not.toContain("private smoke prompt")
    expect(fetchCalls).toBe(0)
  })
})
