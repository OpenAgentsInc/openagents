import { optionalString } from './json-boundary'

export type OperatorTargetUser = Readonly<{
  userId: string
  displayName: string
  email: string | null
  githubUsername: string | null
}>

const operatorTargetUserFromRow = (
  row: Readonly<{
    user_id: string
    display_name: string
    primary_email: string | null
    github_username: string | null
  }>,
): OperatorTargetUser => ({
  userId: row.user_id,
  displayName: row.display_name,
  email: row.primary_email,
  githubUsername: row.github_username,
})

export const readOperatorTargetByUserId = async (
  db: D1Database,
  userId: string,
): Promise<OperatorTargetUser | undefined> => {
  const row = await db
    .prepare(
      `SELECT
         users.id AS user_id,
         users.display_name,
         users.primary_email,
         auth_identities.provider_username AS github_username
       FROM users
       LEFT JOIN auth_identities
         ON auth_identities.user_id = users.id
        AND auth_identities.provider = 'github'
        AND auth_identities.deleted_at IS NULL
       WHERE users.id = ?
         AND users.kind = 'human'
         AND users.status = 'active'
         AND users.deleted_at IS NULL
       LIMIT 1`,
    )
    .bind(userId)
    .first<
      Readonly<{
        user_id: string
        display_name: string
        primary_email: string | null
        github_username: string | null
      }>
    >()

  return row === null ? undefined : operatorTargetUserFromRow(row)
}

export const readOperatorTargetByIdentity = async (
  db: D1Database,
  target: string,
): Promise<OperatorTargetUser | undefined> => {
  const normalized = target.trim().toLowerCase().replace(/^@/, '')
  const row = await db
    .prepare(
      `SELECT
         users.id AS user_id,
         users.display_name,
         users.primary_email,
         auth_identities.provider_username AS github_username
       FROM users
       LEFT JOIN auth_identities
         ON auth_identities.user_id = users.id
        AND auth_identities.provider = 'github'
        AND auth_identities.deleted_at IS NULL
       WHERE users.kind = 'human'
         AND users.status = 'active'
         AND users.deleted_at IS NULL
         AND (
           lower(users.primary_email) = ?
           OR lower(auth_identities.email) = ?
           OR lower(auth_identities.provider_username) = ?
         )
       ORDER BY users.updated_at DESC
       LIMIT 1`,
    )
    .bind(normalized, normalized, normalized)
    .first<
      Readonly<{
        user_id: string
        display_name: string
        primary_email: string | null
        github_username: string | null
      }>
    >()

  return row === null ? undefined : operatorTargetUserFromRow(row)
}

export const readOperatorTargetUser = async (
  db: D1Database,
  selector: Record<string, unknown>,
  defaultIdentity: string,
): Promise<OperatorTargetUser | undefined> => {
  const userId = optionalString(selector.userId)
  const login =
    optionalString(selector.githubLogin) ?? optionalString(selector.login)
  const email = optionalString(selector.email)

  if (userId !== undefined) {
    return readOperatorTargetByUserId(db, userId)
  }

  return readOperatorTargetByIdentity(db, email ?? login ?? defaultIdentity)
}
