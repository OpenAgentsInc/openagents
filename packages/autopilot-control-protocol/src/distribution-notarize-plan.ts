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
        ? [
          "sign and notarize desktop build",
          "publish full desktop artifact",
          "create bsdiff delta",
          "publish desktop update feed",
        ]
        : [
          "sign and notarize desktop build",
          "publish full desktop artifact",
          "publish desktop update feed",
        ],
      usesBsdiff,
      requiresNotarize: true,
    }
  }

  if (input.target === "mobile") {
    return {
      steps: [
        "build and upload local iOS binary to TestFlight",
        "submit owner-approved build to App Store review",
      ],
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
