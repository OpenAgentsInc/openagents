export type DistributionTarget = "desktop" | "mobile" | "ota"

export type DistributionNotarizePlanInput = {
  target: DistributionTarget
  hasPrevBuild: boolean
}

export type DistributionNotarizePlan = {
  steps: string[]
  usesBsdiff: boolean
  requiresNotarize: boolean
}

export function planDistribution(input: DistributionNotarizePlanInput): DistributionNotarizePlan {
  if (input.target === "desktop") {
    const usesBsdiff = input.hasPrevBuild

    return {
      steps: usesBsdiff
        ? ["notarize desktop build", "create bsdiff delta"]
        : ["notarize desktop build", "publish full desktop build"],
      usesBsdiff,
      requiresNotarize: true,
    }
  }

  if (input.target === "mobile") {
    return {
      steps: ["prepare mobile store submission", "submit mobile build to store"],
      usesBsdiff: false,
      requiresNotarize: false,
    }
  }

  return {
    steps: ["prepare ota bundle", "publish ota update"],
    usesBsdiff: false,
    requiresNotarize: false,
  }
}
