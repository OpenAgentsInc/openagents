export type UpdateCheckResultLike = {
  isAvailable: boolean
  isRollBackToEmbedded?: boolean
}

export type UpdatePolicyOptions = {
  hasEmbeddedUpdate?: boolean
}

export type UpdatePolicyAction = "download_and_reload" | "none"

export type LaunchSource = "embedded" | "update"

export function decideUpdatePolicyAction(
  result: UpdateCheckResultLike,
  options: UpdatePolicyOptions = {},
): UpdatePolicyAction {
  if (!result.isAvailable) {
    return "none"
  }

  if (result.isRollBackToEmbedded && options.hasEmbeddedUpdate === false) {
    return "none"
  }

  return "download_and_reload"
}

export function describeLaunchSource(isEmbeddedLaunch: boolean): LaunchSource {
  return isEmbeddedLaunch ? "embedded" : "update"
}
