import { describe, expect, test } from "bun:test"

import { buildDecisionResolve } from "./decision-resolve-command.js"

describe("decision resolve command builder", () => {
  test("builds an approve command", () => {
    expect(buildDecisionResolve({
      ref: "decision.request.fixture.0001",
      choice: "approve",
    })).toEqual({
      ok: true,
      command: {
        type: "decision.resolve",
        ref: "decision.request.fixture.0001",
        choice: "approve",
      },
      errors: [],
    })
  })

  test("builds a deny command with a trimmed ref", () => {
    expect(buildDecisionResolve({
      ref: "  decision.request.fixture.0002  ",
      choice: "deny",
    })).toEqual({
      ok: true,
      command: {
        type: "decision.resolve",
        ref: "decision.request.fixture.0002",
        choice: "deny",
      },
      errors: [],
    })
  })

  test("builds an answer command with a trimmed answer", () => {
    expect(buildDecisionResolve({
      ref: "decision.request.fixture.0003",
      choice: "answer",
      answer: "  Use the preview environment  ",
    })).toEqual({
      ok: true,
      command: {
        type: "decision.resolve",
        ref: "decision.request.fixture.0003",
        choice: "answer",
        answer: "Use the preview environment",
      },
      errors: [],
    })
  })

  test("rejects blank refs", () => {
    expect(buildDecisionResolve({
      ref: "   ",
      choice: "approve",
    })).toEqual({
      ok: false,
      command: {
        type: "decision.resolve",
        ref: "",
        choice: "approve",
      },
      errors: ["ref is required"],
    })
  })

  test("rejects answer choices without an answer", () => {
    expect(buildDecisionResolve({
      ref: "decision.request.fixture.0004",
      choice: "answer",
    })).toEqual({
      ok: false,
      command: {
        type: "decision.resolve",
        ref: "decision.request.fixture.0004",
        choice: "answer",
      },
      errors: ["answer is required when choice is answer"],
    })
  })

  test("rejects answer choices with a blank answer", () => {
    expect(buildDecisionResolve({
      ref: "decision.request.fixture.0005",
      choice: "answer",
      answer: "   ",
    })).toEqual({
      ok: false,
      command: {
        type: "decision.resolve",
        ref: "decision.request.fixture.0005",
        choice: "answer",
      },
      errors: ["answer is required when choice is answer"],
    })
  })
})
