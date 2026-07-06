import { describe, expect, test } from "bun:test"

import {
  buildRegisterPushDeviceTokenBody,
  permissionOutcomeIsRegisterable,
  shouldPromptForPushPermission
} from "../src/push/push-registration-core"

// Oracle for khala_mobile.push.permission_prompt_on_first_task_dispatch.v1
describe("shouldPromptForPushPermission", () => {
  test("prompts on the first task dispatch when never prompted before", () => {
    expect(shouldPromptForPushPermission({ hasEverPrompted: false }, "task_dispatched")).toBe(true)
  })

  test("never prompts on app launch, prompted or not", () => {
    expect(shouldPromptForPushPermission({ hasEverPrompted: false }, "app_launch")).toBe(false)
    expect(shouldPromptForPushPermission({ hasEverPrompted: true }, "app_launch")).toBe(false)
  })

  test("never prompts again once already prompted, even on a later task dispatch", () => {
    expect(shouldPromptForPushPermission({ hasEverPrompted: true }, "task_dispatched")).toBe(false)
  })
})

describe("buildRegisterPushDeviceTokenBody", () => {
  test("carries deviceId, token, and platform through unchanged", () => {
    expect(
      buildRegisterPushDeviceTokenBody({
        deviceId: "device-1",
        expoPushToken: "ExponentPushToken[abc]",
        platform: "ios"
      })
    ).toEqual({ deviceId: "device-1", expoPushToken: "ExponentPushToken[abc]", platform: "ios" })
  })
})

describe("permissionOutcomeIsRegisterable", () => {
  test("only 'granted' is registerable", () => {
    expect(permissionOutcomeIsRegisterable("granted")).toBe(true)
    expect(permissionOutcomeIsRegisterable("denied")).toBe(false)
    expect(permissionOutcomeIsRegisterable("undetermined")).toBe(false)
  })
})
