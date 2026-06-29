export type SmokeClaim = {
  claim: string
  evidenceRef?: string
}

export type SmokeProofBoundaryResult = {
  ok: boolean
  unbacked: string[]
}

export type SmokeReceipt = {
  ok: boolean
  evidenceRefs: string[]
  unbackedCount: number
}

export function checkProofBoundary(claims: readonly SmokeClaim[]): SmokeProofBoundaryResult {
  const unbacked = claims
    .filter((claim) => !claim.evidenceRef?.trim())
    .map((claim) => claim.claim)

  return {
    ok: unbacked.length === 0,
    unbacked,
  }
}

export function buildSmokeReceipt(claims: readonly SmokeClaim[]): SmokeReceipt {
  const result = checkProofBoundary(claims)

  return {
    ok: result.ok,
    evidenceRefs: claims
      .map((claim) => claim.evidenceRef?.trim())
      .filter((evidenceRef): evidenceRef is string => Boolean(evidenceRef)),
    unbackedCount: result.unbacked.length,
  }
}
