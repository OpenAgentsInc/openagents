import { Schema } from "effect";
import { LocalKeySigner } from "nostr-effect/identity";

import type { Issue31NostrSigner } from "@openagentsinc/sarah/issue31-nostr";

declare const require: (id: string) => unknown;

export const ISSUE31_DEVICE_KEY_STORE_KEY = "openagents.omega.issue31.device-key.v1" as const;
export const ISSUE31_DEVICE_KEYCHAIN_SERVICE = "com.openagents.mobile.omega-device" as const;

export interface Issue31SecureStoreOptions {
  readonly keychainService?: string;
  readonly keychainAccessible?: unknown;
}

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

const storeOptions = (store: Issue31SecureStore): Issue31SecureStoreOptions => {
  if (store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY === undefined) {
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "This-device-only Omega key custody is unavailable.",
    );
  }
  return {
    keychainService: ISSUE31_DEVICE_KEYCHAIN_SERVICE,
    keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
  };
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
  }>,
): Promise<Issue31DeviceIdentity> => {
  const options = storeOptions(input.store);
  let stored: string | null;
  try {
    stored = await input.store.getItemAsync(ISSUE31_DEVICE_KEY_STORE_KEY, options);
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
      ISSUE31_DEVICE_KEY_STORE_KEY,
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

export const clearIssue31DeviceIdentity = async (store: Issue31SecureStore): Promise<void> => {
  try {
    await store.deleteItemAsync(ISSUE31_DEVICE_KEY_STORE_KEY, storeOptions(store));
  } catch {
    throw new Issue31DeviceKeyVaultError(
      "secure_store_unavailable",
      "The Omega device key could not be removed.",
    );
  }
};

export const openExpoIssue31DeviceIdentity = async (): Promise<Issue31DeviceIdentity> => {
  const store = require("expo-secure-store") as Issue31SecureStore;
  const crypto = require("expo-crypto") as Readonly<{
    getRandomBytesAsync: (length: number) => Promise<Uint8Array>;
  }>;
  return openIssue31DeviceIdentity({
    store,
    randomBytes: (length) => crypto.getRandomBytesAsync(length),
  });
};
