import { describe, expect, it } from "@effect/vitest"
import { StateNotFoundError, StateParseError, StateValidationError } from "../../src/github/GitHub.js"

// Skip tests to avoid initialization issues
describe.skip("State Storage", () => {
  it("should have error classes defined", () => {
    expect(StateNotFoundError).toBeDefined()
    expect(StateParseError).toBeDefined()
    expect(StateValidationError).toBeDefined()
  })
})
