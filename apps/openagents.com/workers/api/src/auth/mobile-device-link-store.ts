import type { IdentityDb } from '../identity-db'

export type MobileDeviceLinkState = 'linked' | 'already_linked' | 'conflict'

export type MobileDeviceLinkStore = Readonly<{
  link: (
    input: Readonly<{
      deviceRef: string
      nowIso: string
      ownerRef: string
      pubkey: string
    }>,
  ) => Promise<MobileDeviceLinkState>
}>

type IdentityRow = Readonly<{
  deleted_at?: unknown
  user_id?: unknown
}>

const readIdentity = async (
  db: IdentityDb,
  pubkey: string,
): Promise<Readonly<{ deleted: boolean; ownerRef: string }> | undefined> => {
  const rows = (await db.query(
    `SELECT user_id, deleted_at
       FROM auth_identities
      WHERE provider = 'nostr'
        AND provider_subject = ?
      LIMIT 1`,
    [pubkey],
  )) as ReadonlyArray<IdentityRow>
  const row = rows[0]
  const ownerRef =
    row === undefined || typeof row.user_id !== 'string'
      ? ''
      : row.user_id.trim()

  return ownerRef === ''
    ? undefined
    : {
        deleted: row?.deleted_at !== null && row?.deleted_at !== undefined,
        ownerRef,
      }
}

/**
 * Link one protected Nostr key to one canonical OpenAgents user.
 *
 * The unique `(provider, provider_subject)` identity constraint is the
 * ownership authority. The conditional upsert can reactivate the same
 * owner's identity, but it cannot move a key between accounts.
 */
export const makeMobileDeviceLinkStore = (
  db: IdentityDb,
): MobileDeviceLinkStore => ({
  link: async input => {
    const before = await readIdentity(db, input.pubkey)
    if (before !== undefined && before.ownerRef !== input.ownerRef) {
      return 'conflict'
    }

    await db.batch([
      {
        sql: `INSERT INTO auth_identities (
                id, user_id, provider, provider_subject, provider_username,
                email, created_at, updated_at, deleted_at
              )
              VALUES (?, ?, 'nostr', ?, ?, NULL, ?, ?, NULL)
              ON CONFLICT(provider, provider_subject) DO UPDATE SET
                provider_username = excluded.provider_username,
                updated_at = excluded.updated_at,
                deleted_at = NULL
              WHERE auth_identities.user_id = excluded.user_id`,
        params: [
          `auth_identity_nostr_${input.pubkey}`,
          input.ownerRef,
          input.pubkey,
          input.deviceRef,
          input.nowIso,
          input.nowIso,
        ],
      },
    ])

    const after = await readIdentity(db, input.pubkey)
    if (after === undefined || after.ownerRef !== input.ownerRef) {
      return 'conflict'
    }

    return before === undefined ? 'linked' : 'already_linked'
  },
})
