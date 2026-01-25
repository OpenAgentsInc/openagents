import { describe, expect, it } from "vitest"
import { runValidationConfig } from "../../../src/effuse/ui/validation.js"

describe("runValidationConfig", () => {
  it("returns errors when checks fail", () => {
    const result = runValidationConfig(
      {
        checks: [
          { fn: "required", message: "Required" },
          { fn: "minLength", args: { min: 4 }, message: "Too short" },
        ],
      },
      {
        value: "hi",
        dataModel: {},
      }
    )

    expect(result.valid).toBe(false)
    expect(result.errors).toEqual(["Too short"])
  })
})
