export type NotificationKind =
  | "decision_required"
  | "session_failed"
  | "session_completed"
  | "attention"

export type NotificationPriority = "high" | "normal"

export type NotificationPayload = {
  kind: NotificationKind
  title: string
  body: string
  sessionRef?: string
  observedAt: string
  priority: NotificationPriority
}

export type NotificationProjectionEvent = {
  type: string
  observedAt: string
  sessionRef?: string
  title?: string
  body?: string
  message?: string
  reason?: string
  requestRef?: string
  decisionRef?: string
}

export function projectNotification(input: NotificationProjectionEvent): NotificationPayload {
  const kind = notificationKind(input)
  if (kind === null) {
    throw new Error(`unsupported notification event: ${input.type}`)
  }

  return withSessionRef({
    kind,
    title: notificationTitle(kind, input),
    body: notificationBody(kind, input),
    observedAt: input.observedAt,
    priority: notificationPriority(kind),
  }, input.sessionRef)
}

export function shouldNotify(event: NotificationProjectionEvent): boolean {
  return notificationKind(event) !== null
}

function notificationKind(event: Pick<NotificationProjectionEvent, "type">): NotificationKind | null {
  switch (event.type) {
    case "decision_required":
    case "decision.requested":
    case "decision_required.requested":
      return "decision_required"
    case "session_failed":
    case "session.failed":
    case "failed":
      return "session_failed"
    case "session_completed":
    case "session.completed":
    case "completed":
      return "session_completed"
    case "attention":
    case "attention.required":
      return "attention"
    default:
      return null
  }
}

function notificationPriority(kind: NotificationKind | null): NotificationPriority {
  return kind === "decision_required" || kind === "session_failed" ? "high" : "normal"
}

function notificationTitle(kind: NotificationKind | null, input: NotificationProjectionEvent): string {
  if (input.title) return input.title

  switch (kind) {
    case "decision_required":
      return "Decision required"
    case "session_failed":
      return "Session failed"
    case "session_completed":
      return "Session completed"
    case "attention":
      return "Attention required"
    default:
      return "Notification"
  }
}

function notificationBody(kind: NotificationKind | null, input: NotificationProjectionEvent): string {
  if (input.body) return input.body
  if (input.message) return input.message
  if (input.reason) return input.reason

  switch (kind) {
    case "decision_required":
      return input.decisionRef ?? input.requestRef ?? "A session needs an operator decision."
    case "session_failed":
      return "The session failed before completion."
    case "session_completed":
      return "The session completed successfully."
    case "attention":
      return "A session needs attention."
    default:
      return "A node event was observed."
  }
}

function withSessionRef(
  payload: Omit<NotificationPayload, "sessionRef">,
  sessionRef: string | undefined,
): NotificationPayload {
  return sessionRef === undefined ? payload : { ...payload, sessionRef }
}
