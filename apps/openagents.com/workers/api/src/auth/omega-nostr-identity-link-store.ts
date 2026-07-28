import type { IdentityDb } from '../identity-db'

export type OmegaNostrIdentityLinkResult =
  'Linked' | 'AlreadyLinked' | 'Conflict' | 'Unavailable'

export const linkOmegaNostrIdentity = async (
  identityDb: IdentityDb,
  input: Readonly<{
    displayName: string
    nowIso: string
    ownerRef: string
    pubkey: string
  }>,
): Promise<OmegaNostrIdentityLinkResult> => {
  if (
    !/^[0-9a-f]{64}$/u.test(input.pubkey) ||
    input.ownerRef.startsWith('nostr:')
  ) {
    return 'Conflict'
  }

  const readBinding = async () =>
    (
      await identityDb.query(
        `SELECT user_id, deleted_at
           FROM auth_identities
          WHERE provider = 'nostr'
            AND provider_subject = ?
          LIMIT 1`,
        [input.pubkey],
      )
    )[0]

  const owner = (
    await identityDb.query(
      `SELECT id, display_name, primary_email
         FROM users
        WHERE id = ?
          AND status = 'active'
          AND deleted_at IS NULL
        LIMIT 1`,
      [input.ownerRef],
    )
  )[0]
  if (owner === undefined) return 'Conflict'

  const existing = await readBinding()
  if (existing !== undefined) {
    return String(existing.user_id) === input.ownerRef &&
      existing.deleted_at === null
      ? 'AlreadyLinked'
      : 'Conflict'
  }

  await identityDb.batch([
    {
      sql: `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email,
           created_at, updated_at)
         VALUES (?, ?, 'nostr', ?, ?, ?, ?, ?)
         ON CONFLICT(provider, provider_subject) DO NOTHING`,
      params: [
        `auth_identity_nostr_${input.pubkey}`,
        input.ownerRef,
        input.pubkey,
        input.displayName,
        String(owner.primary_email ?? ''),
        input.nowIso,
        input.nowIso,
      ],
    },
  ])

  const stored = await readBinding()
  if (stored === undefined) return 'Unavailable'
  if (String(stored.user_id) !== input.ownerRef || stored.deleted_at !== null) {
    return 'Conflict'
  }
  return 'Linked'
}
