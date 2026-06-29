import type {
  DecisionRecord,
  SessionEvent,
} from "@openagentsinc/autopilot-control-protocol"

export type NotificationKind =
  | "decision_requested"
  | "completed"
  | "failed"
  | "decision_resolved"
  | "session_update"

export type NotificationPayload = {
  kind: NotificationKind
  title: string
  sessionRef: string
  detailRef?: string
  decisionRef?: string
}

export type DecisionRequestNotificationInput = Pick<
  DecisionRecord,
  "requestId" | "actionRef"
> & {
  sessionRef: string
}

export type NotificationInput = SessionEvent | DecisionRequestNotificationInput

export type QuietHours = {
  enabled: boolean
  startHour: number
  endHour: number
}

export function projectNotification(input: NotificationInput): NotificationPayload {
  if (isDecisionRequestInput(input)) {
    return {
      kind: "decision_requested",
      title: "Decision requested",
      sessionRef: input.sessionRef,
      detailRef: input.actionRef,
      decisionRef: input.requestId,
    }
  }

  const base = {
    sessionRef: input.sessionRef,
    ...(input.detailRef ? { detailRef: input.detailRef } : {}),
  }

  switch (input.phase) {
    case "decision_requested":
      return {
        ...base,
        kind: "decision_requested",
        title: "Decision requested",
        ...(input.detailRef ? { decisionRef: input.detailRef } : {}),
      }
    case "completed":
      return {
        ...base,
        kind: "completed",
        title: "Session completed",
      }
    case "failed":
      return {
        ...base,
        kind: "failed",
        title: "Session failed",
      }
    case "decision_resolved":
      return {
        ...base,
        kind: "decision_resolved",
        title: "Decision resolved",
        ...(input.detailRef ? { decisionRef: input.detailRef } : {}),
      }
    default:
      return {
        ...base,
        kind: "session_update",
        title: "Session updated",
      }
  }
}

export function shouldDeliver(
  payload: NotificationPayload,
  now: Date,
  quietHours: QuietHours,
): boolean {
  if (payload.kind === "decision_requested") return true
  if (!quietHours.enabled) return true
  if (!isValidHour(quietHours.startHour) || !isValidHour(quietHours.endHour)) return true
  if (quietHours.startHour === quietHours.endHour) return true

  return !isWithinQuietHours(now.getHours(), quietHours)
}

function isDecisionRequestInput(
  input: NotificationInput,
): input is DecisionRequestNotificationInput {
  return "requestId" in input && "actionRef" in input
}

function isWithinQuietHours(
  hour: number,
  quietHours: Pick<QuietHours, "startHour" | "endHour">,
): boolean {
  const { startHour, endHour } = quietHours

  if (startHour < endHour) {
    return hour >= startHour && hour < endHour
  }

  return hour >= startHour || hour < endHour
}

function isValidHour(hour: number): boolean {
  return Number.isInteger(hour) && hour >= 0 && hour <= 23
}
