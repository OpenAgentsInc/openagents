export type ApprovalDecision = "approve" | "deny" | "answer"

export type ApprovalLedgerRecord = {
  applied: boolean
  decision: ApprovalDecision
  duplicate: boolean
}

export type ApprovalLedger = {
  record(key: string, decision: ApprovalDecision): ApprovalLedgerRecord
}

export function createApprovalLedger(): ApprovalLedger {
  const decisions = new Map<string, ApprovalDecision>()

  return {
    record(key, decision) {
      const recorded = decisions.get(key)

      if (recorded !== undefined) {
        return {
          applied: false,
          decision: recorded,
          duplicate: true,
        }
      }

      decisions.set(key, decision)

      return {
        applied: true,
        decision,
        duplicate: false,
      }
    },
  }
}
