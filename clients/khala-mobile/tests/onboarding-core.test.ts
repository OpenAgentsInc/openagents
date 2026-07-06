import { describe, expect, test } from "bun:test"

import {
  blocksOnZeroBalance,
  deriveThreadTitleFromTask,
  ONBOARDING_SUGGESTED_TASKS,
} from "../src/screens/onboarding-core"

// Oracle for khala_mobile.onboarding.first_task_straight_line.v1
describe("contract khala_mobile.onboarding.first_task_straight_line.v1", () => {
  test("onboarding_never_blocks_on_undetermined_balance.unit — Start is only ever blocked on a CONFIRMED zero/negative balance", () => {
    expect(blocksOnZeroBalance({ ok: true, value: 0 })).toBe(true)
    expect(blocksOnZeroBalance({ ok: false })).toBe(false)
  })
})

describe("ONBOARDING_SUGGESTED_TASKS", () => {
  test("has at least the two audit-named examples plus a third, each with a non-empty prompt", () => {
    expect(ONBOARDING_SUGGESTED_TASKS.length).toBeGreaterThanOrEqual(3)
    const labels = ONBOARDING_SUGGESTED_TASKS.map(task => task.label.toLowerCase())
    expect(labels.some(label => label.includes("explain"))).toBe(true)
    expect(labels.some(label => label.includes("todo"))).toBe(true)
    for (const task of ONBOARDING_SUGGESTED_TASKS) {
      expect(task.prompt.trim().length).toBeGreaterThan(0)
      expect(task.id.trim().length).toBeGreaterThan(0)
    }
  })

  test("every task has a unique id", () => {
    const ids = ONBOARDING_SUGGESTED_TASKS.map(task => task.id)
    expect(new Set(ids).size).toBe(ids.length)
  })
})

describe("deriveThreadTitleFromTask", () => {
  test("uses the trimmed task text as the title when short enough", () => {
    expect(deriveThreadTitleFromTask("  Explain this codebase  ")).toBe("Explain this codebase")
  })

  test("falls back to 'New chat' for blank input", () => {
    expect(deriveThreadTitleFromTask("   ")).toBe("New chat")
  })

  test("truncates a long task with an ellipsis", () => {
    const long = "x".repeat(200)
    const title = deriveThreadTitleFromTask(long)
    expect(title.length).toBeLessThanOrEqual(80)
    expect(title.endsWith("…")).toBe(true)
  })
})

describe("blocksOnZeroBalance", () => {
  test("blocks on a confirmed zero balance", () => {
    expect(blocksOnZeroBalance({ ok: true, value: 0 })).toBe(true)
  })

  test("blocks on a confirmed negative balance", () => {
    expect(blocksOnZeroBalance({ ok: true, value: -5 })).toBe(true)
  })

  test("does not block on a confirmed positive balance", () => {
    expect(blocksOnZeroBalance({ ok: true, value: 500 })).toBe(false)
  })

  test("does not block when the balance could not be determined", () => {
    expect(blocksOnZeroBalance({ ok: false })).toBe(false)
  })
})
