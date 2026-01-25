import { describe, expect, it } from "vitest"
import { evaluateVisibility } from "../../../src/effuse/ui/visibility.js"

describe("evaluateVisibility", () => {
  it("evaluates path-based visibility", () => {
    const visible = evaluateVisibility(
      { path: "/flags/show" },
      { dataModel: { flags: { show: true } } }
    )
    const hidden = evaluateVisibility(
      { path: "/flags/show" },
      { dataModel: { flags: { show: false } } }
    )
    expect(visible).toBe(true)
    expect(hidden).toBe(false)
  })

  it("evaluates logic expressions", () => {
    const result = evaluateVisibility(
      { and: [{ path: "/ready" }, { eq: [{ path: "/mode" }, "live"] }] },
      { dataModel: { ready: true, mode: "live" } }
    )
    expect(result).toBe(true)
  })
})
