export type NotificationPriority = "low" | "normal" | "high"

export type NotificationInput = {
  phase: string
  sessionRef: string
  messageText: string
}

export type NotificationDispatch = {
  shouldNotify: boolean
  title: string
  body: string
  priority: NotificationPriority
}

const HIGH_PRIORITY_PHASES = new Set(["needs_decision", "needs_approval", "failed"])
const SUPPRESSED_PHASES = new Set([
  "agent_message",
  "tool_use",
  "tool_result",
  "reasoning",
])

function normalizeText(value: string): string {
  return value.trim().replace(/\s+/g, " ")
}

function titleForPhase(phase: string): string {
  switch (phase) {
    case "needs_decision":
      return "Autopilot needs decision"
    case "needs_approval":
      return "Autopilot needs approval"
    case "failed":
      return "Autopilot failed"
    case "completed":
      return "Autopilot completed"
    default:
      return "Autopilot update"
  }
}

export function buildNotification(input: NotificationInput): NotificationDispatch {
  const phase = normalizeText(input.phase)
  const sessionRef = normalizeText(input.sessionRef)
  const messageText = normalizeText(input.messageText)
  const body = sessionRef ? `${sessionRef}: ${messageText}` : messageText

  if (HIGH_PRIORITY_PHASES.has(phase)) {
    return {
      shouldNotify: true,
      title: titleForPhase(phase),
      body,
      priority: "high",
    }
  }

  if (phase === "completed") {
    return {
      shouldNotify: true,
      title: titleForPhase(phase),
      body,
      priority: "normal",
    }
  }

  if (SUPPRESSED_PHASES.has(phase)) {
    return {
      shouldNotify: false,
      title: titleForPhase(phase),
      body,
      priority: "low",
    }
  }

  return {
    shouldNotify: false,
    title: titleForPhase(phase),
    body,
    priority: "low",
  }
}
