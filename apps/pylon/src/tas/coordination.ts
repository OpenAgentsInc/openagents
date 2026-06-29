export type WorkItemRef = string
export type AgentRef = string

export type WorkItemClaim = {
  readonly workItemRef: WorkItemRef
  readonly agentRef: AgentRef
  readonly claimedAtMs: number
  readonly leaseExpiresAtMs: number
}

export type ClaimLedger = Readonly<Record<WorkItemRef, WorkItemClaim>>

export type ClaimWorkItemInput = {
  readonly workItemRef: WorkItemRef
  readonly agentRef: AgentRef
  readonly nowMs: number
  readonly leaseMs: number
}

export type ClaimWorkItemResult =
  | {
      readonly ok: true
      readonly ledger: ClaimLedger
    }
  | {
      readonly ok: false
      readonly ledger: ClaimLedger
      readonly reason: "live_claim_exists" | "invalid_lease"
    }

export type ReleaseClaimInput = {
  readonly workItemRef: WorkItemRef
}

export type ReleaseClaimResult = {
  readonly ok: boolean
  readonly ledger: ClaimLedger
  readonly reason?: "claim_not_found"
}

export function isExpired(claim: WorkItemClaim, nowMs: number): boolean {
  return nowMs >= claim.leaseExpiresAtMs
}

export function claimWorkItem(
  ledger: ClaimLedger,
  input: ClaimWorkItemInput,
): ClaimWorkItemResult {
  if (input.leaseMs <= 0) {
    return {
      ok: false,
      ledger,
      reason: "invalid_lease",
    }
  }

  const currentClaim = ledger[input.workItemRef]

  if (currentClaim && !isExpired(currentClaim, input.nowMs)) {
    return {
      ok: false,
      ledger,
      reason: "live_claim_exists",
    }
  }

  return {
    ok: true,
    ledger: {
      ...ledger,
      [input.workItemRef]: {
        workItemRef: input.workItemRef,
        agentRef: input.agentRef,
        claimedAtMs: input.nowMs,
        leaseExpiresAtMs: input.nowMs + input.leaseMs,
      },
    },
  }
}

export function releaseClaim(
  ledger: ClaimLedger,
  input: ReleaseClaimInput,
): ReleaseClaimResult {
  if (!ledger[input.workItemRef]) {
    return {
      ok: false,
      ledger,
      reason: "claim_not_found",
    }
  }

  const { [input.workItemRef]: _releasedClaim, ...nextLedger } = ledger

  return {
    ok: true,
    ledger: nextLedger,
  }
}
