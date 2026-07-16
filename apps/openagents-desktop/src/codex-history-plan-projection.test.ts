import { describe, expect, test } from "vite-plus/test"

import { planEntriesFromRecord, planWorkbenchItemFromRow } from "./codex-history.ts"

/**
 * History `plan`/`todo_list` row -> typed `WorkbenchItem` projection (T8
 * #8865, one of the three unified plan sources). `projectRow` in
 * `codex-history.ts` is not itself exported (it works over raw rollout JSONL
 * envelopes and this repo has no existing fixture-file test harness for it),
 * so this covers the new tolerant reader / typed-sidecar constructor directly
 * — the exact logic `projectRow` calls for `kind === "plan"` rows.
 */
describe("planEntriesFromRecord (tolerant history plan/todo_list reader)", () => {
  test("reads the canonical {step,status} shape", () => {
    expect(planEntriesFromRecord([
      { step: "Reproduce the bug", status: "completed" },
      { step: "Land the fix", status: "in_progress" },
      { step: "Ship it", status: "pending" },
    ])).toEqual([
      { step: "Reproduce the bug", status: "completed" },
      { step: "Land the fix", status: "in_progress" },
      { step: "Ship it", status: "pending" },
    ])
  })

  test("reads alternate field names ({content,status}, {text,completed})", () => {
    expect(planEntriesFromRecord([
      { content: "Audit the queue", status: "in-progress" },
      { text: "Write tests", completed: true },
      { title: "Ship", completed: false },
    ])).toEqual([
      { step: "Audit the queue", status: "in_progress" },
      { step: "Write tests", status: "completed" },
      { step: "Ship", status: "pending" },
    ])
  })

  test("returns [] for a non-array or unrecognizable value so callers fall back to prose", () => {
    expect(planEntriesFromRecord("Investigate the bug, then fix it.")).toEqual([])
    expect(planEntriesFromRecord(undefined)).toEqual([])
    expect(planEntriesFromRecord([{ nothing: "useful" }])).toEqual([])
  })
})

describe("planWorkbenchItemFromRow (history plan/todo_list typed sidecar)", () => {
  test("structured entries win when the row carries a recognizable list", () => {
    const item = planWorkbenchItemFromRow(
      { type: "todo_list", plan: [{ step: "Audit the queue", status: "completed" }] },
      value => value,
    )
    expect(item).toEqual({
      kind: "plan",
      source: "codex",
      entries: [{ step: "Audit the queue", status: "completed" }],
    })
  })

  test("falls back to flattened prose when there is no structured list", () => {
    const item = planWorkbenchItemFromRow({ type: "plan", text: "Investigate the flaky auth test." }, value => value)
    expect(item).toEqual({
      kind: "plan",
      source: "codex",
      entries: [],
      prose: "Investigate the flaky auth test.",
    })
  })

  test("returns null for a genuinely empty plan row (neither entries nor prose)", () => {
    expect(planWorkbenchItemFromRow({ type: "plan" }, value => value)).toBeNull()
    expect(planWorkbenchItemFromRow({ type: "plan", text: "" }, value => value)).toBeNull()
  })

  test("applies the redactor to prose text", () => {
    const item = planWorkbenchItemFromRow(
      { type: "plan", text: "the plan touches a secret file" },
      value => value.replaceAll("secret", "[REDACTED]"),
    )
    expect(item).toMatchObject({ prose: "the plan touches a [REDACTED] file" })
  })
})
