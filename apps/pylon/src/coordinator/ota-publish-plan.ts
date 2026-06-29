import type { ShipMode } from "./ship-mode.js"

export type OtaPublishPlanInput = {
  shipMode: "ota"
  branch: string
  platform: "ios" | "android"
  message: string
}

export type OtaPublishPlan = {
  argv: string[]
  receipt: {
    kind: "ota_publish"
    branch: string
    platform: "ios" | "android"
  }
}

export function requireOtaEligible(shipMode: ShipMode): void {
  if (shipMode !== "ota") {
    throw new Error(`OTA publish requires ship mode ota; received ${shipMode}`)
  }
}

export function buildOtaPublishPlan(input: OtaPublishPlanInput): OtaPublishPlan {
  requireOtaEligible(input.shipMode)

  return {
    argv: [
      "eas",
      "update",
      "--branch",
      input.branch,
      "--platform",
      input.platform,
      "--message",
      input.message,
      "--non-interactive",
      "--environment",
      input.branch,
    ],
    receipt: {
      kind: "ota_publish",
      branch: input.branch,
      platform: input.platform,
    },
  }
}
