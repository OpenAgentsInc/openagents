export type OtaPublishEligibilityInput = {
  currentFingerprint: string
  lastPublishedFingerprint: string | null
  hasJsChanges: boolean
  hasNativeChanges: boolean
}

export type OtaPublishEligibilityDecision = {
  publish: boolean
  mode: "ota" | "rebuild" | "noop"
  reason: string
}

export function decideOtaPublish(input: OtaPublishEligibilityInput): OtaPublishEligibilityDecision {
  if (input.hasNativeChanges) {
    return {
      publish: false,
      mode: "rebuild",
      reason: "native changes require a rebuild",
    }
  }

  if (input.lastPublishedFingerprint !== input.currentFingerprint) {
    return {
      publish: false,
      mode: "rebuild",
      reason: "current native fingerprint has not been published",
    }
  }

  if (!input.hasJsChanges) {
    return {
      publish: false,
      mode: "noop",
      reason: "no JavaScript changes to publish",
    }
  }

  return {
    publish: true,
    mode: "ota",
    reason: "JavaScript changes are OTA-eligible for the published native fingerprint",
  }
}
