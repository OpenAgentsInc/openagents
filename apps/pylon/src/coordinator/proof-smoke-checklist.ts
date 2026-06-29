export type ProofSmokeLane = {
  name: string
  ran: boolean
  passed: boolean
}

export type ProofSmokeSurface = {
  name: string
  verified: boolean
}

export type ProofSmokeInput = {
  lanes: ProofSmokeLane[]
  surfaces: ProofSmokeSurface[]
}

export type ProofSmokeChecklist = {
  ready: boolean
  missing: string[]
  summary: string
}

export function evaluateProofSmoke(input: ProofSmokeInput): ProofSmokeChecklist {
  const missing = [
    ...input.lanes.flatMap((lane) => {
      const missingLaneChecks: string[] = []

      if (!lane.ran) {
        missingLaneChecks.push(`lane:${lane.name}:ran`)
      }

      if (!lane.passed) {
        missingLaneChecks.push(`lane:${lane.name}:passed`)
      }

      return missingLaneChecks
    }),
    ...input.surfaces
      .filter((surface) => !surface.verified)
      .map((surface) => `surface:${surface.name}:verified`),
  ]

  const ready = missing.length === 0

  return {
    ready,
    missing,
    summary: ready
      ? `Proof smoke ready: ${input.lanes.length} lane(s), ${input.surfaces.length} surface(s).`
      : `Proof smoke blocked: ${missing.length} missing check(s).`,
  }
}
