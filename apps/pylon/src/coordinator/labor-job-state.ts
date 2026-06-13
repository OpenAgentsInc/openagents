export type LaborJobState =
  | "quoted"
  | "accepted"
  | "in_progress"
  | "delivered"
  | "settled"
  | "cancelled"

type LaborJobEvent = "accept" | "start" | "deliver" | "settle" | "cancel"

type LaborJobAdvanceResult = {
  next: LaborJobState
  accepted: boolean
  reason: string
}

const legalTransitions: Record<
  Exclude<LaborJobEvent, "cancel">,
  Partial<Record<LaborJobState, LaborJobState>>
> = {
  accept: { quoted: "accepted" },
  start: { accepted: "in_progress" },
  deliver: { in_progress: "delivered" },
  settle: { delivered: "settled" },
}

export function advanceLaborJob(
  current: LaborJobState,
  event: LaborJobEvent,
): LaborJobAdvanceResult {
  if (event === "cancel") {
    if (current === "settled" || current === "cancelled") {
      return {
        next: current,
        accepted: false,
        reason: `cannot cancel ${current} labor job`,
      }
    }

    return {
      next: "cancelled",
      accepted: true,
      reason: `cancelled ${current} labor job`,
    }
  }

  const next = legalTransitions[event][current]
  if (!next) {
    return {
      next: current,
      accepted: false,
      reason: `illegal ${event} from ${current}`,
    }
  }

  return {
    next,
    accepted: true,
    reason: `advanced ${current} to ${next}`,
  }
}
