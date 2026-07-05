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
}>

const OWNER_USER_ID_KEY = "khala.auth.ownerUserId"
const TOKEN_KEY = "khala.auth.token"

export const loadStoredCredentials = async (): Promise<KhalaStoredCredentials | null> => {
  const [ownerUserId, token] = await Promise.all([
    SecureStore.getItemAsync(OWNER_USER_ID_KEY),
    SecureStore.getItemAsync(TOKEN_KEY)
  ])
  if (ownerUserId === null || ownerUserId === "" || token === null || token === "") return null
  return { ownerUserId, token }
}

export const saveStoredCredentials = async (credentials: KhalaStoredCredentials): Promise<void> => {
  await Promise.all([
    SecureStore.setItemAsync(OWNER_USER_ID_KEY, credentials.ownerUserId),
    SecureStore.setItemAsync(TOKEN_KEY, credentials.token)
  ])
}

export const clearStoredCredentials = async (): Promise<void> => {
  await Promise.all([
    SecureStore.deleteItemAsync(OWNER_USER_ID_KEY),
    SecureStore.deleteItemAsync(TOKEN_KEY)
  ])
}
