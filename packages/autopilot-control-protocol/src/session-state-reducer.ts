export type SessionState = "running" | "completed" | "failed" | "cancelled" | "idle"

export type SessionPhaseEvent = {
  phase: string
}

export type SessionStateReduction = {
  state: SessionState
  lastPhase: string
}

const terminalStateByPhase = {
  failed: "failed",
  cancelled: "cancelled",
  completed: "completed",
} as const

export function reduceSessionState(events: SessionPhaseEvent[]): SessionStateReduction {
  let state: SessionState = "idle"
  let lastPhase = ""

  for (const event of events) {
    lastPhase = event.phase

    if (event.phase in terminalStateByPhase) {
      state = terminalStateByPhase[event.phase as keyof typeof terminalStateByPhase]
    } else if (state === "idle") {
      state = "running"
    }
  }

  return { state, lastPhase }
}
