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

  if (input.platform === "android") {
    return {
      steps: [
        "android local release build is not configured; add a signed Gradle bundle path before shipping Android",
      ],
      willBuild: false,
      willSubmit: false,
      reason: "android_local_release_not_configured",
    }
  }

  return {
    steps: [
      input.autoSubmit
        ? "clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh"
        : "clients/khala-ios/AutopilotRemoteControl/scripts/build-and-submit.sh --build-only",
    ],
    willBuild: true,
    willSubmit: input.autoSubmit,
    reason: "rebuild_required",
  }
}
