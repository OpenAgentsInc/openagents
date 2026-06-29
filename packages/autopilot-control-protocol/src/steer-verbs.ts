export type SteerState = "running" | "paused" | "cancelled"
export type SteerVerb = "pause" | "resume" | "interrupt" | "cancel"

export type SteerVerbResult = {
  state: SteerState
  accepted: boolean
  reason: string
}

function rejected(state: SteerState, reason: string): SteerVerbResult {
  return {
    state,
    accepted: false,
    reason,
  }
}

export function applySteerVerb(state: SteerState, verb: SteerVerb): SteerVerbResult {
  if (state === "cancelled") {
    return rejected(state, "session_cancelled")
  }

  if (verb === "pause") {
    if (state !== "running") return rejected(state, "pause_requires_running")

    return {
      state: "paused",
      accepted: true,
      reason: "paused",
    }
  }

  if (verb === "resume") {
    if (state !== "paused") return rejected(state, "resume_requires_paused")

    return {
      state: "running",
      accepted: true,
      reason: "resumed",
    }
  }

  if (verb === "interrupt") {
    return {
      state: "running",
      accepted: true,
      reason: "interrupt_injected",
    }
  }

  return {
    state: "cancelled",
    accepted: true,
    reason: "cancelled",
  }
}
