import { describe, expect, test } from "bun:test"

import { validateSpawnRequest } from "./spawn-request-validate.js"

describe("spawn request validation", () => {
  test("accepts codex requests with a trimmed objective", () => {
    expect(validateSpawnRequest({
      adapter: "codex",
      objective: "  implement the session spawn flow  ",
    })).toEqual({
      ok: true,
      adapter: "codex",
      objective: "implement the session spawn flow",
      accountRef: null,
      errors: [],
    })
  })

  test("accepts claude_agent requests with a trimmed account ref", () => {
    expect(validateSpawnRequest({
      adapter: "claude_agent",
      objective: "review the failing tests",
      accountRef: "  account_123  ",
    })).toEqual({
      ok: true,
      adapter: "claude_agent",
      objective: "review the failing tests",
      accountRef: "account_123",
      errors: [],
    })
  })

  test("rejects unsupported adapters", () => {
    expect(validateSpawnRequest({
      adapter: "shell",
      objective: "run a smoke test",
    })).toEqual({
      ok: false,
      adapter: null,
      objective: "run a smoke test",
      accountRef: null,
      errors: ["adapter must be one of codex|claude_agent"],
    })
  })

  test("rejects non-string objectives defensively", () => {
    expect(validateSpawnRequest({
      adapter: "codex",
      objective: { task: "spawn" },
      accountRef: 42,
    })).toEqual({
      ok: false,
      adapter: "codex",
      objective: "",
      accountRef: null,
      errors: ["objective must be a string"],
    })
  })

  test("rejects blank objectives after trimming", () => {
    expect(validateSpawnRequest({
      adapter: "claude_agent",
      objective: " \n\t ",
    })).toEqual({
      ok: false,
      adapter: "claude_agent",
      objective: "",
      accountRef: null,
      errors: ["objective must be non-empty"],
    })
  })

  test("accepts objective strings at the 4000 character limit", () => {
    const objective = "x".repeat(4000)

    expect(validateSpawnRequest({
      adapter: "codex",
      objective,
    })).toEqual({
      ok: true,
      adapter: "codex",
      objective,
      accountRef: null,
      errors: [],
    })
  })

  test("rejects objective strings over the 4000 character limit", () => {
    const objective = "x".repeat(4001)

    expect(validateSpawnRequest({
      adapter: "codex",
      objective,
    })).toEqual({
      ok: false,
      adapter: "codex",
      objective,
      accountRef: null,
      errors: ["objective must be <=4000 characters"],
    })
  })

  test("returns all validation errors without throwing on bad input", () => {
    expect(validateSpawnRequest({
      adapter: null,
      objective: "",
      accountRef: "",
    })).toEqual({
      ok: false,
      adapter: null,
      objective: "",
      accountRef: null,
      errors: [
        "adapter must be one of codex|claude_agent",
        "objective must be non-empty",
      ],
    })
  })
})
