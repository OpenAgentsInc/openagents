import { describe, expect, test } from "bun:test"

import {
  decideUpdatePolicyAction,
  describeLaunchSource,
} from "./update-policy"

describe("update policy", () => {
  test("downloads and reloads when an update is available", () => {
    expect(decideUpdatePolicyAction({ isAvailable: true })).toBe("download_and_reload")
  })

  test("does nothing when no update is available", () => {
    expect(decideUpdatePolicyAction({ isAvailable: false })).toBe("none")
  })

  test("does nothing for a rollback directive when no embedded update is available", () => {
    expect(
      decideUpdatePolicyAction(
        { isAvailable: true, isRollBackToEmbedded: true },
        { hasEmbeddedUpdate: false },
      ),
    ).toBe("none")
  })

  test("labels the current launch source", () => {
    expect(describeLaunchSource(true)).toBe("embedded")
    expect(describeLaunchSource(false)).toBe("update")
  })
})
