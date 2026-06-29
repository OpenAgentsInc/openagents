import { describe, expect, test } from "bun:test"

import { buildAssignmentAccept } from "./assignment-accept-command.js"

describe("assignment accept command builder", () => {
  test("builds an accept command for open assignments with a non-empty lease ref", () => {
    expect(buildAssignmentAccept({
      leaseRef: "lease.public.4928",
      state: "open",
    })).toEqual({
      ok: true,
      command: {
        type: "assignments.accept",
        leaseRef: "lease.public.4928",
      },
      errors: [],
    })
  })

  test("trims lease refs before building the command", () => {
    expect(buildAssignmentAccept({
      leaseRef: "  lease.public.4928  ",
      state: "open",
    })).toEqual({
      ok: true,
      command: {
        type: "assignments.accept",
        leaseRef: "lease.public.4928",
      },
      errors: [],
    })
  })

  test("rejects blank lease refs", () => {
    expect(buildAssignmentAccept({
      leaseRef: " \n\t ",
      state: "open",
    })).toEqual({
      ok: false,
      command: null,
      errors: ["leaseRef must be a non-empty string"],
    })
  })

  test("rejects non-string lease refs defensively", () => {
    expect(buildAssignmentAccept({
      leaseRef: 4928,
      state: "open",
    })).toEqual({
      ok: false,
      command: null,
      errors: ["leaseRef must be a non-empty string"],
    })
  })

  test("rejects accept attempts for non-open assignments", () => {
    expect(buildAssignmentAccept({
      leaseRef: "lease.public.4928",
      state: "accepted",
    })).toEqual({
      ok: false,
      command: null,
      errors: ["state must be open"],
    })
  })

  test("accumulates lease ref and state errors without throwing", () => {
    expect(buildAssignmentAccept({
      leaseRef: null,
      state: undefined,
    })).toEqual({
      ok: false,
      command: null,
      errors: [
        "leaseRef must be a non-empty string",
        "state must be open",
      ],
    })
  })
})
