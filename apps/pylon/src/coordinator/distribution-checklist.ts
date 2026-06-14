export type DistributionTarget = "desktop" | "mobile" | "ota"

export type DistributionReadinessInput = {
  target: DistributionTarget
  signed?: boolean
  notarized?: boolean
  artifactPublished?: boolean
  bsdiffAvailable?: boolean
  desktopFeedPublished?: boolean
  testflightUploaded?: boolean
  storeSubmitted?: boolean
  otaPublished?: boolean
}

export type DistributionReadinessStep = {
  name: string
  done: boolean
}

export type DistributionReadiness = {
  ready: boolean
  missing: string[]
  steps: DistributionReadinessStep[]
}

export function evaluateDistributionReadiness(
  input: DistributionReadinessInput,
): DistributionReadiness {
  const stepsByTarget: Record<
    DistributionTarget,
    DistributionReadinessStep[]
  > = {
    desktop: [
      { name: "signed", done: input.signed === true },
      { name: "notarized", done: input.notarized === true },
      { name: "artifactPublished", done: input.artifactPublished === true },
      { name: "bsdiffAvailable", done: input.bsdiffAvailable === true },
      { name: "desktopFeedPublished", done: input.desktopFeedPublished === true },
    ],
    mobile: [
      { name: "testflightUploaded", done: input.testflightUploaded === true },
      { name: "storeSubmitted", done: input.storeSubmitted === true },
    ],
    ota: [{ name: "otaPublished", done: input.otaPublished === true }],
  }

  const steps = stepsByTarget[input.target]
  const missing = steps.filter((step) => !step.done).map((step) => step.name)

  return {
    ready: missing.length === 0,
    missing,
    steps,
  }
}
