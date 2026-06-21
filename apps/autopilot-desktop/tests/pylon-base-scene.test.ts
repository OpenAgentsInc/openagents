import { describe, expect, test } from "bun:test"

import {
  PAYMENT_PARTICLE_GOLD,
  type ChatWorldPylonScene,
  type PaymentParticle,
  pylonGrowthTier,
} from "../src/shared/chat-world-scene"
import {
  PYLON_BASE_IDENTITY_MISSING_BLOCKER,
  PYLON_BASE_NODE_ID,
  projectPylonBase,
  withPylonBaseLayer,
} from "../src/shared/pylon-base-scene"
import type { OnboardingStatusResponse } from "../src/shared/onboarding-status"
import type {
  IdentityChoiceStateResponse,
  TrainingOperatorReadinessResponse,
} from "../src/shared/rpc"
import { VERSE_TASSADAR_CORE_NODE_ID } from "../src/shared/verse-training-visualization"

const identity = (
  pylonRef: string | null,
): IdentityChoiceStateResponse => ({
  choiceNeeded: false,
  detected: {
    present: pylonRef !== null,
    shortLabel: pylonRef,
    npub: null,
    pylonRef,
    source: pylonRef === null ? null : "discovered_openagents_pylon",
  },
  chosen: pylonRef === null ? null : { kind: "use_existing", displayName: null },
  createNewAvailable: true,
})

const readiness = (
  overrides: Partial<TrainingOperatorReadinessResponse> = {},
): TrainingOperatorReadinessResponse => ({
  ok: true,
  fetchedAt: "2026-06-20T00:00:00.000Z",
  sourceUrl: "desktop:training-operator-readiness",
  trainingBaseUrl: "https://openagents.test",
  adminEnabled: false,
  adminTokenPresent: false,
  adminReady: false,
  leaseEnabled: true,
  leaseReady: true,
  pylonRefPresent: true,
  pylonRefSource: "identity",
  pylonRef: "pylon.local",
  pylonHomePresent: true,
  controlTokenPresent: true,
  localPylonReady: true,
  evidenceEnabled: true,
  evidencePacketPathPresent: true,
  evidenceReady: true,
  blockerRefs: [],
  ...overrides,
})

const onboarding = (
  statuses: Record<string, OnboardingStatusResponse["steps"][number]["status"]> = {},
): OnboardingStatusResponse => ({
  ok: true,
  fetchedAt: "2026-06-20T00:00:00.000Z",
  sourceUrl: "desktop:onboarding-status",
  complete: false,
  currentStepId: null,
  hasRetryableFailure: false,
  steps: Object.entries(statuses).map(([id, status]) => ({
    id,
    label: id,
    status,
    message: id,
    retryable: false,
  })),
})

const scene = (
  overrides: Partial<ChatWorldPylonScene> = {},
): ChatWorldPylonScene => ({
  empty: false,
  onlineNow: 2,
  nodes: [
    {
      id: "local",
      label: "studio",
      state: "assignment_ready",
      color: 0x4ade80,
      online: true,
      pulseSpeed: 1.8,
      products: ["pylon.local", "compute"],
    },
    {
      id: "remote",
      label: "elsewhere",
      state: "wallet_ready",
      color: 0x7dd3fc,
      online: true,
      pulseSpeed: 0.4,
      products: ["pylon.remote"],
    },
  ],
  growth: pylonGrowthTier(50_000),
  asOfLabel: "moments ago",
  ...overrides,
})

const particle = (overrides: Partial<PaymentParticle> = {}): PaymentParticle => ({
  id: "evt-1",
  fromRef: "pylon:local",
  toRef: "pylon:buyer",
  amountSats: 1_000,
  realBitcoinMoved: true,
  color: PAYMENT_PARTICLE_GOLD,
  size: 0.5,
  sourceRefs: ["receipt:local"],
  ts: "2026-06-20T00:00:00.000Z",
  text: null,
  ...overrides,
})

describe("projectPylonBase", () => {
  test("identity present yields a distinct base node connected to Tassadar", () => {
    const projection = projectPylonBase({
      chatWorldScene: scene(),
      identityChoice: identity("pylon.local"),
      onboardingStatus: onboarding(),
      particles: [],
      trainingOperatorReadiness: readiness(),
    })

    expect(projection.pylonRef).toBe("pylon.local")
    expect(projection.status).toBe("assignment_ready")
    expect(projection.matchedFleetNodeId).toBe("local")

    const options = withPylonBaseLayer(
      {
        nodes: [
          {
            id: VERSE_TASSADAR_CORE_NODE_ID,
            label: "Tassadar",
            role: "run",
            status: "active",
            position: [0, 0, 0],
          },
        ],
      },
      projection,
    )
    const baseNode = options.nodes?.find((node) => node.id === PYLON_BASE_NODE_ID)
    expect(baseNode).toBeDefined()
    expect(baseNode?.label).toBe("My Pylon Base · studio")
    expect(baseNode?.connectedTo).toEqual([VERSE_TASSADAR_CORE_NODE_ID])
    expect(baseNode?.detail).toContain("pylon.local")
  })

  test("missing identity returns one blocker and one next action", () => {
    const projection = projectPylonBase({
      chatWorldScene: null,
      identityChoice: identity(null),
      onboardingStatus: onboarding(),
      particles: [],
      trainingOperatorReadiness: readiness({
        pylonRefPresent: false,
        pylonRefSource: "missing",
        pylonRef: null,
        localPylonReady: false,
      }),
    })

    expect(projection.status).toBe("missing")
    expect(projection.blockerRefs).toEqual([PYLON_BASE_IDENTITY_MISSING_BLOCKER])
    expect(projection.nextAction).toBe("Choose or create a Pylon identity")
    expect(projection.mana).toEqual({ current: 0, total: 5, ratio: 0 })
  })

  test("readiness and mana come from local/public state", () => {
    const projection = projectPylonBase({
      chatWorldScene: scene({
        nodes: [
          {
            id: "local",
            label: "studio",
            state: "wallet_ready",
            color: 0x7dd3fc,
            online: true,
            pulseSpeed: 0.8,
            products: ["pylon.local"],
          },
        ],
      }),
      identityChoice: identity("pylon.local"),
      onboardingStatus: onboarding({ presence: "done", claimed: "active" }),
      particles: [],
      trainingOperatorReadiness: readiness(),
    })

    expect(projection.readiness).toEqual({
      identityPresent: true,
      online: true,
      presence: true,
      walletReady: true,
      assignmentReady: true,
      localPylonReady: true,
    })
    expect(projection.mana).toEqual({ current: 4, total: 5, ratio: 0.8 })
    expect(projection.nextAction).toBeNull()
  })

  test("splits fleet growth from receipt-backed my-Pylon growth", () => {
    const projection = projectPylonBase({
      chatWorldScene: scene(),
      identityChoice: identity("pylon.local"),
      onboardingStatus: onboarding(),
      particles: [
        particle(),
        particle({
          id: "evt-remote",
          fromRef: "pylon:remote",
          amountSats: 99_000,
          sourceRefs: ["receipt:remote"],
        }),
        particle({
          id: "evt-no-ref",
          amountSats: 5_000,
          sourceRefs: [],
        }),
      ],
      trainingOperatorReadiness: readiness(),
    })

    expect(projection.fleetGrowth.settledSats).toBe(50_000)
    expect(projection.settledSats).toBe(1_000)
    expect(projection.growth.settledSats).toBe(1_000)
    expect(projection.sourceRefs).toEqual(["receipt:local"])
  })
})
