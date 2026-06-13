export type OtaUpdateCheckInput = {
  currentUpdateId: string | null
  manifestId: string | null
  runtimeMatches: boolean
}

export type OtaUpdateCheckDecision = {
  apply: boolean
  reason: string
}

export function decideUpdateApply(input: OtaUpdateCheckInput): OtaUpdateCheckDecision {
  if (!input.runtimeMatches) {
    return {
      apply: false,
      reason: "runtime does not match",
    }
  }

  if (input.manifestId === null) {
    return {
      apply: false,
      reason: "manifest id is missing",
    }
  }

  if (input.manifestId === input.currentUpdateId) {
    return {
      apply: false,
      reason: "manifest is already current",
    }
  }

  return {
    apply: true,
    reason: "manifest is new and runtime matches",
  }
}
