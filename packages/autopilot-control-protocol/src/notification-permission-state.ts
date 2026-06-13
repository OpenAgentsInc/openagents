export type NotificationPermissionInput = {
  granted: boolean
  canAskAgain: boolean
  pushToken: string | null
}

export type NotificationPermissionState =
  | "enabled"
  | "denied"
  | "undetermined"
  | "no_token"

export type NotificationPermissionProjection = {
  state: NotificationPermissionState
  canPrompt: boolean
  registered: boolean
  reason: string
}

export function projectNotificationPermission(
  input: NotificationPermissionInput,
): NotificationPermissionProjection {
  if (input.granted && input.pushToken) {
    return {
      state: "enabled",
      canPrompt: false,
      registered: true,
      reason: "permission_granted_with_token",
    }
  }

  if (!input.granted && !input.canAskAgain) {
    return {
      state: "denied",
      canPrompt: false,
      registered: false,
      reason: "permission_denied",
    }
  }

  if (!input.granted && input.canAskAgain) {
    return {
      state: "undetermined",
      canPrompt: true,
      registered: false,
      reason: "permission_prompt_available",
    }
  }

  return {
    state: "no_token",
    canPrompt: false,
    registered: false,
    reason: "permission_granted_missing_token",
  }
}
