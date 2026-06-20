import { describe, expect, test } from "bun:test"

import { projectDecisionPrompt } from "./decision-prompt-view.js"

describe("decision prompt view projection", () => {
  test("projects a DecisionRecord-shaped approval from requestId and actionRef", () => {
    expect(
      projectDecisionPrompt({
        requestId: "decision-1",
        actionRef: "Deploy to production?",
        state: "pending",
        resolvedVerb: null,
        expiresAtMs: 1_900_000_000_000,
        kind: "approval",
      }),
    ).toEqual({
      ref: "decision-1",
      kind: "approval",
      prompt: "Deploy to production?",
      options: ["approve", "deny"],
      requiresAnswer: true,
    })
  })

  test("projects question prompts without options", () => {
    expect(
      projectDecisionPrompt({
        ref: "decision-2",
        kind: "question",
        question: "Which branch should ship?",
      }),
    ).toEqual({
      ref: "decision-2",
      kind: "question",
      prompt: "Which branch should ship?",
      options: [],
      requiresAnswer: true,
    })
  })

  test("projects choice options from strings and labeled records", () => {
    expect(
      projectDecisionPrompt({
        decisionRef: "decision-3",
        kind: "choice",
        prompt: "Pick an environment",
        options: ["preview", { label: "production" }, { value: "staging" }, "", 7],
      }),
    ).toEqual({
      ref: "decision-3",
      kind: "choice",
      prompt: "Pick an environment",
      options: ["preview", "production", "staging"],
      requiresAnswer: true,
    })
  })

  test("infers choice kind when valid options are present", () => {
    expect(
      projectDecisionPrompt({
        id: "decision-4",
        message: "Choose retry strategy",
        options: [{ title: "retry now" }, { id: "retry_later" }],
      }),
    ).toEqual({
      ref: "decision-4",
      kind: "choice",
      prompt: "Choose retry strategy",
      options: ["retry now", "retry_later"],
      requiresAnswer: true,
    })
  })

  test("returns a stable unknown projection for malformed input", () => {
    expect(projectDecisionPrompt(null)).toEqual({
      ref: "",
      kind: "unknown",
      prompt: "",
      options: [],
      requiresAnswer: false,
    })

    expect(projectDecisionPrompt(["not", "a", "record"])).toEqual({
      ref: "",
      kind: "unknown",
      prompt: "",
      options: [],
      requiresAnswer: false,
    })
  })

  test("does not mutate source option arrays and honors explicit requiresAnswer", () => {
    const raw = {
      ref: "decision-5",
      kind: "approval",
      prompt: "Approve cleanup?",
      options: ["allow", "deny"],
      requiresAnswer: false,
    }

    const projection = projectDecisionPrompt(raw)
    projection.options.push("later")

    expect(raw.options).toEqual(["allow", "deny"])
    expect(projectDecisionPrompt(raw)).toEqual({
      ref: "decision-5",
      kind: "approval",
      prompt: "Approve cleanup?",
      options: ["allow", "deny"],
      requiresAnswer: false,
    })
  })

  test("infers approval from available decision verbs", () => {
    expect(
      projectDecisionPrompt({
        requestId: "decision-6",
        prompt: "Allow file edits?",
        availableVerbs: ["approve", "deny", "answer"],
      }),
    ).toEqual({
      ref: "decision-6",
      kind: "approval",
      prompt: "Allow file edits?",
      options: ["approve", "deny"],
      requiresAnswer: true,
    })
  })
})
