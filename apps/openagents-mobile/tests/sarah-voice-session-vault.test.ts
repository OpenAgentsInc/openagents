import { describe, expect, test } from "vite-plus/test";

import {
  SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
  SARAH_VOICE_SESSION_STORE_KEY,
  makeSarahVoiceSessionVault,
} from "../src/sarah-voice/session-vault.ts";
import type {
  Issue31SecureStore,
  Issue31SecureStoreOptions,
} from "../src/workroom/issue31-device-key-vault.ts";

const publicKeyHex = "a".repeat(64);
const record = {
  schemaVersion: 1 as const,
  publicKeyHex,
  ownerRef: "user-1",
  accessToken: `oa_omega_${"b".repeat(43)}`,
  expiresAtMs: 20_000,
};

const makeStore = () => {
  let value: string | null = null;
  const calls: Array<Readonly<{ operation: string; options?: Issue31SecureStoreOptions }>> = [];
  const store: Issue31SecureStore = {
    AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: "this-device",
    getItemAsync: async (_key, options) => {
      calls.push({ operation: "get", options });
      return value;
    },
    setItemAsync: async (_key, next, options) => {
      calls.push({ operation: "set", options });
      value = next;
    },
    deleteItemAsync: async (_key, options) => {
      calls.push({ operation: "delete", options });
      value = null;
    },
  };
  return { calls, readRaw: () => value, setRaw: (next: string) => { value = next; }, store };
};

describe("Sarah mobile protected session vault", () => {
  test("uses this-device iOS custody and returns a valid bound session", async () => {
    const fixture = makeStore();
    const vault = makeSarahVoiceSessionVault(fixture.store, "ios");
    await vault.write(record);
    expect(fixture.readRaw()).toContain(record.accessToken);
    expect(await vault.read(publicKeyHex, 10_000)).toEqual(record);
    expect(fixture.calls).toEqual([
      {
        operation: "set",
        options: {
          keychainService: SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
          keychainAccessible: "this-device",
        },
      },
      {
        operation: "get",
        options: {
          keychainService: SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
          keychainAccessible: "this-device",
        },
      },
    ]);
  });

  test("deletes expired, wrong-device, and malformed records", async () => {
    const fixture = makeStore();
    const vault = makeSarahVoiceSessionVault(fixture.store, "android");

    fixture.setRaw(JSON.stringify(record));
    expect(await vault.read("c".repeat(64), 10_000)).toBeNull();
    fixture.setRaw(JSON.stringify(record));
    expect(await vault.read(publicKeyHex, 16_000)).toBeNull();
    fixture.setRaw("{bad json");
    expect(await vault.read(publicKeyHex, 10_000)).toBeNull();

    expect(fixture.calls.filter((call) => call.operation === "delete")).toHaveLength(3);
    expect(fixture.calls.at(-1)?.options).toEqual({
      keychainService: SARAH_VOICE_SESSION_KEYCHAIN_SERVICE,
      requireAuthentication: false,
    });
  });

  test("fails closed when protected platform storage is not available", () => {
    const fixture = makeStore();
    expect(() => makeSarahVoiceSessionVault(fixture.store, "unknown")).toThrow(
      "Protected Sarah voice session storage is unavailable.",
    );
    expect(SARAH_VOICE_SESSION_STORE_KEY).toBe(
      "openagents.mobile.sarah.voice-session.v1",
    );
  });
});
