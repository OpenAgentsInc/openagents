import type { ShipMode } from "./ship-mode.js"

export type LocalBuildPlanInput = {
  shipMode: "rebuild"
  platform: "ios"
  profile: string
  outPath: string
}

export type LocalBuildPlanStep = {
  name: string
  argv: string[]
}

export type LocalBuildPlan = {
  steps: LocalBuildPlanStep[]
}

export function requireRebuild(shipMode: ShipMode): asserts shipMode is "rebuild" {
  if (shipMode !== "rebuild") {
    throw new Error(`Local build plan requires rebuild ship mode; received ${shipMode}.`)
  }
}

export function buildLocalBuildPlan(input: LocalBuildPlanInput): LocalBuildPlan {
  requireRebuild(input.shipMode)

  return {
    steps: [
      {
        name: "bundle pre-check",
        argv: ["expo", "export", "--platform", input.platform],
      },
      {
        name: "local build",
        argv: [
          "eas",
          "build",
          "--platform",
          input.platform,
          "--profile",
          input.profile,
          "--local",
          "--non-interactive",
          "--output",
          input.outPath,
        ],
      },
      {
        name: "submit",
        argv: [
          "eas",
          "submit",
          "--platform",
          input.platform,
          "--path",
          input.outPath,
          "--non-interactive",
        ],
      },
    ],
  }
}
