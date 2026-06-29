export type ProviderMode = "offline" | "online"

export type ProviderAvailabilityInput = {
  mode: ProviderMode
  ownedQueueDepth: number
  maxConcurrent: number
  activeJobs: number
}

export type ProviderAvailability = {
  accepting: boolean
  reason: string
  freeSlots: number
}

export function evaluateProviderAvailability(
  input: ProviderAvailabilityInput,
): ProviderAvailability {
  const freeSlots = Math.max(0, input.maxConcurrent - input.activeJobs)

  if (input.mode !== "online") {
    return {
      accepting: false,
      reason: "provider is offline",
      freeSlots,
    }
  }

  if (input.ownedQueueDepth !== 0) {
    return {
      accepting: false,
      reason: "owned work is queued",
      freeSlots,
    }
  }

  if (input.activeJobs >= input.maxConcurrent) {
    return {
      accepting: false,
      reason: "provider has no free slots",
      freeSlots,
    }
  }

  return {
    accepting: true,
    reason: "provider is online with spare capacity",
    freeSlots,
  }
}
