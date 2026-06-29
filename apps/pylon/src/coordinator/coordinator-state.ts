export type CoordinatorStatus =
  | "received"
  | "planning"
  | "fanning_out"
  | "shipping"
  | "shipped"
  | "failed"

export type CoordinatorState = {
  intentId: string
  status: CoordinatorStatus
  failureReason?: string
}

export type CoordinatorEvent =
  | { type: "start_planning" }
  | { type: "fan_out" }
  | { type: "start_shipping" }
  | { type: "mark_shipped" }
  | { type: "mark_failed"; reason?: string }

const legalTransitions: Record<CoordinatorEvent["type"], readonly CoordinatorStatus[]> = {
  start_planning: ["received"],
  fan_out: ["planning"],
  start_shipping: ["fanning_out"],
  mark_shipped: ["shipping"],
  mark_failed: ["planning", "fanning_out", "shipping"],
}

function nextStatus(event: CoordinatorEvent): CoordinatorStatus {
  switch (event.type) {
    case "start_planning":
      return "planning"
    case "fan_out":
      return "fanning_out"
    case "start_shipping":
      return "shipping"
    case "mark_shipped":
      return "shipped"
    case "mark_failed":
      return "failed"
  }
}

export function initCoordinatorState(intentId: string): CoordinatorState {
  return { intentId, status: "received" }
}

export function transitionCoordinatorState(
  state: CoordinatorState,
  event: CoordinatorEvent,
): CoordinatorState {
  const allowedFrom = legalTransitions[event.type]
  if (!allowedFrom.includes(state.status)) {
    throw new Error(`illegal coordinator transition: ${state.status} -> ${nextStatus(event)}`)
  }

  const status = nextStatus(event)
  return {
    intentId: state.intentId,
    status,
    ...(event.type === "mark_failed" && event.reason !== undefined ? { failureReason: event.reason } : {}),
  }
}
