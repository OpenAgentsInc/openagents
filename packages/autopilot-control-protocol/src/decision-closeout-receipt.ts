// #5004 receipt-backed command closeout. When a remote decision is resolved over
// the bridge (createRemoteDecisionQueue().resolve) and the relay finishes with a
// TERMINAL outcome, the queue produces a canonical, verifiable closeout receipt
// so every client surface (desktop / web / Expo) records the same audit line for
// "this command was closed out, here is exactly what happened".
//
// Pure + transport-agnostic, matching the rest of this package: the caller
// injects the resolution result and a `decidedAt` timestamp; this module
// classifies it into one immutable receipt whose `line` is reconstructed by the
// validator, so tampering with any field invalidates the receipt.
//
// Transient outcomes (offline / overloaded) are EXCLUDED by type: they are not
// closed out — the queue replays them on drain. Only a terminal outcome closes a
// command.

import { isRetryableOutcome, type ActionOutcome } from "./action-receipt.js"
import type { DecisionVerb } from "./decision.js"

// The outcomes that actually close a command out. Everything ActionOutcome can
// be EXCEPT the two transient ones, which the offline queue replays instead.
export type TerminalDecisionOutcome = Exclude<ActionOutcome, "offline" | "overloaded">

// The client surface that resolved the decision. One vocabulary shared across
// the three Autopilot clients.
export type DecisionClient = "desktop" | "web" | "expo"

// The terminal outcomes, enumerated. Single-sourced so the validator and any
// audit consumer (e.g. the closeout ledger) iterate the same vocabulary.
export const TERMINAL_DECISION_OUTCOMES = [
  "applied",
  "duplicate",
  "expired",
  "revoked",
  "stale",
  "unauthorized",
  "unsupported",
  "error",
] as const satisfies ReadonlyArray<TerminalDecisionOutcome>

// The client surfaces, enumerated, same single-sourcing rationale.
export const DECISION_CLIENTS = ["desktop", "web", "expo"] as const satisfies ReadonlyArray<DecisionClient>

export type BuildDecisionCloseoutReceiptInput = {
  // The node's exactly-once decision key (the decision requestId).
  requestId: string
  // What the decision was about (the prompt's actionRef).
  actionRef: string
  // The verb the client chose.
  verb: DecisionVerb
  // The classified terminal transport result.
  outcome: TerminalDecisionOutcome
  // Which surface resolved it.
  client: DecisionClient
  // Who triggered the resolution (owner / autopilot / an agent ref).
  actor: string
  // ISO timestamp of the closeout.
  decidedAt: string
  // The free-text answer, only meaningful when verb === "answer".
  answer?: string
}

export type DecisionCloseoutReceipt = {
  kind: "decision_closeout_receipt"
  requestId: string
  actionRef: string
  verb: DecisionVerb
  outcome: TerminalDecisionOutcome
  client: DecisionClient
  actor: string
  decidedAt: string
  hasAnswer: boolean
  line: string
}

// Is an outcome terminal (closes the command) rather than transient (replayed)?
// The inverse of `isRetryableOutcome`, narrowed for the receipt type.
export function isTerminalDecisionOutcome(outcome: ActionOutcome): outcome is TerminalDecisionOutcome {
  return !isRetryableOutcome(outcome)
}

export function buildDecisionCloseoutReceipt(input: BuildDecisionCloseoutReceiptInput): DecisionCloseoutReceipt {
  const hasAnswer = input.verb === "answer" && (input.answer?.length ?? 0) > 0

  return {
    kind: "decision_closeout_receipt",
    requestId: input.requestId,
    actionRef: input.actionRef,
    verb: input.verb,
    outcome: input.outcome,
    client: input.client,
    actor: input.actor,
    decidedAt: input.decidedAt,
    hasAnswer,
    line: formatDecisionCloseoutLine({
      requestId: input.requestId,
      actionRef: input.actionRef,
      verb: input.verb,
      outcome: input.outcome,
      client: input.client,
      actor: input.actor,
      decidedAt: input.decidedAt,
      hasAnswer,
    }),
  }
}

export function validateDecisionCloseoutReceipt(receipt: unknown): boolean {
  if (!isReceiptRecord(receipt)) return false
  if (receipt.kind !== "decision_closeout_receipt") return false
  if (typeof receipt.requestId !== "string") return false
  if (typeof receipt.actionRef !== "string") return false
  if (!isDecisionVerb(receipt.verb)) return false
  if (!isTerminalOutcomeValue(receipt.outcome)) return false
  if (!isDecisionClient(receipt.client)) return false
  if (typeof receipt.actor !== "string") return false
  if (typeof receipt.decidedAt !== "string") return false
  if (typeof receipt.hasAnswer !== "boolean") return false
  if (typeof receipt.line !== "string") return false

  // hasAnswer is only true for the answer verb; reject inconsistent receipts.
  if (receipt.hasAnswer && receipt.verb !== "answer") return false

  // Reconstruct the canonical line from the validated fields; any tamper of a
  // field (or the line itself) breaks this equality.
  return receipt.line === formatDecisionCloseoutLine({
    requestId: receipt.requestId,
    actionRef: receipt.actionRef,
    verb: receipt.verb,
    outcome: receipt.outcome,
    client: receipt.client,
    actor: receipt.actor,
    decidedAt: receipt.decidedAt,
    hasAnswer: receipt.hasAnswer,
  })
}

function formatDecisionCloseoutLine(receipt: {
  requestId: string
  actionRef: string
  verb: DecisionVerb
  outcome: TerminalDecisionOutcome
  client: DecisionClient
  actor: string
  decidedAt: string
  hasAnswer: boolean
}): string {
  const answer = receipt.hasAnswer ? " with answer" : ""

  return `Decision ${receipt.requestId} (${receipt.actionRef}) ${receipt.verb}${answer} closed out as ${receipt.outcome} on ${receipt.client} by ${receipt.actor} at ${receipt.decidedAt}.`
}

const TERMINAL_OUTCOMES: ReadonlySet<string> = new Set(TERMINAL_DECISION_OUTCOMES)

function isTerminalOutcomeValue(value: unknown): value is TerminalDecisionOutcome {
  return typeof value === "string" && TERMINAL_OUTCOMES.has(value)
}

function isDecisionVerb(value: unknown): value is DecisionVerb {
  return value === "approve" || value === "deny" || value === "answer"
}

function isDecisionClient(value: unknown): value is DecisionClient {
  return typeof value === "string" && (DECISION_CLIENTS as ReadonlyArray<string>).includes(value)
}

function isReceiptRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
