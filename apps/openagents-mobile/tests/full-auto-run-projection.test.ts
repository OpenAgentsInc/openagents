import { describe, expect, test } from "vite-plus/test"

import {
  FULL_AUTO_RUN_STALE_AFTER_MS,
  isFullAutoRunLifecycleActive,
  isFullAutoRunProjectionActive,
  isFullAutoRunProjectionFresh,
  truncateFullAutoRunObjective,
  type FullAutoRunMobileProjection,
} from "../src/full-auto/full-auto-run-projection"

const baseProjection: FullAutoRunMobileProjection = {
  runRef: "run.full-auto.test-0001",
  threadRef: "thread.full-auto.test.0001",
  objective: "Ship the mobile Full Auto live thread.",
  doneCondition: "Mobile shows the live thread and header.",
  lifecycleState: "running",
  workspaceLabel: "openagents",
  startedAt: "2026-07-17T00:00:00.000Z",
  updatedAt: "2026-07-17T00:00:00.000Z",
  lastTransition: { actor: "owner_ui", at: "2026-07-17T00:00:00.000Z" },
}

describe("full-auto-run-projection mobile helpers (openagents #8982, over the real #8981 schema)", () => {
  test("isFullAutoRunLifecycleActive: running/pausing/paused/retrying/stalled count as active", () => {
    expect(isFullAutoRunLifecycleActive("running")).toBe(true)
    expect(isFullAutoRunLifecycleActive("pausing")).toBe(true)
    expect(isFullAutoRunLifecycleActive("paused")).toBe(true)
    expect(isFullAutoRunLifecycleActive("retrying")).toBe(true)
    expect(isFullAutoRunLifecycleActive("stalled")).toBe(true)
  })

  test("isFullAutoRunLifecycleActive: draft (not yet started) and terminal states are not active", () => {
    expect(isFullAutoRunLifecycleActive("draft")).toBe(false)
    expect(isFullAutoRunLifecycleActive("completed")).toBe(false)
    expect(isFullAutoRunLifecycleActive("failed")).toBe(false)
    expect(isFullAutoRunLifecycleActive("stopped")).toBe(false)
    expect(isFullAutoRunLifecycleActive("cap_reached")).toBe(false)
  })

  test("isFullAutoRunProjectionFresh: within the staleness window is fresh", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + 1_000
    expect(isFullAutoRunProjectionFresh(baseProjection, nowMs)).toBe(true)
  })

  test("isFullAutoRunProjectionFresh: past the staleness window is stale", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + FULL_AUTO_RUN_STALE_AFTER_MS + 1
    expect(isFullAutoRunProjectionFresh(baseProjection, nowMs)).toBe(false)
  })

  test("isFullAutoRunProjectionActive: active-run case (fresh + active lifecycle)", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + 1_000
    expect(isFullAutoRunProjectionActive(baseProjection, nowMs)).toBe(true)
  })

  test("isFullAutoRunProjectionActive: stale/expired-run case (active lifecycle but too old)", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + FULL_AUTO_RUN_STALE_AFTER_MS + 1
    expect(isFullAutoRunProjectionActive(baseProjection, nowMs)).toBe(false)
  })

  test("isFullAutoRunProjectionActive: no-active-run case (terminal lifecycle, even if fresh)", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + 1_000
    expect(isFullAutoRunProjectionActive({ ...baseProjection, lifecycleState: "completed" }, nowMs)).toBe(false)
  })

  test("isFullAutoRunProjectionActive: draft case (not yet started, even if fresh)", () => {
    const nowMs = Date.parse(baseProjection.updatedAt) + 1_000
    expect(isFullAutoRunProjectionActive({ ...baseProjection, lifecycleState: "draft" }, nowMs)).toBe(false)
  })

  test("truncateFullAutoRunObjective leaves short objectives untouched", () => {
    expect(truncateFullAutoRunObjective("Ship it.")).toBe("Ship it.")
  })

  test("truncateFullAutoRunObjective truncates long objectives with an ellipsis", () => {
    const long = "a".repeat(200)
    const truncated = truncateFullAutoRunObjective(long, 96)
    expect(truncated.length).toBe(96)
    expect(truncated.endsWith("…")).toBe(true)
  })
})
