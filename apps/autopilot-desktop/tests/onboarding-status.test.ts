import { describe, expect, it } from "bun:test"
import {
  projectOnboardingStatus,
  type OnboardingStatusInput,
  type OnboardingStep,
} from "../src/shared/onboarding-status"

// AO-4 (#5445): the live onboarding chain projection. Every step is driven by
// real observable state — these tests assert pending/active/done/failed +
// retry-on-failure across the chain, including a failed+retry path.

const base: OnboardingStatusInput = {
  fetchedAt: "2026-06-18T00:00:00.000Z",
  identityChoiceMade: false,
  identityLabel: null,
  agentRegistered: false,
  nodeLaunchStatus: null,
  localPylonReady: false,
  onboardingEnvConfigured: false,
  walletReceiveReady: false,
  walletBalanceSats: null,
  openAssignmentCount: 0,
  forumTipReady: false,
}

const step = (steps: readonly OnboardingStep[], id: string): OnboardingStep => {
  const found = steps.find(s => s.id === id)
  if (!found) throw new Error(`step ${id} missing`)
  return found
}

describe("projectOnboardingStatus (AO-4)", () => {
  it("fresh first run: identity choice is the active 'you are here' step", () => {
    const res = projectOnboardingStatus(base)
    expect(res.complete).toBe(false)
    expect(res.currentStepId).toBe("identity")
    expect(step(res.steps, "identity").status).toBe("active")
    // Downstream steps are pending, never done, never faked.
    expect(step(res.steps, "registered").status).toBe("pending")
    expect(step(res.steps, "earned").status).toBe("pending")
    // The full chain is present.
    expect(res.steps.map(s => s.id)).toEqual([
      "identity",
      "registered",
      "node-online",
      "wallet",
      "payout",
      "tip-ready",
      "presence",
      "tassadar",
      "claimed",
      "earned",
    ])
  })

  it("AF-2: tip-ready is pending until the wallet is receive-ready, then active, then done", () => {
    const registered = {
      ...base,
      identityChoiceMade: true,
      agentRegistered: true,
      onboardingEnvConfigured: true,
      nodeLaunchStatus: "online" as const,
      localPylonReady: true,
    }
    // Registered but wallet not receive-ready yet → pending.
    expect(
      step(projectOnboardingStatus(registered).steps, "tip-ready").status,
    ).toBe("pending")
    // Wallet receive-ready, claim not yet landed → active.
    expect(
      step(
        projectOnboardingStatus({ ...registered, walletReceiveReady: true })
          .steps,
        "tip-ready",
      ).status,
    ).toBe("active")
    // Receipt persisted (forumTipReady) → done.
    const done = step(
      projectOnboardingStatus({
        ...registered,
        walletReceiveReady: true,
        forumTipReady: true,
      }).steps,
      "tip-ready",
    )
    expect(done.status).toBe("done")
    expect(done.message).toContain("tips")
  })

  it("identity chosen, node launching: registration is active, current step advances", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      identityLabel: "new: Studio Agent",
      nodeLaunchStatus: "launching",
    })
    expect(step(res.steps, "identity").status).toBe("done")
    expect(step(res.steps, "identity").message).toContain("Studio Agent")
    expect(step(res.steps, "node-online").status).toBe("active")
    expect(res.currentStepId).toBe("registered")
  })

  it("registered + online + env configured: presence + payout are done", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      agentRegistered: true,
      onboardingEnvConfigured: true,
      nodeLaunchStatus: "online",
      localPylonReady: true,
      walletReceiveReady: true,
    })
    expect(step(res.steps, "registered").status).toBe("done")
    expect(step(res.steps, "node-online").status).toBe("done")
    expect(step(res.steps, "wallet").status).toBe("done")
    expect(step(res.steps, "presence").status).toBe("done")
    expect(step(res.steps, "payout").status).toBe("done")
    // Not yet joined/claimed/earned.
    expect(step(res.steps, "tassadar").status).toBe("active")
    expect(step(res.steps, "claimed").status).toBe("active")
    expect(step(res.steps, "earned").status).toBe("pending")
  })

  it("work claimed → Tassadar joined + claimed done, earning active", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      agentRegistered: true,
      onboardingEnvConfigured: true,
      nodeLaunchStatus: "online",
      localPylonReady: true,
      walletReceiveReady: true,
      openAssignmentCount: 2,
    })
    expect(step(res.steps, "tassadar").status).toBe("done")
    expect(step(res.steps, "claimed").status).toBe("done")
    expect(step(res.steps, "claimed").message).toContain("2")
    expect(step(res.steps, "earned").status).toBe("active")
  })

  it("sats earned: the whole chain is complete", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      agentRegistered: true,
      onboardingEnvConfigured: true,
      nodeLaunchStatus: "online",
      localPylonReady: true,
      walletReceiveReady: true,
      openAssignmentCount: 1,
      walletBalanceSats: 1234,
    })
    expect(step(res.steps, "earned").status).toBe("done")
    expect(step(res.steps, "earned").message).toContain("1234")
    expect(res.complete).toBe(true)
    expect(res.currentStepId).toBeNull()
    expect(res.hasRetryableFailure).toBe(false)
  })

  it("node failed before online: node + registration FAIL and are retryable", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      agentRegistered: false,
      nodeLaunchStatus: "failed",
      localPylonReady: false,
    })
    const node = step(res.steps, "node-online")
    expect(node.status).toBe("failed")
    expect(node.retryable).toBe(true)
    const registered = step(res.steps, "registered")
    expect(registered.status).toBe("failed")
    expect(registered.retryable).toBe(true)
    // The wizard surfaces a retry affordance rather than dead-ending.
    expect(res.hasRetryableFailure).toBe(true)
    // Still has a current step (not complete, not blank).
    expect(res.currentStepId).toBe("registered")
  })

  it("node fails AFTER registration: presence/payout fail retryably, not silently", () => {
    const res = projectOnboardingStatus({
      ...base,
      identityChoiceMade: true,
      agentRegistered: true,
      onboardingEnvConfigured: true,
      nodeLaunchStatus: "failed",
      localPylonReady: false,
    })
    expect(step(res.steps, "presence").status).toBe("failed")
    expect(step(res.steps, "presence").retryable).toBe(true)
    expect(step(res.steps, "payout").status).toBe("failed")
    expect(step(res.steps, "tassadar").status).toBe("failed")
    expect(res.hasRetryableFailure).toBe(true)
  })
})
