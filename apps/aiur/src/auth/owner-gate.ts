/**
 * The owner allowlist gate (AIUR-1, #8499). FAIL-CLOSED is the entire
 * point of this module: an unset or empty `AIUR_OWNER_USER_IDS` must deny
 * every user id, including a verified, legitimately-signed-in one — never
 * "no allowlist configured means allow everyone".
 */

export const parseOwnerAllowlist = (
  raw: string | undefined,
): ReadonlySet<string> => {
  if (raw === undefined) {
    return new Set()
  }

  const ids = raw
    .split(',')
    .map(id => id.trim())
    .filter(id => id.length > 0)

  return new Set(ids)
}

export const isAllowedOwnerUserId = (
  userId: string | undefined,
  allowlist: ReadonlySet<string>,
): boolean => {
  if (allowlist.size === 0) {
    // Fail closed: no configured allowlist denies everyone, always.
    return false
  }

  if (userId === undefined || userId.trim() === '') {
    return false
  }

  return allowlist.has(userId.trim())
}
