import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createAskUserTool,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaInteractionAskInput,
  type KhalaInteractionAskResult,
  type KhalaInteractionEvent,
  type KhalaInteractionService,
} from "./index.js"

function interactionService(
  handler: (input: KhalaInteractionAskInput) => KhalaInteractionAskResult,
): KhalaInteractionService {
  return {
    askUser: input => Effect.sync(() => handler(input)),
    marker: "khala.interaction_service",
  }
}

function runAsk(args: Readonly<Record<string, unknown>>, interaction?: KhalaInteractionService) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createAskUserTool()]),
      { arguments: args, id: "call_ask_1", name: "ask_user", sessionId: "s1" },
      makeKhalaToolServices({
        ...(interaction === undefined ? {} : { interaction }),
      }),
    ),
  )
}

function requested(input: KhalaInteractionAskInput, requestId = "req-1"): KhalaInteractionEvent {
  return {
    kind: "user_input_requested",
    payload: {
      allowFreeform: input.allowFreeform,
      choices: input.choices,
      nonBlocking: input.nonBlocking,
      prompt: input.prompt,
      requestId,
    },
    timestampMs: 1,
  }
}

describe("ask_user tool", () => {
  test("returns an interactive free-form answer without leaking prompt or answer to public summary by default", async () => {
    const result = await runAsk(
      { allow_freeform: true, prompt: "Which implementation path should I use?" },
      interactionService(input => ({
        answer: { kind: "freeform", text: "Use the existing package boundary." },
        events: [
          requested(input),
          {
            kind: "user_input_answered",
            payload: { requestId: "req-1" },
            timestampMs: 2,
          },
        ],
        requestId: "req-1",
        status: "answered",
      })),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("Use the existing package boundary.")
    expect(result.publicSummary).toBe("User input received.")
    expect(result.publicSummary).not.toContain("implementation path")
    expect(result.ui).toMatchObject({
      events: [
        { kind: "user_input_requested" },
        { kind: "user_input_answered" },
      ],
      kind: "user_question",
      state: "answered",
    })
    expect(JSON.stringify(result.ui)).not.toContain("approval_requested")
  })

  test("returns a selected choice answer", async () => {
    const result = await runAsk(
      {
        choices: [
          { id: "bun", label: "Bun" },
          { id: "node", label: "Node" },
        ],
        prompt: "Which runtime?",
      },
      interactionService(input => ({
        answer: { choiceId: "bun", kind: "choice", text: "Bun" },
        events: [
          requested(input),
          {
            kind: "user_input_answered",
            payload: { choiceId: "bun", requestId: "req-1" },
            timestampMs: 2,
          },
        ],
        requestId: "req-1",
        status: "answered",
      })),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("Answer choice: Bun")
    expect(result.ui).toMatchObject({
      answer: { choiceId: "bun", kind: "choice" },
    })
    expect(result.ui).toMatchObject({
      choices: expect.arrayContaining([
        expect.objectContaining({ id: "bun", label: "Bun" }),
      ]),
    })
  })

  test("returns typed unavailable in non-interactive hosts", async () => {
    const result = await runAsk({ prompt: "What should I call this branch?" })

    expect(result.status).toBe("unavailable")
    expect(result.modelOutput.text).toContain("User input unavailable")
    expect(result.publicSummary).toBe("User input requested but unavailable.")
    expect(result.ui).toMatchObject({
      events: [
        { kind: "user_input_requested" },
        { kind: "user_input_unavailable" },
      ],
      reason: "host_interaction_unavailable",
      state: "unavailable",
    })
  })

  test("returns pending needs-input for non-blocking host prompts", async () => {
    const result = await runAsk(
      { non_blocking: true, prompt: "Pick a theme color" },
      interactionService(input => ({
        events: [requested(input)],
        reason: "awaiting_operator",
        requestId: "req-1",
        status: "pending",
      })),
    )

    expect(result.status).toBe("needs_input")
    expect(result.publicSummary).toBe("User input requested.")
    expect(result.ui).toMatchObject({
      nonBlocking: true,
      state: "pending",
    })
    expect(JSON.stringify(result.ui)).not.toContain("approval")
  })

  test("uses a default answer after host timeout", async () => {
    const result = await runAsk(
      {
        default_answer: "Keep the current name",
        prompt: "Rename this tool?",
        timeout_ms: 5,
      },
      interactionService(input => ({
        answer: { kind: "default", text: input.defaultAnswer ?? "" },
        events: [
          requested(input),
          {
            kind: "user_input_timed_out",
            payload: { requestId: "req-1" },
            timestampMs: 6,
          },
        ],
        requestId: "req-1",
        status: "timed_out",
      })),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("Default answer: Keep the current name")
    expect(result.ui).toMatchObject({
      state: "timed_out",
      timeoutMs: 5,
    })
  })

  test("rejects permission-shaped requests", async () => {
    const result = await runAsk({ prompt: "Do you approve shell access for this command?" })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("ask_user_failed")
    expect(result.publicSummary).toContain("permission")
  })
})
