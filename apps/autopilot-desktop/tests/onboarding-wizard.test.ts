import { describe, expect, test } from "bun:test"
import { initialModel, Model } from "../src/ui/model"
import { view } from "../src/ui/view"
import { projectOnboardingStatus } from "../src/shared/onboarding-status"
import type {
  IdentityChoiceStateResponse,
  OnboardingStatusResponse,
} from "../src/shared/rpc"

// AO-4 (#5445): wizard view rendering across onboarding states, including the
// identity-choice screen (AO-3) and a failed+retry path. We render the real
// Foldkit `view(model)` (a vnode Document) and walk the tree — the same
// approach as cl-53-sanitize.test.ts — so the wizard provably reflects state.

const treeContainsText = (node: unknown, text: string): boolean => {
  if (typeof node === "string") return node.includes(text)
  if (node === null || typeof node !== "object") return false
  const vnode = node as { children?: unknown[]; text?: unknown }
  if (typeof vnode.text === "string" && vnode.text.includes(text)) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some(child => treeContainsText(child, text))
    : false
}

const treeContainsClass = (node: unknown, className: string): boolean => {
  if (node === null || typeof node !== "object") return false
  const vnode = node as {
    children?: unknown[]
    data?: { class?: Record<string, boolean> }
  }
  if (vnode.data?.class?.[className]) return true
  return Array.isArray(vnode.children)
    ? vnode.children.some(child => treeContainsClass(child, className))
    : false
}

const onboardingModel = (over: {
  onboardingStatus?: OnboardingStatusResponse | null
  identityChoiceState?: IdentityChoiceStateResponse | null
  newIdentityName?: string
}): Model =>
  Model.make({
    ...initialModel,
    pane: "onboarding",
    onboardingStatus: over.onboardingStatus ?? null,
    identityChoiceState: over.identityChoiceState ?? null,
    newIdentityName: over.newIdentityName ?? "",
  })

const freshStatus = projectOnboardingStatus({
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
})

const failedStatus = projectOnboardingStatus({
  fetchedAt: "2026-06-18T00:00:00.000Z",
  identityChoiceMade: true,
  identityLabel: "new: Studio",
  agentRegistered: false,
  nodeLaunchStatus: "failed",
  localPylonReady: false,
  onboardingEnvConfigured: false,
  walletReceiveReady: false,
  walletBalanceSats: null,
  openAssignmentCount: 0,
})

const earningStatus = projectOnboardingStatus({
  fetchedAt: "2026-06-18T00:00:00.000Z",
  identityChoiceMade: true,
  identityLabel: "new: Studio",
  agentRegistered: true,
  nodeLaunchStatus: "online",
  localPylonReady: true,
  onboardingEnvConfigured: true,
  walletReceiveReady: true,
  walletBalanceSats: 4200,
  openAssignmentCount: 1,
})

describe("onboarding wizard view (AO-4)", () => {
  test("renders every step label of the chain", () => {
    const doc = view(onboardingModel({ onboardingStatus: freshStatus }))
    for (const label of [
      "Identity",
      "Agent registered",
      "Node online",
      "Wallet receive-ready",
      "Payout target registered",
      "Presence live",
      "Joined Tassadar",
      "First work claimed",
      "First sats earned",
    ]) {
      expect(treeContainsText(doc.body, label)).toBe(true)
    }
  })

  test("a failed step renders a blocked tone + a Retry affordance (no dead end)", () => {
    const doc = view(onboardingModel({ onboardingStatus: failedStatus }))
    // The node-online failure shows a retry message and a blocked-tone row.
    expect(treeContainsText(doc.body, "Retry")).toBe(true)
    expect(treeContainsClass(doc.body, "readiness-blocked")).toBe(true)
    // Crucially not blank: the chain steps still render.
    expect(treeContainsText(doc.body, "Node online")).toBe(true)
  })

  test("complete chain shows the earned step done + earning summary", () => {
    const doc = view(onboardingModel({ onboardingStatus: earningStatus }))
    expect(treeContainsText(doc.body, "Earned 4200 sats")).toBe(true)
    expect(treeContainsClass(doc.body, "readiness-ready")).toBe(true)
  })

  test("identity choice screen: existing detected offers BOTH options", () => {
    const choice: IdentityChoiceStateResponse = {
      choiceNeeded: true,
      detected: {
        present: true,
        shortLabel: "pylon.ab12cd",
        npub: "npub1existing",
        pylonRef: "pylon.ab12cd",
        source: "discovered_openagents_pylon",
      },
      chosen: null,
      createNewAvailable: true,
    }
    const doc = view(
      onboardingModel({ onboardingStatus: freshStatus, identityChoiceState: choice }),
    )
    expect(treeContainsText(doc.body, "Use existing identity")).toBe(true)
    expect(treeContainsText(doc.body, "pylon.ab12cd")).toBe(true)
    // Create-new is ALWAYS available, even with an existing Pylon detected.
    expect(treeContainsText(doc.body, "Create new identity")).toBe(true)
  })

  test("identity choice screen: fresh machine defaults to create-new only", () => {
    const choice: IdentityChoiceStateResponse = {
      choiceNeeded: true,
      detected: { present: false, shortLabel: null, npub: null, pylonRef: null, source: null },
      chosen: null,
      createNewAvailable: true,
    }
    const doc = view(
      onboardingModel({ onboardingStatus: freshStatus, identityChoiceState: choice }),
    )
    expect(treeContainsText(doc.body, "Create new identity")).toBe(true)
    // No "use existing" button when nothing is detected.
    expect(treeContainsText(doc.body, "Use existing identity")).toBe(false)
  })

  test("once a choice is made, the choice screen is gone and the chain shows", () => {
    const choice: IdentityChoiceStateResponse = {
      choiceNeeded: false,
      detected: { present: false, shortLabel: null, npub: null, pylonRef: null, source: null },
      chosen: { kind: "create_new", displayName: "Studio" },
      createNewAvailable: true,
    }
    const doc = view(
      onboardingModel({ onboardingStatus: earningStatus, identityChoiceState: choice }),
    )
    expect(treeContainsText(doc.body, "Choose your identity")).toBe(false)
    expect(treeContainsText(doc.body, "First sats earned")).toBe(true)
  })
})
