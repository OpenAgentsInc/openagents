import { availableSteerVerbs, type AvailableSteerVerb, type SteerAvailabilityState } from "./steer-availability.js"

export type SessionActionsInput = {
  state: string
  nodeSupports: string[]
}

export type SessionActionsAvailability = {
  cancel: boolean
  pause: boolean
  resume: boolean
  interrupt: boolean
  reasons: Record<string, string>
}

const actionOrder: AvailableSteerVerb[] = ["cancel", "pause", "resume", "interrupt"]

const stateReasons: Record<AvailableSteerVerb, string> = {
  pause: "only running sessions can be paused",
  resume: "only paused sessions can be resumed",
  interrupt: "only running or paused sessions can be interrupted",
  cancel: "only running or paused sessions can be cancelled",
}

export function sessionActions(input: SessionActionsInput): SessionActionsAvailability {
  const availability = availableSteerVerbs({
    nodeSupports: input.nodeSupports,
    state: input.state as SteerAvailabilityState,
  })
  const available = new Set(availability.verbs)
  const supported = new Set(input.nodeSupports)
  const reasons: Record<string, string> = {}

  for (const action of actionOrder) {
    if (available.has(action)) continue

    reasons[action] = supported.has(action)
      ? stateReasons[action]
      : `node does not support ${action} yet`
  }

  return {
    cancel: available.has("cancel"),
    pause: available.has("pause"),
    resume: available.has("resume"),
    interrupt: available.has("interrupt"),
    reasons,
  }
}
