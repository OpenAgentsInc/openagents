/**
 * On-device persistence for push registration state (MM-G1, #8485): a stable
 * per-install device id (so the server can key a token registration per
 * user+device and dedupe re-registrations) and the "has the OS permission
 * prompt ever been shown" flag that `shouldPromptForPushPermission` reads.
 * Mirrors the `SecureStoreLike` injection pattern in `../security/keychain.ts`
 * so both stay unit-testable without a real device.
 */

export type SecureStoreOptions = Readonly<{
  keychainService?: string
  keychainAccessible?: unknown
}>

export type SecureStoreLike = Readonly<{
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY?: unknown
  deleteItemAsync: (key: string, options?: SecureStoreOptions) => Promise<void>
  getItemAsync: (key: string, options?: SecureStoreOptions) => Promise<string | null>
  setItemAsync: (key: string, value: string, options?: SecureStoreOptions) => Promise<void>
}>

export const KHALA_PUSH_KEYCHAIN_SERVICE = "com.openagents.khala.mobile"
const DEVICE_ID_KEY = "khala.push.deviceId"
const HAS_PROMPTED_KEY = "khala.push.hasPrompted"

const loadSecureStore = async (): Promise<SecureStoreLike> =>
  (await import("expo-secure-store")) as SecureStoreLike

const keychainOptions = (secureStore: SecureStoreLike): SecureStoreOptions => ({
  keychainAccessible: secureStore.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
  keychainService: KHALA_PUSH_KEYCHAIN_SERVICE
})

/** A caller-supplied random id generator, injected so tests never need a
 * real crypto RNG or expo-crypto's native binding. */
export type MakeDeviceId = () => string

export const loadOrCreatePushDeviceId = async (input: {
  makeDeviceId: MakeDeviceId
  secureStoreLoader?: () => Promise<SecureStoreLike>
}): Promise<string> => {
  const secureStore = await (input.secureStoreLoader ?? loadSecureStore)()
  const existing = await secureStore.getItemAsync(DEVICE_ID_KEY, keychainOptions(secureStore))
  const trimmed = existing?.trim() ?? ""
  if (trimmed.length > 0) return trimmed

  const created = input.makeDeviceId().trim()
  if (created.length === 0) {
    throw new Error("push device id generator returned an empty id")
  }
  await secureStore.setItemAsync(DEVICE_ID_KEY, created, keychainOptions(secureStore))
  return created
}

export const loadHasEverPromptedForPush = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<boolean> => {
  const secureStore = await secureStoreLoader()
  const value = await secureStore.getItemAsync(HAS_PROMPTED_KEY, keychainOptions(secureStore))
  return value === "1"
}

export const saveHasEverPromptedForPush = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  const secureStore = await secureStoreLoader()
  await secureStore.setItemAsync(HAS_PROMPTED_KEY, "1", keychainOptions(secureStore))
}

/** Sign-out cleanup: forget the device id so a fresh sign-in registers a
 * fresh one (the server row for the old id is removed via the unregister
 * call this pairs with, see `push-notifications-client.ts`). The
 * "has ever prompted" flag intentionally survives sign-out — the OS
 * permission grant is a device-level fact, not a per-account one, and
 * re-prompting a user who already answered on this device would violate
 * `khala_mobile.push.permission_prompt_on_first_task_dispatch.v1`. */
export const clearPushDeviceId = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  const secureStore = await secureStoreLoader()
  await secureStore.deleteItemAsync(DEVICE_ID_KEY, keychainOptions(secureStore))
}

export const readPushDeviceIdIfPresent = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<string | null> => {
  const secureStore = await secureStoreLoader()
  const value = await secureStore.getItemAsync(DEVICE_ID_KEY, keychainOptions(secureStore))
  const trimmed = value?.trim() ?? ""
  return trimmed.length === 0 ? null : trimmed
}
