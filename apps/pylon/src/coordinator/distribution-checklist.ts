export type DistributionTarget = "desktop" | "mobile" | "ota"

export type DistributionReadinessInput = {
  target: DistributionTarget
  signed?: boolean
  notarized?: boolean
  bsdiffAvailable?: boolean
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
      { name: "bsdiffAvailable", done: input.bsdiffAvailable === true },
    ],
    mobile: [{ name: "storeSubmitted", done: input.storeSubmitted === true }],
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
