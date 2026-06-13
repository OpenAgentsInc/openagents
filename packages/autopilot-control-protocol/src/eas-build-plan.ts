export type EasBuildPlanInput = {
  mode: "ota" | "rebuild"
  platform: "ios" | "android"
  autoSubmit: boolean
}

export type EasBuildPlan = {
  steps: string[]
  willBuild: boolean
  willSubmit: boolean
  reason: string
}

export function planEasBuild(input: EasBuildPlanInput): EasBuildPlan {
  if (input.mode === "ota") {
    return {
      steps: [],
      willBuild: false,
      willSubmit: false,
      reason: "ota_no_build",
    }
  }

  return {
    steps: [
      `eas build --local --platform ${input.platform}`,
      ...(input.autoSubmit ? [`eas submit -p ${input.platform}`] : []),
    ],
    willBuild: true,
    willSubmit: input.autoSubmit,
    reason: "rebuild_required",
  }
}
