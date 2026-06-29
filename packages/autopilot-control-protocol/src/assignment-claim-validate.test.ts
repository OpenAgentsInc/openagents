import { describe, expect, test } from "bun:test"

import { validateAssignmentClaim } from "./assignment-claim-validate.js"

describe("assignment claim validation", () => {
  test("accepts open assignments with a non-empty assignment ref", () => {
    expect(validateAssignmentClaim({
      assignmentRef: "assignment_4928",
      state: "open",
    })).toEqual({
      ok: true,
      assignmentRef: "assignment_4928",
      errors: [],
    })
  })

  test("trims assignment refs before checking emptiness", () => {
    expect(validateAssignmentClaim({
      assignmentRef: "  OpenAgentsInc/openagents#4928  ",
      state: "open",
    })).toEqual({
      ok: true,
      assignmentRef: "OpenAgentsInc/openagents#4928",
      errors: [],
    })
  })

  test("rejects blank assignment refs", () => {
    expect(validateAssignmentClaim({
      assignmentRef: " \n\t ",
      state: "open",
    })).toEqual({
      ok: false,
      assignmentRef: "",
      errors: ["assignmentRef must be a non-empty string"],
    })
  })

  test("rejects non-string assignment refs defensively", () => {
    expect(validateAssignmentClaim({
      assignmentRef: 4928,
      state: "open",
    })).toEqual({
      ok: false,
      assignmentRef: "",
      errors: ["assignmentRef must be a non-empty string"],
    })
  })

  test("rejects claim attempts for non-open assignments", () => {
    expect(validateAssignmentClaim({
      assignmentRef: "assignment_4928",
      state: "claimed",
    })).toEqual({
      ok: false,
      assignmentRef: "assignment_4928",
      errors: ["state must be open"],
    })
  })

  test("accumulates assignment ref and state errors without throwing", () => {
    expect(validateAssignmentClaim({
      assignmentRef: null,
      state: undefined,
    })).toEqual({
      ok: false,
      assignmentRef: "",
      errors: [
        "assignmentRef must be a non-empty string",
        "state must be open",
      ],
    })
  })
})
