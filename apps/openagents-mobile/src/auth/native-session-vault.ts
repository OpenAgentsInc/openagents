import { Schema } from "effect";

export const OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE = "com.openagents.mobile.session";
export const OPENAGENTS_NATIVE_SESSION_KEY = "openagents.native.session";
export const OPENAGENTS_NATIVE_SESSION_EPOCH = "2026-07-10-openauth-native-session-v1";

export type NativeSessionSecureStoreOptions = Readonly<{
  keychainService?: string;
  keychainAccessible?: unknown;
}>;

export type NativeSessionSecureStore = Readonly<{
  AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY?: unknown;
  deleteItemAsync: (key: string, options?: NativeSessionSecureStoreOptions) => Promise<void>;
  getItemAsync: (key: string, options?: NativeSessionSecureStoreOptions) => Promise<string | null>;
  setItemAsync: (
    key: string,
    value: string,
    options?: NativeSessionSecureStoreOptions,
  ) => Promise<void>;
}>;

const NativeSessionRecordSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  credentialEpoch: Schema.Literal(OPENAGENTS_NATIVE_SESSION_EPOCH),
  ownerUserId: Schema.String,
  accessToken: Schema.String,
  refreshToken: Schema.String,
});

type NativeSessionRecord = typeof NativeSessionRecordSchema.Type;

export type NativeSessionCredential = Readonly<{
  ownerUserId: string;
  accessToken: string;
  refreshToken: string;
}>;

export type NativeSessionVaultErrorReason = "invalid_credential" | "secure_store_unavailable";

export class NativeSessionVaultError extends Error {
  readonly _tag = "NativeSessionVaultError";
  override readonly name = "NativeSessionVaultError";

  constructor(
    readonly reason: NativeSessionVaultErrorReason,
    message: string,
  ) {
    super(message);
  }
}

const secureStoreOptions = (store: NativeSessionSecureStore): NativeSessionSecureStoreOptions => ({
  keychainService: OPENAGENTS_NATIVE_SESSION_KEYCHAIN_SERVICE,
  keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
});

const requireValidCredential = (credential: NativeSessionCredential): NativeSessionCredential => {
  const normalized = {
    ownerUserId: credential.ownerUserId.trim(),
    accessToken: credential.accessToken.trim(),
    refreshToken: credential.refreshToken.trim(),
  };
  if (
    normalized.ownerUserId === "" ||
    normalized.accessToken === "" ||
    normalized.refreshToken === ""
  ) {
    throw new NativeSessionVaultError(
      "invalid_credential",
      "The mobile session credential is incomplete.",
    );
  }
  return normalized;
};

const decodeRecord = (raw: string): NativeSessionCredential | null => {
  try {
    return requireValidCredential(
      Schema.decodeUnknownSync(NativeSessionRecordSchema)(JSON.parse(raw)),
    );
  } catch {
    return null;
  }
};

const storeFailure = (error: unknown): NativeSessionVaultError =>
  error instanceof NativeSessionVaultError
    ? error
    : new NativeSessionVaultError(
        "secure_store_unavailable",
        "The secure mobile session store is unavailable.",
      );

export const loadNativeSessionCredential = async (
  store: NativeSessionSecureStore,
): Promise<NativeSessionCredential | null> => {
  const options = secureStoreOptions(store);
  try {
    const raw = await store.getItemAsync(OPENAGENTS_NATIVE_SESSION_KEY, options);
    if (raw === null || raw.trim() === "") return null;
    const credential = decodeRecord(raw);
    if (credential !== null) return credential;
    await store.deleteItemAsync(OPENAGENTS_NATIVE_SESSION_KEY, options);
    return null;
  } catch (error) {
    throw storeFailure(error);
  }
};

export const saveNativeSessionCredential = async (
  store: NativeSessionSecureStore,
  credential: NativeSessionCredential,
): Promise<void> => {
  const normalized = requireValidCredential(credential);
  const record: NativeSessionRecord = {
    schemaVersion: 1,
    credentialEpoch: OPENAGENTS_NATIVE_SESSION_EPOCH,
    ...normalized,
  };
  try {
    await store.setItemAsync(
      OPENAGENTS_NATIVE_SESSION_KEY,
      JSON.stringify(record),
      secureStoreOptions(store),
    );
  } catch (error) {
    throw storeFailure(error);
  }
};

export const clearNativeSessionCredential = async (
  store: NativeSessionSecureStore,
): Promise<void> => {
  try {
    await store.deleteItemAsync(OPENAGENTS_NATIVE_SESSION_KEY, secureStoreOptions(store));
  } catch (error) {
    throw storeFailure(error);
  }
};
