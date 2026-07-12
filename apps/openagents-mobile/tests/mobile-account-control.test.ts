import { describe, expect, test } from "bun:test"

import {
  initialHomeState,
  mobileAccountControl,
  renderContentView,
  type HomeState,
  type MobileSyncPhase,
} from "../src/screens/home-core"

const openagentsSurface = (syncPhase: MobileSyncPhase): HomeState => ({
  ...initialHomeState,
  surfaceMode: "openagents",
  syncPhase,
})

const buttonLabels = (syncPhase: MobileSyncPhase): string => {
  const serialized = JSON.stringify(renderContentView(openagentsSurface(syncPhase)))
  return serialized
}

describe("contract openagents_mobile.chat.post_auth_live_upgrade.v1 · account control", () => {
  test("every confirmed post-authentication phase shows Sign out", () => {
    const authenticated: MobileSyncPhase[] = [
      "session_ready",
      "bootstrapping",
      "catching_up",
      "live",
      "must_refetch",
      "stale",
    ]
    for (const phase of authenticated) {
      expect(mobileAccountControl(phase)).toBe("sign_out")
      const serialized = buttonLabels(phase)
      expect(serialized).toContain("openagents-sign-out")
      expect(serialized).not.toContain("Link OpenAgents account")
    }
  })

  test("genuinely unauthenticated phases show Link OpenAgents account", () => {
    const unauthenticated: MobileSyncPhase[] = [
      "unconfigured",
      "local_ready",
      "unavailable",
      "denied",
      "credential_present_unverified",
      "idle",
    ]
    for (const phase of unauthenticated) {
      expect(mobileAccountControl(phase)).toBe("sign_in")
      const serialized = buttonLabels(phase)
      expect(serialized).toContain("Link OpenAgents account")
      expect(serialized).not.toContain("openagents-sign-out")
    }
  })

  test("an in-flight browser step shows neither control", () => {
    expect(mobileAccountControl("authenticating")).toBe("none")
    const serialized = buttonLabels("authenticating")
    expect(serialized).not.toContain("Link OpenAgents account")
    expect(serialized).not.toContain("openagents-sign-out")
  })
})
