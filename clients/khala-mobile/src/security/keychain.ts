export const KHALA_MOBILE_KEYCHAIN_SERVICE = "com.openagents.khala.mobile"
export const KHALA_MOBILE_API_KEY_ACCOUNT = "khala_api_key"

export type SecureStoreOptions = Readonly<{
  keychainService?: string
  keychainAccessible?: unknown
}>

export type SecureStoreLike = Readonly<{
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY?: unknown
  deleteItemAsync: (
    key: string,
    options?: SecureStoreOptions,
  ) => Promise<void>
  getItemAsync: (
    key: string,
    options?: SecureStoreOptions,
  ) => Promise<string | null>
  setItemAsync: (
    key: string,
    value: string,
    options?: SecureStoreOptions,
  ) => Promise<void>
}>

const loadSecureStore = async (): Promise<SecureStoreLike> =>
  (await import("expo-secure-store")) as SecureStoreLike

const keychainOptions = (secureStore: SecureStoreLike): SecureStoreOptions => ({
  keychainAccessible: secureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
  keychainService: KHALA_MOBILE_KEYCHAIN_SERVICE
})

const normalizeApiKey = (apiKey: string): string => apiKey.trim()

export const saveKhalaApiKey = async (
  apiKey: string,
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  const normalized = normalizeApiKey(apiKey)
  if (normalized.length === 0) {
    throw new Error("Khala API key is required.")
  }

  const secureStore = await secureStoreLoader()
  await secureStore.setItemAsync(
    KHALA_MOBILE_API_KEY_ACCOUNT,
    normalized,
    keychainOptions(secureStore)
  )
}

export const loadKhalaApiKey = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<string | null> => {
  const secureStore = await secureStoreLoader()
  const value = await secureStore.getItemAsync(
    KHALA_MOBILE_API_KEY_ACCOUNT,
    keychainOptions(secureStore)
  )
  const normalized = value === null ? "" : normalizeApiKey(value)

  return normalized.length === 0 ? null : normalized
}

export const deleteKhalaApiKey = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  const secureStore = await secureStoreLoader()
  await secureStore.deleteItemAsync(
    KHALA_MOBILE_API_KEY_ACCOUNT,
    keychainOptions(secureStore)
  )
}
