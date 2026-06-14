// CL-30 mobile notifications. Local (on-device) notifications fired when a
// Pylon session newly enters a notify-worthy state (needs_decision / failed /
// completed). Uses expo-notifications for the OS notification + permission, and
// the shared protocol cores for the derive/permission logic (no APNs/push
// server needed — these are local notifications scheduled from JS).

import * as Notifications from "expo-notifications"
import {
  notificationsFromSessions,
  projectNotificationPermission,
} from "@openagentsinc/autopilot-control-protocol"

// Foreground notifications should still show a banner.
Notifications.setNotificationHandler({
  handleNotification: async () => ({
    shouldShowBanner: true,
    shouldShowList: true,
    shouldPlaySound: false,
    shouldSetBadge: false,
  }),
})

export type NotificationPermission = ReturnType<typeof projectNotificationPermission>

// Request OS permission and project it through the shared state model.
export async function ensureNotificationPermission(): Promise<NotificationPermission> {
  const current = await Notifications.getPermissionsAsync()
  let granted = current.granted
  let canAskAgain = current.canAskAgain
  if (!granted && canAskAgain) {
    const asked = await Notifications.requestPermissionsAsync()
    granted = asked.granted
    canAskAgain = asked.canAskAgain
  }
  return projectNotificationPermission({ granted, canAskAgain, pushToken: granted ? "local" : null })
}

// Fire a local OS notification immediately (trigger: null).
async function fireLocal(title: string, body: string): Promise<void> {
  try {
    await Notifications.scheduleNotificationAsync({ content: { title, body }, trigger: null })
  } catch {
    // never let a notification failure break the polling loop
  }
}

export type SessionLike = { sessionRef: string; state: string; latestActivity?: string }

// Given the latest session list + the refs already notified, fire OS
// notifications for newly notify-worthy sessions and return the updated seen
// set. Pure derive via the shared core; only the OS fire is a side effect.
export async function notifyNewSessionStates(
  sessions: SessionLike[],
  seenRefs: string[],
  permitted: boolean,
): Promise<string[]> {
  const result = notificationsFromSessions(
    sessions.map((s) => ({ sessionRef: s.sessionRef, state: s.state, latestActivity: s.latestActivity })),
    seenRefs,
  )
  if (permitted) {
    for (const n of result.new) {
      await fireLocal(n.title, n.body)
    }
  }
  return result.seenRefs
}
