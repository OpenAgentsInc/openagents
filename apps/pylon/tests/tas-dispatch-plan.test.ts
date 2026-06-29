import { describe, expect, test } from "bun:test"
import { join, relative } from "node:path"

import { parsePlanJson, type MultiSessionPlanEntry } from "../scripts/multi-session-run"
import { toMultiSessionPlan } from "../src/coordinator/dispatch-plan"

const intentPlan: MultiSessionPlanEntry[] = [
  {
    id: "M6-CL-36 planner",
    adapter: "codex",
    objective: "Plan the coordinator dispatch.",
    verify: ["bun", "test", "apps/pylon/tests/tas-dispatch-plan.test.ts"],
    worktreePath: ".",
  },
  {
    id: "M6-CL-36 planner",
    adapter: "codex",
    objective: "Prove the coordinator dispatch.",
    verify: ["bun", "--version"],
    worktreePath: ".",
  },
  {
    id: "Final wiring",
    adapter: "codex",
    objective: "Leave runner wiring for a later change.",
    verify: ["bun", "--version"],
    worktreePath: ".",
  },
]

describe("coordinator dispatch plan", () => {
  test("maps N planned specs to N runnable sessions", () => {
    const plan = toMultiSessionPlan(intentPlan, {
      accountPool: [{ codexHome: "/tmp/codex-a" }],
      worktreeBase: "/tmp/pylon-worktrees",
    })

    expect(plan.sessions).toHaveLength(intentPlan.length)
    expect(plan.sessions.map(session => session.objective)).toEqual(intentPlan.map(session => session.objective))
    expect(plan.sessions.map(session => session.adapter)).toEqual(["codex", "codex", "codex"])
    expect(parsePlanJson(plan)).toHaveLength(intentPlan.length)
  })

  test("carries the run-level account pool", () => {
    const accountPool = [{ codexHome: "/tmp/codex-a" }, { codexHome: "/tmp/codex-b" }]
    const plan = toMultiSessionPlan(intentPlan.slice(0, 1), {
      accountPool,
      worktreeBase: "/tmp/pylon-worktrees",
    })

    expect(plan.accountPool).toEqual(accountPool)

    accountPool[0] = { codexHome: "/tmp/mutated" }
    expect(plan.accountPool[0]).toEqual({ codexHome: "/tmp/codex-a" })
  })

  test("assigns unique worktree paths under the requested base", () => {
    const worktreeBase = "/tmp/pylon-worktrees"
    const plan = toMultiSessionPlan(intentPlan, {
      accountPool: [{ codexHome: "/tmp/codex-a" }],
      worktreeBase,
    })
    const paths = plan.sessions.map(session => session.worktreePath)

    expect(new Set(paths).size).toBe(intentPlan.length)
    expect(paths).toEqual([
      join(worktreeBase, "01-m6-cl-36-planner"),
      join(worktreeBase, "02-m6-cl-36-planner"),
      join(worktreeBase, "03-final-wiring"),
    ])
    expect(paths.every(path => path !== undefined && !relative(worktreeBase, path).startsWith(".."))).toBe(true)
  })
})
