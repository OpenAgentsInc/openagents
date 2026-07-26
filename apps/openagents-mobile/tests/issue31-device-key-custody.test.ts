import { describe, expect, test } from "vite-plus/test";

import {
  ISSUE31_ANDROID_DEVICE_KEY_OPTIONS,
  ISSUE31_DEVICE_KEYCHAIN_SERVICE,
  ISSUE31_DEVICE_KEY_STORE_KEY,
  Issue31DeviceKeyVaultError,
  clearIssue31DeviceIdentity,
  openIssue31DeviceIdentity,
  type Issue31SecureStore,
  type Issue31SecureStoreOptions,
} from "../src/workroom/issue31-device-key-vault";

const deviceSecret = new Uint8Array(32).fill(7);

/**
 * expo-secure-store re-exports `AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY` straight
 * off the native module. Only the iOS module declares accessibility constants
 * (`ios/SecureStoreModule.swift`); `android/.../SecureStoreModule.kt` declares
 * none, so on Android the JS value is genuinely `undefined`. These two shapes
 * are exactly that asymmetry.
 */
const iosShapedStore = (): StoreProbe => storeProbe({ withAccessibilityConstant: true });
const androidShapedStore = (): StoreProbe => storeProbe({ withAccessibilityConstant: false });

type StoreProbe = Readonly<{
  store: Issue31SecureStore;
  values: Map<string, string>;
  options: Issue31SecureStoreOptions[];
}>;

const IOS_ACCESSIBILITY_CONSTANT = "kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly";

const storeProbe = (
  input: Readonly<{ withAccessibilityConstant: boolean }>,
): StoreProbe => {
  const values = new Map<string, string>();
  const options: Issue31SecureStoreOptions[] = [];
  return {
    values,
    options,
    store: {
      ...(input.withAccessibilityConstant
        ? { AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: IOS_ACCESSIBILITY_CONSTANT }
        : {}),
      getItemAsync: async (key, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        return values.get(key) ?? null;
      },
      setItemAsync: async (key, value, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        values.set(key, value);
      },
      deleteItemAsync: async (key, nextOptions) => {
        if (nextOptions !== undefined) options.push(nextOptions);
        values.delete(key);
      },
    },
  };
};

describe("Issue 31 device key custody across platforms", () => {
  test("an iOS store still gets this-device-only keychain accessibility", async () => {
    const probe = iosShapedStore();
    const identity = await openIssue31DeviceIdentity({
      store: probe.store,
      randomBytes: async () => new Uint8Array(deviceSecret),
      platform: "ios",
    });
    identity.close();

    expect(probe.values.has(ISSUE31_DEVICE_KEY_STORE_KEY)).toBe(true);
    expect(probe.options.length).toBeGreaterThan(0);
    for (const options of probe.options) {
      expect(options.keychainService).toBe(ISSUE31_DEVICE_KEYCHAIN_SERVICE);
      expect(options.keychainAccessible).toBe(IOS_ACCESSIBILITY_CONSTANT);
      // iOS custody must never be downgraded onto the Android record.
      expect(options.requireAuthentication).toBeUndefined();
    }
  });

  test("an Android store mints and stores a device key with the documented Android options", async () => {
    const probe = androidShapedStore();
    const first = await openIssue31DeviceIdentity({
      store: probe.store,
      randomBytes: async () => new Uint8Array(deviceSecret),
      platform: "android",
    });

    expect(first.npub.startsWith("npub1")).toBe(true);
    expect(first.publicKeyHex).toMatch(/^[0-9a-f]{64}$/);
    expect(probe.values.has(ISSUE31_DEVICE_KEY_STORE_KEY)).toBe(true);

    // The stored key must survive a reopen rather than being regenerated.
    const second = await openIssue31DeviceIdentity({
      store: probe.store,
      randomBytes: async () => {
        throw new Error("must not regenerate the Android device key");
      },
      platform: "android",
    });
    expect(second.publicKeyHex).toBe(first.publicKeyHex);

    expect(probe.options.length).toBeGreaterThan(0);
    for (const options of probe.options) {
      // Dedicated AndroidKeyStore alias for Omega, not the shared default.
      expect(options.keychainService).toBe(ISSUE31_DEVICE_KEYCHAIN_SERVICE);
      // Matches the iOS class, which is not user-auth gated either.
      expect(options.requireAuthentication).toBe(false);
      // The Android record has no accessibility field; never pretend it does.
      expect(options.keychainAccessible).toBeUndefined();
    }
    expect(ISSUE31_ANDROID_DEVICE_KEY_OPTIONS).toEqual({
      keychainService: ISSUE31_DEVICE_KEYCHAIN_SERVICE,
      requireAuthentication: false,
    });

    first.close();
    second.close();
    await clearIssue31DeviceIdentity(probe.store, "android");
    expect(probe.values.size).toBe(0);
  });

  test("a missing accessibility constant on iOS still fails closed instead of downgrading", async () => {
    // An iOS binary whose native module failed to export its accessibility
    // classes must not silently fall back to the weaker Android options.
    await expect(
      openIssue31DeviceIdentity({
        store: androidShapedStore().store,
        randomBytes: async () => new Uint8Array(deviceSecret),
        platform: "ios",
      }),
    ).rejects.toMatchObject({
      reason: "secure_store_unavailable",
    } satisfies Partial<Issue31DeviceKeyVaultError>);
  });

  test("an unidentified platform without the constant fails closed", async () => {
    await expect(
      openIssue31DeviceIdentity({
        store: androidShapedStore().store,
        randomBytes: async () => new Uint8Array(deviceSecret),
        platform: "unknown",
      }),
    ).rejects.toMatchObject({
      reason: "secure_store_unavailable",
    } satisfies Partial<Issue31DeviceKeyVaultError>);
  });

  test("a store broken in some other way fails closed without minting key material", async () => {
    // The accessibility constant used to double as a native-link canary. Android
    // never had that canary, so an unlinked call surface must be caught directly
    // — and caught up front, before any device key is generated. A store that
    // cannot hold a key must never be handed one, not even transiently in memory.
    const unlinked = {
      AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: IOS_ACCESSIBILITY_CONSTANT,
    } as unknown as Issue31SecureStore;
    const readableButUnwritable = {
      AFTER_FIRST_UNLOCK_THIS_DEVICE_ONLY: IOS_ACCESSIBILITY_CONSTANT,
      getItemAsync: async () => null,
    } as unknown as Issue31SecureStore;

    for (const platform of ["ios", "android", "unknown"] as const) {
      for (const store of [unlinked, readableButUnwritable]) {
        let minted = 0;
        await expect(
          openIssue31DeviceIdentity({
            store,
            randomBytes: async () => {
              minted += 1;
              return new Uint8Array(deviceSecret);
            },
            platform,
          }),
        ).rejects.toMatchObject({
          reason: "secure_store_unavailable",
        } satisfies Partial<Issue31DeviceKeyVaultError>);
        expect(minted).toBe(0);
      }
    }
  });

  test("a store that throws on read fails closed rather than minting a second key", async () => {
    const throwing: Issue31SecureStore = {
      getItemAsync: async () => {
        throw new Error("keystore entry permanently invalidated");
      },
      setItemAsync: async () => {},
      deleteItemAsync: async () => {},
    };
    await expect(
      openIssue31DeviceIdentity({
        store: throwing,
        randomBytes: async () => new Uint8Array(deviceSecret),
        platform: "android",
      }),
    ).rejects.toMatchObject({
      reason: "secure_store_unavailable",
    } satisfies Partial<Issue31DeviceKeyVaultError>);
  });
});
