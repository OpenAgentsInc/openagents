import type { DecisionVerb } from "./decision.js"

export type DecisionResolveCommandInput = {
  ref: string
  choice: DecisionVerb
  answer?: string
}

export type DecisionResolveCommand = {
  type: "decision.resolve"
  ref: string
  choice: string
  answer?: string
}

export type DecisionResolveCommandResult = {
  ok: boolean
  command: DecisionResolveCommand
  errors: string[]
}

const VALID_CHOICES: readonly DecisionVerb[] = ["approve", "deny", "answer"]

export function buildDecisionResolve(
  input: DecisionResolveCommandInput,
): DecisionResolveCommandResult {
  const ref = input.ref.trim()
  const answer = input.answer?.trim()
  const errors: string[] = []

  if (ref.length === 0) errors.push("ref is required")
  if (!VALID_CHOICES.includes(input.choice)) {
    errors.push("choice must be approve, deny, or answer")
  }
  if (input.choice === "answer" && (answer === undefined || answer.length === 0)) {
    errors.push("answer is required when choice is answer")
  }

  const command: DecisionResolveCommand = {
    type: "decision.resolve",
    ref,
    choice: input.choice,
  }

  if (input.choice === "answer" && answer !== undefined && answer.length > 0) {
    command.answer = answer
  }

  return {
    ok: errors.length === 0,
    command,
    errors,
  }
}
