import {
  buildNotificationCenter,
  notificationsFromSessions,
  type NotificationCenterInputItem,
  type NotificationCenterView,
  type NotificationSession,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

// CL-30 (desktop): turn newly notify-worthy Pylon sessions into native OS
// notifications + an in-app notification center. The derivation lives in the
// shared cores (`notificationsFromSessions` + `buildNotificationCenter`); this
// module only tracks the seen-set across polls, raises the OS notification, and
// accumulates the center feed. It observes — it never mutates session state.

export type RaiseOsNotification = (notification: {
  readonly title: string
  readonly body: string
  readonly priority: string
}) => void

export type SessionNotifier = {
  // Fold a fresh session list into the notifier, raising OS notifications for
  // any session that newly entered a notify-worthy state, and return the
  // current in-app notification center view.
  ingest(sessions: readonly SessionSummary[]): NotificationCenterView
  view(): NotificationCenterView
}

function toNotificationSession(session: SessionSummary): NotificationSession {
  return {
    sessionRef: session.sessionRef,
    state: session.state,
    ...(session.latestActivity !== undefined
      ? { latestActivity: session.latestActivity }
      : {}),
  }
}

export function createSessionNotifier(input: {
  raise: RaiseOsNotification
  now?: () => string
  // Bound the in-app feed so a long-lived desktop session does not grow it
  // without limit. The center view already sorts/highlights; this is storage.
  maxItems?: number
}): SessionNotifier {
  const now = input.now ?? (() => new Date().toISOString())
  const maxItems = input.maxItems ?? 100
  let seenRefs: string[] = []
  const items: NotificationCenterInputItem[] = []

  function ingest(sessions: readonly SessionSummary[]): NotificationCenterView {
    const result = notificationsFromSessions(
      sessions.map(toNotificationSession),
      seenRefs,
    )
    seenRefs = result.seenRefs

    for (const notification of result.new) {
      try {
        input.raise({
          title: notification.title,
          body: notification.body,
          priority: notification.priority,
        })
      } catch {
        // A failed OS notification must never break polling or the feed.
      }
      items.push({
        sessionRef: notification.sessionRef,
        title: notification.title,
        body: notification.body,
        priority: notification.priority as NotificationCenterInputItem["priority"],
        at: now(),
      })
    }

    if (items.length > maxItems) items.splice(0, items.length - maxItems)

    return buildNotificationCenter(items)
  }

  return {
    ingest,
    view() {
      return buildNotificationCenter(items)
    },
  }
}
