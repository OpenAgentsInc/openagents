// Per-user push notification preference (MM-G2, #8486). Deliberately just a
// global on/off toggle for the MVP, per the issue's "at minimum a global
// toggle" bar — a future per-event-kind granular preference table can be
// added without migrating this one (it would live alongside, not replace it).

export type PushNotificationPreference = Readonly<{
  userId: string
  pushEnabled: boolean
  updatedAt: string
}>

type PushNotificationPreferenceRow = Readonly<{
  user_id: string
  push_enabled: number
  updated_at: string
}>

/** No stored row == enabled (opt-out, not opt-in) — matches this repo's
 * default posture for other notification-adjacent surfaces. */
export const readPushNotificationPreference = async (
  db: D1Database,
  userId: string,
): Promise<PushNotificationPreference> => {
  const row = await db
    .prepare(
      `SELECT user_id, push_enabled, updated_at FROM push_notification_preferences WHERE user_id = ?`,
    )
    .bind(userId)
    .first<PushNotificationPreferenceRow>()

  if (row === null) {
    return { pushEnabled: true, updatedAt: '', userId }
  }

  return { pushEnabled: row.push_enabled === 1, updatedAt: row.updated_at, userId }
}

export const writePushNotificationPreference = async (
  db: D1Database,
  input: Readonly<{ userId: string; pushEnabled: boolean; nowIso: string }>,
): Promise<PushNotificationPreference> => {
  await db
    .prepare(
      `INSERT INTO push_notification_preferences (user_id, push_enabled, created_at, updated_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT (user_id) DO UPDATE SET
         push_enabled = excluded.push_enabled,
         updated_at = excluded.updated_at`,
    )
    .bind(input.userId, input.pushEnabled ? 1 : 0, input.nowIso, input.nowIso)
    .run()

  return { pushEnabled: input.pushEnabled, updatedAt: input.nowIso, userId: input.userId }
}
