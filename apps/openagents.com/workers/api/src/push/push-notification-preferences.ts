// Per-user push notification preference (MM-G2, #8486). Deliberately just a
// global on/off toggle for the MVP, per the issue's "at minimum a global
// toggle" bar — a future per-event-kind granular preference table can be
// added without migrating this one (it would live alongside, not replace it).
//
// CFG-4 Domain 4 (#8519): Cloud SQL Postgres-AUTHORITATIVE (khala-sync-server
// migration 0044_push_tables_hard_cut.sql). The D1 code path is DELETED —
// reads/writes run through the generic `PaymentsLedgerDb` Hyperdrive executor
// (`payments-ledger-db.ts`). `push_enabled` is a smallint 0/1 in Postgres, so
// decode with `Number(...) === 1`.

import type { PaymentsLedgerDb } from '../payments-ledger-db'

/** The push-preference database handle. Postgres-authoritative in production
 * (`paymentsLedgerDbForEnv`); tests back it with the SQLite adapter in
 * `../test/payments-ledger-sqlite.ts`. */
export type PushNotificationPreferenceDb = PaymentsLedgerDb

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
  db: PushNotificationPreferenceDb,
  userId: string,
): Promise<PushNotificationPreference> => {
  const rows = await db.query(
    `SELECT user_id, push_enabled, updated_at FROM push_notification_preferences WHERE user_id = ?`,
    [userId],
  )

  const row = rows[0] as unknown as PushNotificationPreferenceRow | undefined
  if (row === undefined) {
    return { pushEnabled: true, updatedAt: '', userId }
  }

  return { pushEnabled: Number(row.push_enabled) === 1, updatedAt: String(row.updated_at), userId }
}

export const writePushNotificationPreference = async (
  db: PushNotificationPreferenceDb,
  input: Readonly<{ userId: string; pushEnabled: boolean; nowIso: string }>,
): Promise<PushNotificationPreference> => {
  await db.batch([
    {
      params: [input.userId, input.pushEnabled ? 1 : 0, input.nowIso, input.nowIso],
      sql: `INSERT INTO push_notification_preferences (user_id, push_enabled, created_at, updated_at)
       VALUES (?, ?, ?, ?)
       ON CONFLICT (user_id) DO UPDATE SET
         push_enabled = excluded.push_enabled,
         updated_at = excluded.updated_at`,
    },
  ])

  return { pushEnabled: input.pushEnabled, updatedAt: input.nowIso, userId: input.userId }
}
