// #5003 quiet-hours notification policy (pure — no expo import, so it's unit
// testable). During the configured local-time window, only high-priority
// notifications (decision_required) fire; low/normal summaries (completed/
// failed) are suppressed. Built on the shared inQuietHours core.

import { inQuietHours } from "@openagentsinc/autopilot-control-protocol"

export type QuietHoursWindow = { startHour: number; endHour: number }

// Default quiet window: 22:00–07:00 local. decision_required still wakes you.
export const DEFAULT_QUIET_HOURS: QuietHoursWindow = { startHour: 22, endHour: 7 }

export type FireableNotification = { priority: string }

// Filter the to-fire notifications by the quiet-hours policy. `window === null`
// disables quiet hours (everything fires). `nowHour` is the local hour (0–23).
export function selectNotificationsToFire<T extends FireableNotification>(
  items: T[],
  window: QuietHoursWindow | null,
  nowHour: number,
): T[] {
  if (window === null) return items.slice()
  const quiet = inQuietHours({ hour: nowHour, startHour: window.startHour, endHour: window.endHour })
  if (!quiet) return items.slice()
  return items.filter((n) => n.priority === "high")
}
