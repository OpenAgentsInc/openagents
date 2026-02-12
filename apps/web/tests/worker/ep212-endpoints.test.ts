import { describe, expect, it } from "vitest"

import { resolveEp212PresetUrl, sanitizeLightningHeadersForTask } from "../../src/effuse-host/ep212Endpoints"

describe("EP212 endpoint presets", () => {
  it("fails with missing_env when preset env var is not configured", () => {
    const env = {} as any
    const resolved = resolveEp212PresetUrl("A", env)
    expect(resolved.ok).toBe(false)
    if (!resolved.ok) {
      expect(resolved.errorCode).toBe("missing_env")
    }
  })

  it("sanitizes headers so we don't persist secrets into Convex tasks", () => {
    const cleaned = sanitizeLightningHeadersForTask({
      Authorization: "Bearer secret",
      Cookie: "session=secret",
      Accept: "application/json",
      "Content-Type": "application/json",
      "X-Api-Key": "secret",
    })

    expect(cleaned).toEqual({
      Accept: "application/json",
      "Content-Type": "application/json",
    })
  })
})

