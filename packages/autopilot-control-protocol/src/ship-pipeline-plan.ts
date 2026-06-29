import { planEasBuild } from "./eas-build-plan.js"
import { decideOtaPublish } from "./ota-publish-eligibility.js"

export type ShipPipelinePlanInput = {
  currentFingerprint: string
  lastPublishedFingerprint: string | null
  hasJsChanges: boolean
  hasNativeChanges: boolean
  platform: "ios" | "android"
  autoSubmit: boolean
}

export type ShipPipelinePlan = {
  action: "ota" | "rebuild" | "noop"
  steps: string[]
  reason: string
}

export function planShipPipeline(input: ShipPipelinePlanInput): ShipPipelinePlan {
  const otaDecision = decideOtaPublish({
    currentFingerprint: input.currentFingerprint,
    lastPublishedFingerprint: input.lastPublishedFingerprint,
    hasJsChanges: input.hasJsChanges,
    hasNativeChanges: input.hasNativeChanges,
  })

  if (otaDecision.mode === "noop") {
    return {
      action: "noop",
      steps: [],
      reason: otaDecision.reason,
    }
  }

  const buildPlan = planEasBuild({
    mode: otaDecision.mode,
    platform: input.platform,
    autoSubmit: input.autoSubmit,
  })

  return {
    action: otaDecision.mode,
    steps: otaDecision.mode === "ota"
      ? ["apps/oa-updates/scripts/publish-ota.sh"]
      : buildPlan.steps,
    reason: otaDecision.reason,
  }
}
