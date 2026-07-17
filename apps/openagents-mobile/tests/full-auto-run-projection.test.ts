import { describe, expect, test } from "vite-plus/test"

import {
  decodeFullAutoRunMobileProjection,
  FULL_AUTO_RUN_STALE_AFTER_MS,
  isFullAutoRunLifecycleActive,
  isFullAutoRunProjectionActive,
  isFullAutoRunProjectionFresh,
  truncateFullAutoRunObjective,
  type FullAutoRunMobileProjection,
} from "../src/full-auto/full-auto-run-projection"

const baseProjection: FullAutoRunMobileProjection = {
  schema: "full_auto_run.mobile_projection.v1",
  runRef: "full_auto_run.test.0001",
  threadRef: "thread.full-auto.test.0001",
  objective: "Ship the mobile Full Auto live thread.",
  doneCondition: "Mobile shows the live thread and header.",
  lifecycleState: "running",
  workspaceLabel: "openagents",
  startedAt: "2026-07-17T00:00:00.000Z",
  updatedAt: "2026-07-17T00:00:00.000Z",
}

describe("contract full_auto_run.mobile_projection.v1", () => {
  test("decodes a well-formed projection", () => {
    expect(decodeFullAutoRunMobileProjection(baseProjection)).toEqual(baseProjection)
  })

  test("rejects a projection with an unknown lifecycle state", () => {
    expect(() => decodeFullAutoRunMobileProjection({
      ...baseProjection,
      lifecycleState: "unknown",
    })).toThrow()
  })

  test("rejects a projection with the wrong schema id", () => {
    expect(() => decodeFullAutoRunMobileProjection({
      ...baseProjection,
      schema: "some.other.schema.v1",
    })).toThrow()
  })

  test("isFullAutoRunLifecycleActive: only running/paused/stalled count as active", () => {
    expect(isFullAutoRunLifecycleActive("running")).toBe(true)
    expect(isFullAutoRunLifecycleActive("paused")).toBe(true)
    expect(isFullAutoRunLifecycleActive("stalled")).toBe(true)
    expect(isFullAutoRunLifecycleActive("completed")).toBe(false)
    expect(isFullAutoRunLifecycleActive("failed")).toBe(false)
    expect(isFullAutoRunLifecycleActive("cancelled")).toBe(false)
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
