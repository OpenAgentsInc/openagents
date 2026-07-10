import { Schema } from "effect"

export const OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE =
  "com.openagents.mobile.session"
export const OPENAGENTS_NATIVE_SESSION_KEY = "openagents.native.session"
export const OPENAGENTS_NATIVE_SESSION_EPOCH =
  "2026-07-10-openauth-native-session-v1"

export type SecureStoreOptions = Readonly<{
  keychainService?: string
  keychainAccessible?: unknown
}>

export type SecureStoreLike = Readonly<{
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY?: unknown
  deleteItemAsync: (key: string, options?: SecureStoreOptions) => Promise<void>
  getItemAsync: (key: string, options?: SecureStoreOptions) => Promise<string | null>
  setItemAsync: (
    key: string,
    value: string,
    options?: SecureStoreOptions,
  ) => Promise<void>
}>

const NativeSessionRecordSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  credentialEpoch: Schema.Literal(OPENAGENTS_NATIVE_SESSION_EPOCH),
  ownerUserId: Schema.String,
  accessToken: Schema.String,
  refreshToken: Schema.String,
})

type NativeSessionRecord = typeof NativeSessionRecordSchema.Type

export type NativeSessionCredential = Readonly<{
  ownerUserId: string
  accessToken: string
  refreshToken: string
}>

export type NativeSessionRecovery = Readonly<{
  state: "signed_out" | "credential_present_unverified"
}>

export type NativeSessionVaultErrorReason =
  | "invalid_credential"
  | "secure_store_unavailable"

export class NativeSessionVaultError extends Error {
  readonly _tag = "NativeSessionVaultError"
  override readonly name = "NativeSessionVaultError"

  constructor(
    readonly reason: NativeSessionVaultErrorReason,
    message: string,
  ) {
    super(message)
  }
}

const loadSecureStore = async (): Promise<SecureStoreLike> =>
  (await import("expo-secure-store")) as SecureStoreLike

const secureStoreOptions = (store: SecureStoreLike): SecureStoreOptions => ({
  keychainService: OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE,
  keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
})

const normalizeCredential = (
  credential: NativeSessionCredential,
): NativeSessionCredential => ({
  ownerUserId: credential.ownerUserId.trim(),
  accessToken: credential.accessToken.trim(),
  refreshToken: credential.refreshToken.trim(),
})

const requireValidCredential = (
  credential: NativeSessionCredential,
): NativeSessionCredential => {
  const normalized = normalizeCredential(credential)
  if (
    normalized.ownerUserId === "" ||
    normalized.accessToken === "" ||
    normalized.refreshToken === ""
  ) {
    throw new NativeSessionVaultError(
      "invalid_credential",
      "native session credential is incomplete",
    )
  }
  return normalized
}

const decodeRecord = (raw: string): NativeSessionCredential | null => {
  try {
    const record = Schema.decodeUnknownSync(NativeSessionRecordSchema)(
      JSON.parse(raw),
    )
    return requireValidCredential(record)
  } catch {
    return null
  }
}

const storeFailure = (error: unknown): NativeSessionVaultError =>
  error instanceof NativeSessionVaultError
    ? error
    : new NativeSessionVaultError(
        "secure_store_unavailable",
        "native secure session storage is unavailable",
      )

export const saveNativeSessionCredential = async (
  credential: NativeSessionCredential,
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  const normalized = requireValidCredential(credential)
  const record: NativeSessionRecord = {
    schemaVersion: 1,
    credentialEpoch: OPENAGENTS_NATIVE_SESSION_EPOCH,
    ...normalized,
  }
  try {
    const store = await secureStoreLoader()
    await store.setItemAsync(
      OPENAGENTS_NATIVE_SESSION_KEY,
      JSON.stringify(record),
      secureStoreOptions(store),
    )
  } catch (error) {
    throw storeFailure(error)
  }
}

export const clearNativeSessionCredential = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<void> => {
  try {
    const store = await secureStoreLoader()
    await store.deleteItemAsync(
      OPENAGENTS_NATIVE_SESSION_KEY,
      secureStoreOptions(store),
    )
  } catch (error) {
    throw storeFailure(error)
  }
}

export const loadNativeSessionCredential = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<NativeSessionCredential | null> => {
  try {
    const store = await secureStoreLoader()
    const options = secureStoreOptions(store)
    const raw = await store.getItemAsync(OPENAGENTS_NATIVE_SESSION_KEY, options)
    if (raw === null || raw.trim() === "") return null
    const credential = decodeRecord(raw)
    if (credential !== null) return credential
    await store.deleteItemAsync(OPENAGENTS_NATIVE_SESSION_KEY, options)
    return null
  } catch (error) {
    throw storeFailure(error)
  }
}

/** Public-safe host recovery state; credential fields never enter view state. */
export const recoverNativeSession = async (
  secureStoreLoader: () => Promise<SecureStoreLike> = loadSecureStore,
): Promise<NativeSessionRecovery> => ({
  state:
    (await loadNativeSessionCredential(secureStoreLoader)) === null
      ? "signed_out"
      : "credential_present_unverified",
})
