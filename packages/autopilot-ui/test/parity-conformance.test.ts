import { describe, expect, test } from "bun:test"

Object.assign(globalThis, {
  window: {
    requestAnimationFrame: (callback: FrameRequestCallback): number => {
      callback(0)
      return 0
    },
  },
})

const autopilotUi = await import("../src/index")

const parityComponents = [
  "SessionList",
  "SessionRow",
  "SessionDetail",
  "SessionActions",
  "DecisionCard",
  "DecisionActions",
  "SteerControls",
  "AccountList",
  "VerifyStatus",
  "NodeStatusBadge",
  "ProviderStatusList",
  "ArtifactList",
  "ReceiptList",
  "AssignmentList",
  "EarningsPanel",
  "EventTimeline",
  "PublicActivityStrip",
] as const

describe("TUI parity conformance exports", () => {
  for (const componentName of parityComponents) {
    test(`${componentName} is exported from the barrel`, () => {
      expect(typeof autopilotUi[componentName]).toBe("function")
    })
  }
})
