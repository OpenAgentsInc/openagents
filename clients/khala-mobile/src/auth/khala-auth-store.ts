import * as SecureStore from "expo-secure-store"

/**
 * Persisted Khala Sync identity for this device — a bearer token plus the
 * owner user id needed to construct `scope.user.<id>` (the token itself is
 * opaque server-side; the client can't derive its own owner id from it, so
 * both fields are stored together).
 */
export type KhalaStoredCredentials = Readonly<{
  ownerUserId: string
  token: string
  // GitHub login (username) for the signed-in user, when the server returned
  // it. Optional: absent for email-provider sessions and for credentials
  // persisted before the greeting change shipped.
  githubLogin?: string
}>

const OWNER_USER_ID_KEY = "khala.auth.ownerUserId"
const TOKEN_KEY = "khala.auth.token"
const GITHUB_LOGIN_KEY = "khala.auth.githubLogin"
const CREDENTIAL_EPOCH_KEY = "khala.auth.credentialEpoch"

/** Bumped whenever the auth model changes underneath stored credentials
 * (retired flows: Tailnet QR pairing, any pre-GitHub-OpenAuth token shape).
 * A credential written under an older/missing epoch is a leftover from a
 * prior auth model — it may still pass server-side validation (the old
 * token is technically live), which is exactly the bug: the app would
 * silently resume as whatever identity that old token belongs to instead
 * of landing on GitHub sign-in. Bumping this forces an unconditional purge
 * on next launch, independent of whether the old token still validates. */
const CURRENT_CREDENTIAL_EPOCH = "2026-07-06-github-openauth-v1"

export const loadStoredCredentials = async (): Promise<KhalaStoredCredentials | null> => {
  const [ownerUserId, token, githubLogin, epoch] = await Promise.all([
    SecureStore.getItemAsync(OWNER_USER_ID_KEY),
    SecureStore.getItemAsync(TOKEN_KEY),
    SecureStore.getItemAsync(GITHUB_LOGIN_KEY),
    SecureStore.getItemAsync(CREDENTIAL_EPOCH_KEY)
  ])
  if (ownerUserId === null || ownerUserId === "" || token === null || token === "") return null
  if (epoch !== CURRENT_CREDENTIAL_EPOCH) {
    await clearStoredCredentials()
    return null
  }
  return githubLogin !== null && githubLogin !== ""
    ? { ownerUserId, token, githubLogin }
    : { ownerUserId, token }
}

export const saveStoredCredentials = async (credentials: KhalaStoredCredentials): Promise<void> => {
  await Promise.all([
    SecureStore.setItemAsync(OWNER_USER_ID_KEY, credentials.ownerUserId),
    SecureStore.setItemAsync(TOKEN_KEY, credentials.token),
    credentials.githubLogin !== undefined && credentials.githubLogin !== ""
      ? SecureStore.setItemAsync(GITHUB_LOGIN_KEY, credentials.githubLogin)
      : SecureStore.deleteItemAsync(GITHUB_LOGIN_KEY),
    SecureStore.setItemAsync(CREDENTIAL_EPOCH_KEY, CURRENT_CREDENTIAL_EPOCH)
  ])
}

export const clearStoredCredentials = async (): Promise<void> => {
  await Promise.all([
    SecureStore.deleteItemAsync(OWNER_USER_ID_KEY),
    SecureStore.deleteItemAsync(TOKEN_KEY),
    SecureStore.deleteItemAsync(GITHUB_LOGIN_KEY),
    SecureStore.deleteItemAsync(CREDENTIAL_EPOCH_KEY)
  ])
}
