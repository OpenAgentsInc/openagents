import type { SafeTurnProjection } from "@openagentsinc/agent-runtime-schema"

const RECEIPT_ONLY = /^(?:done|complete|completed|finished|handled|ok|success|successful)[.!]?$/iu
const IDENTITY_REQUEST = /\b(?:who|what)\s+(?:are|r)\s+you\b/iu
const IDENTITY_ANSWER = /\b(?:agent|ai|assistant|claude|codex|grok|model|openagents)\b/iu
const STOP_WORDS = new Set([
  "about",
  "after",
  "again",
  "also",
  "and",
  "are",
  "ask",
  "can",
  "claude",
  "codex",
  "could",
  "current",
  "for",
  "from",
  "grok",
  "have",
  "help",
  "into",
  "need",
  "please",
  "request",
  "should",
  "task",
  "that",
  "the",
  "their",
  "them",
  "then",
  "there",
  "these",
  "they",
  "this",
  "those",
  "through",
  "use",
  "want",
  "what",
  "when",
  "where",
  "which",
  "who",
  "will",
  "with",
  "work",
  "would",
  "you",
  "your",
])

export type DelegatedAnswerRejection =
  | "missing_answer"
  | "receipt_only"
  | "objective_mismatch"

export type DelegatedAnswerAdmission =
  | Readonly<{ kind: "accepted"; answer: string }>
  | Readonly<{ kind: "rejected"; reason: DelegatedAnswerRejection }>

export const DELEGATED_RESULT_MISMATCH_REASON =
  "result_mismatch: delegate answer did not address the current request" as const

const signalTokens = (value: string): ReadonlyArray<string> =>
  [...new Set(
    (value.toLocaleLowerCase().match(/[\p{L}\p{N}_-]+/gu) ?? [])
      .map((token) => token.replaceAll(/^[-_]+|[-_]+$/gu, ""))
      .filter((token) => token.length >= 4 || /^\d{2,}$/u.test(token))
      .filter((token) => !STOP_WORDS.has(token)),
  )]

const tokensMatch = (left: string, right: string): boolean =>
  left === right ||
  (left.length >= 5 && right.length >= 5 && (left.startsWith(right) || right.startsWith(left)))

/**
 * Apply a deterministic minimum relevance gate before a delegate answer can
 * become primary assistant prose. This is not a quality score. It rejects only
 * missing answers, receipt-only acknowledgements, and answers with no lexical
 * connection to the current objective.
 */
export const evaluateDelegatedAnswer = (input: Readonly<{
  objective: string
  answer: string | null | undefined
}>): DelegatedAnswerAdmission => {
  const answer = input.answer?.trim() ?? ""
  if (answer === "") return { kind: "rejected", reason: "missing_answer" }
  if (RECEIPT_ONLY.test(answer)) return { kind: "rejected", reason: "receipt_only" }
  if (IDENTITY_REQUEST.test(input.objective) && !IDENTITY_ANSWER.test(answer)) {
    return { kind: "rejected", reason: "objective_mismatch" }
  }
  const objectiveSignals = signalTokens(input.objective)
  const answerSignals = signalTokens(answer)
  if (
    objectiveSignals.length > 0 &&
    !objectiveSignals.some((objective) =>
      answerSignals.some((candidate) => tokensMatch(objective, candidate)),
    )
  ) {
    return { kind: "rejected", reason: "objective_mismatch" }
  }
  return { kind: "accepted", answer }
}

/** The final non-empty assistant line is the only candidate for promotion. */
export const finalDelegatedAnswer = (projection: SafeTurnProjection): string | null => {
  for (let index = projection.messageChain.length - 1; index >= 0; index -= 1) {
    const entry = projection.messageChain[index]
    if (entry?.role === "assistant" && entry.text.trim() !== "") return entry.text
  }
  return null
}

/** Convert a completed but incoherent delegate projection into an honest failure. */
export const admitDelegatedAnswerProjection = (input: Readonly<{
  objective: string
  projection: SafeTurnProjection
}>): SafeTurnProjection => {
  if (input.projection.cardState !== "done") return input.projection
  const admission = evaluateDelegatedAnswer({
    objective: input.objective,
    answer: finalDelegatedAnswer(input.projection),
  })
  if (admission.kind === "accepted") return input.projection
  return {
    ...input.projection,
    cardState: "failed",
    failureReason: DELEGATED_RESULT_MISMATCH_REASON,
  }
}
