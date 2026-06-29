import { describe, expect, test } from "bun:test"
import {
  resolvePrompt,
  statusLine,
  type InteractivePrompt,
} from "../src/tas/non-interactive"

const ANSI_PATTERN = /\u001b\[[0-9;]*m/

describe("TAS non-interactive core", () => {
  test("turns an interactive prompt into a typed blocker outside interactive mode", () => {
    const prompt: InteractivePrompt = {
      promptRef: "prompt.fixture.approval",
      interactive: true,
    }

    const resolved = resolvePrompt(prompt, { interactive: false })

    expect(resolved).toEqual({
      kind: "prompt_blocked",
      promptRef: "prompt.fixture.approval",
      reason: "interactive_prompt_unavailable",
    })
  })

  test("passes prompts through in interactive mode", () => {
    const prompt: InteractivePrompt = {
      promptRef: "prompt.fixture.choice",
      interactive: true,
    }

    expect(resolvePrompt(prompt, { interactive: true })).toBe(prompt)
  })

  test("formats no-color status lines without ANSI codes", () => {
    const line = statusLine(
      {
        state: "blocked",
        label: "Waiting for operator approval",
        subjectRef: "run.fixture.non_interactive",
      },
      { color: false },
    )

    expect(line).toBe(
      'state=blocked label="Waiting for operator approval" subject_ref=run.fixture.non_interactive',
    )
    expect(line).not.toMatch(ANSI_PATTERN)
  })
})
