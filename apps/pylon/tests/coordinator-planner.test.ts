import { describe, expect, test } from "bun:test"

import { parsePlanJson } from "../scripts/multi-session-run"
import {
  initCoordinatorState,
  transitionCoordinatorState,
} from "../src/coordinator/coordinator-state"
import { planIntent } from "../src/coordinator/planner"
import type { MultiSessionAccountSelector } from "../scripts/multi-session-run"

const accountPool: MultiSessionAccountSelector[] = [
  { accountRef: "codex-a" },
  { codexHome: "/tmp/pylon-codex-b" },
]

describe("coordinator planner", () => {
  test("produces one session per multipart intent scope", () => {
    const plan = planIntent(
      {
        intentId: "M6-CL-36",
        title: "Coordinator planning core",
        body: "Productize the manual coordinator loop.",
        scopeHint: ["planner", "state machine", "tests"],
      },
      {
        adapter: "codex",
        availableAccounts: accountPool,
        worktreePath: ".",
        verify: ["bun", "test", "apps/pylon/tests/coordinator-planner.test.ts"],
      },
    )

    expect(plan).toHaveLength(3)
    expect(plan.map(entry => entry.id)).toEqual([
      "m6-cl-36-01-planner",
      "m6-cl-36-02-state-machine",
      "m6-cl-36-03-tests",
    ])
    expect(plan.every(entry => entry.adapter === "codex")).toBe(true)
    expect(plan.every(entry => entry.worktreePath === ".")).toBe(true)
    expect(plan[0]?.objective).toContain("Implement intent M6-CL-36 part: planner")
    expect(parsePlanJson(plan)).toHaveLength(3)
  })

  test("produces a single session for a simple intent", () => {
    const plan = planIntent(
      {
        intentId: "simple-1",
        title: "Tighten coordinator copy",
        body: "Update the coordinator objective wording without splitting work.",
      },
      {
        availableAccounts: accountPool,
        worktreePath: ".",
      },
    )

    expect(plan).toHaveLength(1)
    expect(plan[0]?.id).toBe("simple-1")
    expect(plan[0]?.adapter).toBe("codex")
    expect(plan[0]?.verify).toEqual(["bun", "--version"])
    expect(plan[0]?.objective).toContain("Implement intent simple-1: Tighten coordinator copy")
    expect(parsePlanJson(plan)).toHaveLength(1)
  })

  test("carries the account pool onto each session spec", () => {
    const plan = planIntent(
      {
        intentId: "pool-1",
        title: "Fan out account pool",
        body: "- first task\n- second task",
      },
      {
        availableAccounts: accountPool,
        worktreePath: ".",
      },
    )

    expect(plan).toHaveLength(2)
    expect(plan[0]?.accountPool).toEqual(accountPool)
    expect(plan[1]?.accountPool).toEqual(accountPool)

    accountPool[0] = { accountRef: "mutated" }
    expect(plan[0]?.accountPool?.[0]).toEqual({ accountRef: "codex-a" })
  })
})

describe("coordinator state machine", () => {
  test("accepts the legal coordinator lifecycle path", () => {
    let state = initCoordinatorState("intent-1")

    state = transitionCoordinatorState(state, { type: "start_planning" })
    expect(state.status).toBe("planning")

    state = transitionCoordinatorState(state, { type: "fan_out" })
    expect(state.status).toBe("fanning_out")

    state = transitionCoordinatorState(state, { type: "start_shipping" })
    expect(state.status).toBe("shipping")

    state = transitionCoordinatorState(state, { type: "mark_shipped" })
    expect(state).toEqual({ intentId: "intent-1", status: "shipped" })
  })

  test("rejects illegal transitions", () => {
    const state = initCoordinatorState("intent-2")

    expect(() => transitionCoordinatorState(state, { type: "fan_out" })).toThrow(
      "illegal coordinator transition: received -> fanning_out",
    )
  })
})
