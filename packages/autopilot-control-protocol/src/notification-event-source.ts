import { buildNotification } from "./notification-dispatch.js"

export type NotificationSession = {
  sessionRef: string
  state: string
  latestActivity?: string
}

export type SessionNotification = {
  sessionRef: string
  title: string
  body: string
  priority: string
}

export type SessionNotificationResult = {
  new: SessionNotification[]
  seenRefs: string[]
}

const NOTIFY_WORTHY_STATES = new Set(["failed", "completed", "needs_decision"])

function seenKey(sessionRef: string, state: string): string {
  return `${sessionRef}:${state}`
}

function messageForSession(session: NotificationSession): string {
  return session.latestActivity ?? `Session ${session.state}`
}

export function notificationsFromSessions(
  sessions: NotificationSession[],
  seenRefs: string[],
): SessionNotificationResult {
  const nextSeenRefs = [...seenRefs]
  const seen = new Set(seenRefs)
  const notifications: SessionNotification[] = []

  for (const session of sessions) {
    if (!NOTIFY_WORTHY_STATES.has(session.state)) {
      continue
    }

    const key = seenKey(session.sessionRef, session.state)

    if (seen.has(key)) {
      continue
    }

    const notification = buildNotification({
      phase: session.state,
      sessionRef: session.sessionRef,
      messageText: messageForSession(session),
    })

    seen.add(key)
    nextSeenRefs.push(key)

    if (!notification.shouldNotify) {
      continue
    }

    notifications.push({
      sessionRef: session.sessionRef,
      title: notification.title,
      body: notification.body,
      priority: notification.priority,
    })
  }

  return {
    new: notifications,
    seenRefs: nextSeenRefs,
  }
}
