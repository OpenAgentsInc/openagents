import { Schema } from "effect";

import type {
  Issue31DeviceKeyPlatform,
  Issue31SecureStore,
  Issue31SecureStoreOptions,
} from "../workroom/issue31-device-key-vault";

export const SARAH_VOICE_SESSION_STORE_KEY = "openagents.mobile.sarah.voice-session.v1" as const;
export const SARAH_VOICE_SESSION_KEYCHAIN_SERVICE =
  "com.openagents.mobile.sarah-voice-session" as const;

const SessionRecord = Schema.Struct({
  schemaVersion: Schema.Literal(1),
  publicKeyHex: Schema.String.check(Schema.isPattern(/^[0-9a-f]{64}$/u)),
  ownerRef: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(256)),
  accessToken: Schema.String.check(Schema.isPattern(/^oa_omega_[A-Za-z0-9_-]{32,256}$/u)),
  expiresAtMs: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
});

export type SarahVoiceStoredSession = typeof SessionRecord.Type;

const optionsFor = (
  store: Issue31SecureStore,
  platform: Issue31DeviceKeyPlatform,
): Issue31SecureStoreOptions => {
  if (platform === "ios" && store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY !== undefined) {
    return {
      keychainService: SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
      keychainAccessible: store.AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY,
    };
  }
  if (platform === "android") {
    return {
      keychainService: SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
      requireAuthentication: false,
    };
  }
  throw new Error("Protected Sarah voice session storage is unavailable.");
};

export interface SarahVoiceSessionVault {
  readonly read: (publicKeyHex: string, nowMs: number) => Promise<SarahVoiceStoredSession | null>;
  readonly write: (record: SarahVoiceStoredSession) => Promise<void>;
  readonly clear: () => Promise<void>;
}

export const makeSarahVoiceSessionVault = (
  store: Issue31SecureStore,
  platform: Issue31DeviceKeyPlatform,
): SarahVoiceSessionVault => {
  const options = optionsFor(store, platform);
  return {
    read: async (publicKeyHex, nowMs) => {
      const value = await store.getItemAsync(SARAH_VOICE_SESSION_STORE_KEY, options);
      if (value === null) return null;
      try {
        const decoded = Schema.decodeUnknownSync(SessionRecord)(JSON.parse(value), {
          onExcessProperty: "error",
        });
        if (decoded.publicKeyHex !== publicKeyHex || decoded.expiresAtMs <= nowMs + 5_000) {
          await store.deleteItemAsync(SARAH_VOICE_SESSION_STORE_KEY, options);
          return null;
        }
        return decoded;
      } catch {
        await store.deleteItemAsync(SARAH_VOICE_SESSION_STORE_KEY, options);
        return null;
      }
    },
    write: (record) =>
      store.setItemAsync(SARAH_VOICE_SESSION_STORE_KEY, JSON.stringify(record), options),
    clear: () => store.deleteItemAsync(SARAH_VOICE_SESSION_STORE_KEY, options),
  };
};
