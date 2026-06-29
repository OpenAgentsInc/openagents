import { describe, expect, test } from "bun:test"

import { projectNotificationPermission } from "./notification-permission-state.js"

describe("notification permission state projection", () => {
  test("enables notifications when permission is granted and a push token exists", () => {
    expect(projectNotificationPermission({
      granted: true,
      canAskAgain: false,
      pushToken: "ExponentPushToken[alpha]",
    })).toEqual({
      state: "enabled",
      canPrompt: false,
      registered: true,
      reason: "permission_granted_with_token",
    })
  })

  test("enabled state does not depend on canAskAgain when a token exists", () => {
    expect(projectNotificationPermission({
      granted: true,
      canAskAgain: true,
      pushToken: "ExponentPushToken[beta]",
    })).toEqual({
      state: "enabled",
      canPrompt: false,
      registered: true,
      reason: "permission_granted_with_token",
    })
  })

  test("denies notifications when permission is blocked and cannot be asked again", () => {
    expect(projectNotificationPermission({
      granted: false,
      canAskAgain: false,
      pushToken: null,
    })).toEqual({
      state: "denied",
      canPrompt: false,
      registered: false,
      reason: "permission_denied",
    })
  })

  test("keeps denied state even if a stale token is still present", () => {
    expect(projectNotificationPermission({
      granted: false,
      canAskAgain: false,
      pushToken: "ExponentPushToken[stale]",
    })).toEqual({
      state: "denied",
      canPrompt: false,
      registered: false,
      reason: "permission_denied",
    })
  })

  test("marks permission as undetermined when the app can still prompt", () => {
    expect(projectNotificationPermission({
      granted: false,
      canAskAgain: true,
      pushToken: null,
    })).toEqual({
      state: "undetermined",
      canPrompt: true,
      registered: false,
      reason: "permission_prompt_available",
    })
  })

  test("keeps undetermined state ahead of stale token registration", () => {
    expect(projectNotificationPermission({
      granted: false,
      canAskAgain: true,
      pushToken: "ExponentPushToken[stale]",
    })).toEqual({
      state: "undetermined",
      canPrompt: true,
      registered: false,
      reason: "permission_prompt_available",
    })
  })

  test("requires a token before considering granted permission registered", () => {
    expect(projectNotificationPermission({
      granted: true,
      canAskAgain: false,
      pushToken: null,
    })).toEqual({
      state: "no_token",
      canPrompt: false,
      registered: false,
      reason: "permission_granted_missing_token",
    })
  })
})
