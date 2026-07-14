import { describe, expect, test } from "bun:test"

import { lintBudgetViolations, summarizeLintResults } from "./lint-baseline"

describe("legacy lint baseline", () => {
  test("counts rules and parser/fatal messages", () => {
    expect(summarizeLintResults([
      { messages: [{ ruleId: "known" }, { ruleId: null }] },
      { messages: [{ ruleId: "known" }] },
    ])).toEqual({ known: 2, fatal: 1 })
  })

  test("allows debt reduction but rejects increases and new rule failures", () => {
    const budgets = { known: 2, fatal: 1 }
    expect(lintBudgetViolations({ known: 1, fatal: 1 }, budgets)).toEqual([])
    expect(lintBudgetViolations({ known: 3, newRule: 1 }, budgets)).toEqual([
      "known: 3 exceeds legacy budget 2",
      "newRule: 1 new unbudgeted violation(s)",
    ])
  })
})
