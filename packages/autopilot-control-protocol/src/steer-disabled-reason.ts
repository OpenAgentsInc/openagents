import { availableSteerVerbs, type AvailableSteerVerb, type SteerAvailabilityState } from "./steer-availability.js"

export type SteerDisabledReasonVerb = "pause" | "resume" | "interrupt" | "cancel"

export type SteerDisabledReasonInput = {
  nodeSupports: string[]
  state: string
}

export type SteerDisabledReason = {
  enabled: boolean
  reason: string
}

const stateReasons: Record<SteerDisabledReasonVerb, string> = {
  pause: "only running sessions can be paused",
  resume: "only paused sessions can be resumed",
  interrupt: "only running or paused sessions can be interrupted",
  cancel: "only running sessions can be cancelled",
}

export function steerDisabledReason(
  verb: SteerDisabledReasonVerb,
  input: SteerDisabledReasonInput,
): SteerDisabledReason {
  const availability = availableSteerVerbs({
    nodeSupports: input.nodeSupports,
    state: input.state as SteerAvailabilityState,
  })

  if (availability.verbs.includes(verb as AvailableSteerVerb)) {
    return {
      enabled: true,
      reason: "available",
    }
  }

  if (!input.nodeSupports.includes(verb)) {
    return {
      enabled: false,
      reason: `node does not support ${verb} yet`,
    }
  }

  return {
    enabled: false,
    reason: stateReasons[verb],
  }
}
