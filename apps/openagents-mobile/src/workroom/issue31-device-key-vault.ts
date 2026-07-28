import { Schema } from "effect";
import { LocalKeySigner } from "nostr-effect/identity";

import type { Issue31NostrSigner } from "@openagentsinc/sarah/issue31-nostr";

declare const require: (id: string) => unknown;

export const ISSUE31_DEVICE_KEY_STORE_KEY = "openagents.omega.issue31.device-key.v1" as const;
export const ISSUE31_DEVICE_KEYCHAIN_SERVICE = "com.openagents.mobile.omega-device" as const;
export const SARAH_STAGING_DEVICE_KEY_STORE_KEY =
  "openagents.mobile.sarah.staging-device-key.v1" as const;

export interface Issue31SecureStoreOptions {
  readonly keychainService?: string;
  readonly keychainAccessible?: unknown;
  readonly requireAuthentication?: boolean;
}

/**
 * The platform whose key-custody contract we are holding the native store to.
 *
 * Anything we cannot positively identify as `ios` or `android` is `unknown` and
 * fails closed: custody rules are stated per platform, never guessed.
 */
export type Issue31DeviceKeyPlatform = "ios" | "android" | "unknown";

export interface Issue31SecureStore {
  readonly AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY?: unknown;
  readonly getItemAsync: (
    key: string,
    options?: Issue31SecureStoreOptions,
  ) => Promise<string | null>;
  readonly setItemAsync: (
    key: string,
    value: string,
    options?: Issue31SecureStoreOptions,
  ) => Promise<void>;
  readonly deleteItemAsync: (key: string, options?: Issue31SecureStoreOptions) => Promise<void>;
}

export interface Issue31DeviceIdentity {
  readonly publicKeyHex: string;
  readonly npub: string;
  readonly signer: Issue31NostrSigner;
  readonly close: () => void;
}

export type Issue31DeviceKeyVaultErrorReason =
  | "secure_store_unavailable"
  | "invalid_device_key"
  | "random_unavailable";

export class Issue31DeviceKeyVaultError extends Error {
  readonly _tag = "Issue31DeviceKeyVaultError";
  override readonly name = "Issue31DeviceKeyVaultError";

  constructor(
    readonly reason: Issue31DeviceKeyVaultErrorReason,
    message: string,
  ) {
    super(message);
  }
}

const DeviceKeyRecordSchema = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  privateKeyHex: Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/)),
});

/**
 * Android custody options for the Omega device key.
 *
 * `expo-secure-store`'s Android module declares no accessibility constants at
 * all — `AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY` and its siblings are exported
 * only by the iOS module, so on Android the JS re-export is `undefined`. The
 * Android record accepts `keychainService`, `requireAuthentication`, and
 * `authenticationPrompt` and nothing else. These are the options that get us
 * closest to the iOS accessibility class, stated exactly:
 *
 * Preserved versus iOS `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`:
 *   - Non-exportability off the device. The value is AES-GCM encrypted under a
 *     key generated inside `AndroidKeyStore` under an alias derived from
 *     `keychainService`; that key is non-extractable and cannot be transferred.
 *     A copy of the ciphertext restored onto another device is undecryptable.
 *     Android reaches "this device only" through key non-transferability where
 *     iOS reaches it through a backup-exclusion attribute.
 *   - App scoping. The ciphertext lives in `MODE_PRIVATE` SharedPreferences and
 *     the keystore entry is scoped to this app's uid.
 *   - A key alias dedicated to Omega, distinct from other vaults in this app,
 *     because we pass our own `keychainService`.
 *   - No user-interaction gate, matching iOS: `AFTER_FIRST_UNLOCK_*` is not
 *     biometry/passcode gated either. `requireAuthentication: true` would be
 *     *stricter* than the iOS class, would prompt on every silent runtime open,
 *     and would hard-fail on a device with no lock screen, so it is not parity.
 *
 * NOT preserved versus iOS:
 *   - Backup exclusion of the stored blob. iOS `ThisDeviceOnly` keeps the item
 *     out of iCloud Keychain and encrypted backups entirely. On Android the
 *     encrypted blob is an ordinary SharedPreferences entry and is eligible for
 *     Android Auto Backup / device transfer. It leaves the device as
 *     ciphertext that no other device can decrypt, but it does leave.
 *   - An API-requested at-rest unlock gate. The keystore key is generated with
 *     `setUserAuthenticationRequired(false)`, so it is usable whenever this app
 *     runs. Pre-first-unlock protection comes only from Android's default
 *     credential-encrypted app storage, which we neither request nor can
 *     verify from JS, rather than from the requested accessibility class.
 *   - Hardware backing is not requested (no StrongBox), and on an emulator the
 *     AndroidKeyStore implementation is software-only, so emulator custody is
 *     weaker than a TEE-backed physical device.
 */
export const ISSUE31_ANDROID_DEVICE_KEY_OPTIONS: Issue31SecureStoreOptions = Object.freeze({
  keychainService: ISSUE31_DEVICE_KEYCHAIN_SERVICE,
  requireAuthentication: false,
});

/**
 * The native store must actually be linked. On iOS the presence of
 * `AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY` doubled as a liveness canary for the
 * native module; Android never had that canary, so we check the call surface
 * directly instead of inferring liveness from a constant.
 */
const hasStoreCallSurface = (store: Issue31SecureStore): boolean =>
  typeof store?.getItemAsync === "function" &&
  typeof store?.setItemAsync === "function" &&
  typeof store?.deleteItemAsync === "function";

const storeOptions = (
  store: Issue31SecureStore,
  platform: Issue31DeviceKeyPlatform,
): Issue31SecureStoreOptions => {
  if (!hasStoreCallSurface(store)) {
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "The Omega device key store is not linked on this device.",
    );
  }
  if (store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY !== undefined) {
    return {
      keychainService: ISSUE31_DEVICE_KEYCHAIN_SERVICE,
      keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
    };
  }
  // Android is the only platform where the constant is absent by design. On
  // iOS an absent constant means the native module did not export its
  // accessibility classes, and we fail closed rather than downgrade custody.
  if (platform === "android") return ISSUE31_ANDROID_DEVICE_KEY_OPTIONS;
  throw new Issue31DeviceKeyVaultError(
    "secure_store_unavailable",
    "This-device-only Omega key custody is unavailable.",
  );
};

const bytesToHex = (bytes: Uint8Array): string =>
  [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");

const hexToBytes = (value: string): Uint8Array => {
  const bytes = new Uint8Array(32);
  for (let index = 0; index < bytes.length; index += 1) {
    const pair = value.slice(index * 2, index * 2 + 2);
    const byte = Number.parseInt(pair, 16);
    if (!Number.isFinite(byte)) throw new Error("invalid key byte");
    bytes[index] = byte;
  }
  return bytes;
};

const publicIdentity = (localSigner: LocalKeySigner): Issue31DeviceIdentity => {
  const manifest = localSigner.toPublicManifest();
  const signer: Issue31NostrSigner = {
    getPublicKey: () => localSigner.getPublicKey(),
    signEvent: (event) => localSigner.signEvent(event),
    nip44Encrypt: (recipientPublicKeyHex, plaintext) =>
      localSigner.nip44Encrypt(recipientPublicKeyHex, plaintext),
    nip44Decrypt: (senderPublicKeyHex, ciphertext) =>
      localSigner.nip44Decrypt(senderPublicKeyHex, ciphertext),
  };
  return {
    publicKeyHex: manifest.pubkey,
    npub: manifest.npub,
    signer,
    close: () => localSigner.dispose(),
  };
};

const signerFromPrivateKey = (privateKey: Uint8Array): LocalKeySigner => {
  try {
    return LocalKeySigner.fromPrivateKey(privateKey);
  } catch {
    throw new Issue31DeviceKeyVaultError("invalid_device_key", "The Omega device key is invalid.");
  } finally {
    privateKey.fill(0);
  }
};

export const openIssue31DeviceIdentity = async (
  input: Readonly<{
    store: Issue31SecureStore;
    randomBytes: (length: number) => Promise<Uint8Array>;
    /** Required: custody rules are per platform and are never defaulted. */
    platform: Issue31DeviceKeyPlatform;
    /** A separate staging key preserves the production device identity. */
    storeKey?: string;
  }>,
): Promise<Issue31DeviceIdentity> => {
  const options = storeOptions(input.store, input.platform);
  let stored: string | null;
  try {
    stored = await input.store.getItemAsync(
      input.storeKey ?? ISSUE31_DEVICE_KEY_STORE_KEY,
      options,
    );
  } catch {
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "The Omega device key store is unavailable.",
    );
  }

  if (stored !== null) {
    let record: Schema.Schema.Type<typeof DeviceKeyRecordSchema>;
    try {
      record = Schema.decodeUnknownSync(DeviceKeyRecordSchema)(JSON.parse(stored), {
        onExcessProperty: "error",
      });
    } catch {
      throw new Issue31DeviceKeyVaultError(
        "invalid_device_key",
        "The stored Omega device key record is invalid.",
      );
    }
    return publicIdentity(signerFromPrivateKey(hexToBytes(record.privateKeyHex)));
  }

  let privateKey: Uint8Array;
  try {
    privateKey = await input.randomBytes(32);
  } catch {
    throw new Issue31DeviceKeyVaultError(
      "random_unavailable",
      "Secure random bytes are unavailable for the Omega device key.",
    );
  }
  if (privateKey.length !== 32) {
    privateKey.fill(0);
    throw new Issue31DeviceKeyVaultError(
      "random_unavailable",
      "Secure random bytes returned the wrong device-key length.",
    );
  }
  const privateKeyHex = bytesToHex(privateKey);
  const localSigner = signerFromPrivateKey(privateKey);
  try {
    await input.store.setItemAsync(
      input.storeKey ?? ISSUE31_DEVICE_KEY_STORE_KEY,
      JSON.stringify({ schemaVersion: 1, privateKeyHex }),
      options,
    );
  } catch {
    localSigner.dispose();
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "The Omega device key could not be stored.",
    );
  }
  return publicIdentity(localSigner);
};

export const clearIssue31DeviceIdentity = async (
  store: Issue31SecureStore,
  platform: Issue31DeviceKeyPlatform,
): Promise<void> => {
  try {
    await store.deleteItemAsync(ISSUE31_DEVICE_KEY_STORE_KEY, storeOptions(store, platform));
  } catch {
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "The Omega device key could not be removed.",
    );
  }
};

/**
 * Resolve the running platform, failing closed to `unknown` for anything we do
 * not have a written custody rule for (web, or a broken react-native surface).
 */
export const expoIssue31DeviceKeyPlatform = (): Issue31DeviceKeyPlatform => {
  let os: unknown;
  try {
    os = (require("react-native") as Readonly<{ Platform?: Readonly<{ OS?: unknown }> }>)?.Platform
      ?.OS;
  } catch {
    return "unknown";
  }
  if (os === "ios") return "ios";
  if (os === "android") return "android";
  return "unknown";
};

export const openExpoIssue31DeviceIdentity = async (
  storeKey?: string,
): Promise<Issue31DeviceIdentity> => {
  const store = require("expo-secure-store") as Issue31SecureStore;
  const crypto = require("expo-crypto") as Readonly<{
    getRandomBytesAsync: (length: number) => Promise<Uint8Array>;
  }>;
  return openIssue31DeviceIdentity({
    store,
    randomBytes: (length) => crypto.getRandomBytesAsync(length),
    platform: expoIssue31DeviceKeyPlatform(),
    storeKey,
  });
};
