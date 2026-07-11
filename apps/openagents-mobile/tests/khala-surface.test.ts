import { describe, expect, test } from "bun:test"
import { Effect, Stream } from "@effect-native/core/effect"

import { buildHomeProgram, renderContentView } from "../src/screens/home-core"
import type { KhalaTurnClient } from "../src/screens/khala-core"

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise((resolve) => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), (option) => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.khala_surface.v1", () => {
  test("Khala mode sends the running transcript to the typed orchestration client and renders its reply", async () => {
    const calls: Array<unknown> = []
    const client: KhalaTurnClient = {
      sendTurn: async (input) => {
        calls.push(input)
        return { reply: "Khala routed this through the public orchestration path." }
      },
    }
    const program = buildHomeProgram({ khalaTurn: client })
    program.chrome.selectSurfaceMode("khala")
    await Effect.runPromise(settle)
    program.khala.submitTurn("Plan a small mobile change")
    await Effect.runPromise(settle)
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(calls).toEqual([
      { messages: [{ role: "user", content: "Plan a small mobile change" }] },
    ])
    expect(state.khala.entries.map((entry) => [entry.role, entry.status])).toEqual([
      ["user", "done"],
      ["assistant", "done"],
    ])
    expect(state.khala.entries[1]?.text).toContain("public orchestration")
    const view = JSON.stringify(renderContentView({ ...state, surfaceMode: "khala" }))
    expect(view).toContain("Khala routed this through the public orchestration path.")
    expect(view).toContain('"_tag":"Composer"')
    expect(view).toContain("khala-composer")
  })
})
