// Khala Mobile push notification device-token registry (MM-G1, #8485).
//
// Keyed per (user_id, device_id) so re-registering the SAME device upserts
// the Expo push token instead of duplicating rows (a fresh token after
// reinstall/OS-level rotation is the common case, per Expo's own push docs).
//
// "Pruned on auth revocation" (the issue's acceptance bar): at registration
// time we store the SAME revocation-lookup key `mobile-session.ts`'s
// `revokeMobileAccessToken`/`isMobileAccessTokenRevoked` already use for the
// bearer access token active at that moment (a SHA-256 hash, never the raw
// token). `pruneRevokedPushDeviceTokens` checks that key directly against
// the auth KV store (Postgres KvStore since CFG-3 #8518 — never Cloudflare
// KV) and deletes any row whose access token has since been revoked
// (sign-out). This runs lazily before every send (MM-G2, #8486) and is
// exported standalone for an optional periodic sweep.

import { mobileRevokedAccessKey, type MobileAccessRevocationStore } from '../auth/mobile-session'

/** Typed invariant-violation error (never a generic `throw new Error` — this
 * repo's zero-debt architecture check requires typed errors at the source,
 * matching the sibling `KhalaCodePaidPlanPaymentError` pattern). Only ever
 * thrown if the D1 write itself succeeded but the immediate re-read somehow
 * returns nothing — an infrastructure anomaly, not a domain outcome. */
export class PushDeviceTokenStoreError extends Error {
  override readonly name = 'PushDeviceTokenStoreError'
}

export type PushPlatform = 'ios' | 'android'

export type PushDeviceTokenRow = Readonly<{
  userId: string
  deviceId: string
  expoPushToken: string
  platform: PushPlatform
  createdAt: string
  updatedAt: string
}>

type PushDeviceTokenSqlRow = Readonly<{
  user_id: string
  device_id: string
  expo_push_token: string
  platform: string
  access_token_revocation_key: string | null
  created_at: string
  updated_at: string
}>

const mapRow = (row: PushDeviceTokenSqlRow): PushDeviceTokenRow => ({
  createdAt: row.created_at,
  deviceId: row.device_id,
  expoPushToken: row.expo_push_token,
  platform: row.platform === 'android' ? 'android' : 'ios',
  updatedAt: row.updated_at,
  userId: row.user_id,
})

export const registerPushDeviceToken = async (
  db: D1Database,
  input: Readonly<{
    userId: string
    deviceId: string
    expoPushToken: string
    platform: PushPlatform
    /** The bearer access token active at registration time. Only its
     * revocation-lookup key is persisted — never the raw token. */
    accessToken: string
    nowIso: string
  }>,
): Promise<PushDeviceTokenRow> => {
  const revocationKey = await mobileRevokedAccessKey(input.accessToken)

  await db
    .prepare(
      `INSERT INTO push_device_tokens
        (user_id, device_id, expo_push_token, platform, access_token_revocation_key,
         created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT (user_id, device_id) DO UPDATE SET
         expo_push_token = excluded.expo_push_token,
         platform = excluded.platform,
         access_token_revocation_key = excluded.access_token_revocation_key,
         updated_at = excluded.updated_at`,
    )
    .bind(
      input.userId,
      input.deviceId,
      input.expoPushToken,
      input.platform,
      revocationKey,
      input.nowIso,
      input.nowIso,
    )
    .run()

  const row = await db
    .prepare(
      `SELECT user_id, device_id, expo_push_token, platform, access_token_revocation_key,
              created_at, updated_at
         FROM push_device_tokens
        WHERE user_id = ? AND device_id = ?`,
    )
    .bind(input.userId, input.deviceId)
    .first<PushDeviceTokenSqlRow>()

  if (row === null) {
    throw new PushDeviceTokenStoreError('push device token row not found after registration')
  }

  return mapRow(row)
}

export const unregisterPushDeviceToken = async (
  db: D1Database,
  input: Readonly<{ userId: string; deviceId: string }>,
): Promise<Readonly<{ removed: boolean }>> => {
  const result = await db
    .prepare(`DELETE FROM push_device_tokens WHERE user_id = ? AND device_id = ?`)
    .bind(input.userId, input.deviceId)
    .run()

  return { removed: (result.meta.changes ?? 0) > 0 }
}

/** Every registered device for a user, WITHOUT pruning — callers that need
 * the revocation guarantee should go through
 * `listActivePushDeviceTokensForUser` instead (MM-G2's sender does). Kept
 * separate so tests and admin inspection can read the raw registry. */
export const listPushDeviceTokensForUser = async (
  db: D1Database,
  userId: string,
): Promise<ReadonlyArray<PushDeviceTokenRow>> => {
  const { results } = await db
    .prepare(
      `SELECT user_id, device_id, expo_push_token, platform, access_token_revocation_key,
              created_at, updated_at
         FROM push_device_tokens
        WHERE user_id = ?
        ORDER BY updated_at DESC`,
    )
    .bind(userId)
    .all<PushDeviceTokenSqlRow>()

  return results.map(mapRow)
}

/** Deletes every row whose stored access-token revocation key is currently
 * present in the revocation KV (i.e. that session was signed out), and
 * returns the still-live rows. This is the "pruned on auth revocation"
 * behavior: a registration tied to an access token that gets revoked is
 * removed the next time this runs (send time, MM-G2) rather than lingering
 * forever. A row registered without ever revoking its access token (still
 * signed in) is untouched. */
export const listActivePushDeviceTokensForUser = async (
  db: D1Database,
  authStorage: MobileAccessRevocationStore,
  userId: string,
): Promise<ReadonlyArray<PushDeviceTokenRow>> => {
  const { results } = await db
    .prepare(
      `SELECT user_id, device_id, expo_push_token, platform, access_token_revocation_key,
              created_at, updated_at
         FROM push_device_tokens
        WHERE user_id = ?`,
    )
    .bind(userId)
    .all<PushDeviceTokenSqlRow>()

  const active: Array<PushDeviceTokenRow> = []
  for (const row of results) {
    if (row.access_token_revocation_key !== null) {
      const revoked = (await authStorage.get(row.access_token_revocation_key)) !== null
      if (revoked) {
        await unregisterPushDeviceToken(db, { deviceId: row.device_id, userId: row.user_id })
        continue
      }
    }
    active.push(mapRow(row))
  }

  return active
}

/** Deletes the Expo push token for EVERY user+device row that currently
 * carries it (MM-G2, #8486's invalidation-receipt handling: Expo's push
 * service reports a token as `DeviceNotRegistered`, and the token itself —
 * not a user id — is the only thing the receipt names). Returns the number
 * of rows removed. */
export const removePushDeviceTokensByExpoToken = async (
  db: D1Database,
  expoPushToken: string,
): Promise<number> => {
  const result = await db
    .prepare(`DELETE FROM push_device_tokens WHERE expo_push_token = ?`)
    .bind(expoPushToken)
    .run()

  return result.meta.changes ?? 0
}
