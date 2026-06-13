export type SteerAvailabilityState =
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled"

export type AvailableSteerVerb = "pause" | "resume" | "interrupt" | "cancel"

export type SteerAvailabilityInput = {
  nodeSupports: string[]
  state: SteerAvailabilityState
}

export type SteerAvailability = {
  verbs: AvailableSteerVerb[]
  reason: string
}

const stateVerbs: Record<SteerAvailabilityState, AvailableSteerVerb[]> = {
  running: ["pause", "interrupt", "cancel"],
  paused: ["resume", "interrupt", "cancel"],
  completed: [],
  failed: [],
  cancelled: [],
}

const verbOrder: AvailableSteerVerb[] = ["pause", "resume", "interrupt", "cancel"]

export function availableSteerVerbs(input: SteerAvailabilityInput): SteerAvailability {
  const supported = new Set(input.nodeSupports)
  const validForState = new Set(stateVerbs[input.state])
  const verbs = verbOrder.filter((verb) => supported.has(verb) && validForState.has(verb))

  return {
    verbs,
    reason: `state:${input.state};nodeSupports:${input.nodeSupports.join(",")}`,
  }
}
