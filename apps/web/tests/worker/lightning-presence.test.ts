import { describe, expect, it } from "vitest"

import {
  DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS,
  isExecutorPresenceFresh,
} from "../../src/effuse-host/lightningPresence"

describe("apps/web lightning executor presence staleness", () => {
  it("treats missing lastSeenAtMs as offline", () => {
    expect(isExecutorPresenceFresh({ lastSeenAtMs: null, nowMs: 1000 })).toBe(false)
  })

  it("treats recent lastSeenAtMs as online", () => {
    expect(
      isExecutorPresenceFresh({
        lastSeenAtMs: 1_000,
        nowMs: 1_000 + DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS - 1,
      }),
    ).toBe(true)
  })

  it("treats stale lastSeenAtMs as offline", () => {
    expect(
      isExecutorPresenceFresh({
        lastSeenAtMs: 1_000,
        nowMs: 1_000 + DEFAULT_EXECUTOR_PRESENCE_MAX_AGE_MS + 1,
      }),
    ).toBe(false)
  })
})

