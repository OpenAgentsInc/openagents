export type ApprovalDecision = "approve" | "deny" | "answer"

export type BuildApprovalResponseInput = {
  ref: string
  decision: ApprovalDecision
  answer?: string
}

export type ApprovalResponse = {
  ref: string
  decision: string
  payload: {
    answer?: string
  }
  readOnlyViolation: boolean
  ok: boolean
}

export function buildApprovalResponse(input: BuildApprovalResponseInput): ApprovalResponse {
  const payload: ApprovalResponse["payload"] = {}

  if (input.decision === "answer" && input.answer !== undefined) {
    payload.answer = input.answer
  }

  return {
    ref: input.ref,
    decision: input.decision,
    payload,
    readOnlyViolation: false,
    ok: input.decision !== "answer" || (input.answer?.length ?? 0) > 0,
  }
}
