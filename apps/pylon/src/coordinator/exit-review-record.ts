export type ExitReviewGate = {
  name: string
  passed: boolean
  evidenceRef?: string
}

export type ExitReviewDecision = "open" | "hold"

export type ExitReviewRecordInput = {
  gates: ExitReviewGate[]
  decidedBy: string
  decidedAt: string
}

export type ExitReviewRecord = {
  schema: string
  decision: ExitReviewDecision
  gates: ExitReviewGate[]
  blockers: string[]
  decidedBy: string
  decidedAt: string
}

export const EXIT_REVIEW_RECORD_SCHEMA =
  "openagents.pylon.exit_review_record.v1"

export function buildExitReviewRecord(
  input: ExitReviewRecordInput,
): ExitReviewRecord {
  const blockers = input.gates
    .filter((gate) => !gate.passed)
    .map((gate) => gate.name)

  return {
    schema: EXIT_REVIEW_RECORD_SCHEMA,
    decision: blockers.length === 0 ? "open" : "hold",
    gates: input.gates,
    blockers,
    decidedBy: input.decidedBy,
    decidedAt: input.decidedAt,
  }
}
