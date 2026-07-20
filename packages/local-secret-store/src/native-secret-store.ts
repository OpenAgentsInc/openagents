/**
 * IDR-05 iOS Keychain and Android Keystore adapters — native-bridge INTERFACES.
 *
 * A phone secret store cannot be reached with an OS command line, so the mobile
 * adapters are typed over a `NativeSecretStoreBridge` port: an async surface a
 * React Native / Expo native module implements (iOS Keychain Services, Android
 * Keystore-backed `EncryptedSharedPreferences`). This neutral package defines the
 * port and the layer that turns any bridge into a `LocalSecretStore`; the real
 * native module is supplied by the mobile host.
 *
 * The bridge moves the payload as base64 text, so the adapter never assumes a
 * particular binary transport. A fail-closed default layer covers a host with no
 * native module, and a test injects an in-memory fake bridge, so this package
 * proves the wiring without a device.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Buffer } from "node:buffer";
import { Effect, Layer } from "effect";
import type { SecretLocator, SecretStorePlatformKind } from "./locator.ts";
import {
  LocalSecretStore,
  type LocalSecretStoreInterface,
  SecretNotFound,
  SecretStoreError,
} from "./secret-store.ts";

/** The mobile platform kinds a native bridge serves. */
export type NativeSecretStorePlatformKind = Extract<
  SecretStorePlatformKind,
  "ios_keychain" | "android_keystore"
>;

/**
 * The native secret-store bridge port. A mobile host implements it with a native
 * module. Every method keys on the service and account and moves the payload as
 * base64 text. It exposes only presence, never a partial read that could leak.
 */
export interface NativeSecretStoreBridge {
  /** Store the base64 payload for the service and account, replacing any entry. */
  readonly setSecret: (service: string, account: string, payloadBase64: string) => Promise<void>;
  /** Read the base64 payload, or `null` when no entry exists. */
  readonly getSecret: (service: string, account: string) => Promise<string | null>;
  /** Delete the entry. Deleting an absent entry succeeds (idempotent). */
  readonly deleteSecret: (service: string, account: string) => Promise<void>;
  /** Whether an entry exists, without returning the bytes. */
  readonly hasSecret: (service: string, account: string) => Promise<boolean>;
}

const toBase64 = (payload: Uint8Array): string => Buffer.from(payload).toString("base64");
const fromBase64 = (value: string): Uint8Array => new Uint8Array(Buffer.from(value, "base64"));

/**
 * Build a `LocalSecretStore` implementation over a native bridge. The store
 * reports `platform_protected` custody for its mobile platform kind, so a caller
 * can never mistake it for the in-memory adapter.
 */
export const nativeBridgeLocalSecretStore = (
  platformKind: NativeSecretStorePlatformKind,
  bridge: NativeSecretStoreBridge,
): LocalSecretStoreInterface => {
  const set = Effect.fn("LocalSecretStore.set")(function* (
    locator: SecretLocator,
    payload: Uint8Array,
  ) {
    yield* Effect.tryPromise({
      try: () => bridge.setSecret(locator.service, locator.account, toBase64(payload)),
      catch: () => new SecretStoreError({ reason: "write_failed" }),
    });
  });

  const get = Effect.fn("LocalSecretStore.get")(function* (locator: SecretLocator) {
    const value = yield* Effect.tryPromise({
      try: () => bridge.getSecret(locator.service, locator.account),
      catch: () => new SecretStoreError({ reason: "storage_unavailable" }),
    });
    if (value === null) return yield* new SecretNotFound({ locator });
    return fromBase64(value);
  });

  const remove = Effect.fn("LocalSecretStore.delete")(function* (locator: SecretLocator) {
    yield* Effect.tryPromise({
      try: () => bridge.deleteSecret(locator.service, locator.account),
      catch: () => new SecretStoreError({ reason: "delete_failed" }),
    });
  });

  const presence = Effect.fn("LocalSecretStore.presence")(function* (locator: SecretLocator) {
    return yield* Effect.tryPromise({
      try: () => bridge.hasSecret(locator.service, locator.account),
      catch: () => new SecretStoreError({ reason: "storage_unavailable" }),
    });
  });

  const custody = Effect.fn("LocalSecretStore.custody")(function* (locator: SecretLocator) {
    const present = yield* presence(locator);
    return {
      locator,
      present,
      platformKind,
      protection: "platform_protected" as const,
    };
  });

  return { set, get, delete: remove, presence, custody };
};

/** Build the native-bridge `LocalSecretStore` layer for a mobile platform. */
export const nativeBridgeLocalSecretStoreLayer = (
  platformKind: NativeSecretStorePlatformKind,
  bridge: NativeSecretStoreBridge,
): Layer.Layer<LocalSecretStore> =>
  Layer.succeed(
    LocalSecretStore,
    LocalSecretStore.of(nativeBridgeLocalSecretStore(platformKind, bridge)),
  );

/** The iOS Keychain layer over an injected native bridge. */
export const iosKeychainBridgeLayer = (
  bridge: NativeSecretStoreBridge,
): Layer.Layer<LocalSecretStore> => nativeBridgeLocalSecretStoreLayer("ios_keychain", bridge);

/** The Android Keystore layer over an injected native bridge. */
export const androidKeystoreBridgeLayer = (
  bridge: NativeSecretStoreBridge,
): Layer.Layer<LocalSecretStore> => nativeBridgeLocalSecretStoreLayer("android_keystore", bridge);

/**
 * A fail-closed native layer for a mobile host with no native module wired yet.
 * Every operation fails `adapter_unavailable` and touches no store.
 */
export const unavailableNativeSecretStoreLayer = (
  _platformKind: NativeSecretStorePlatformKind,
): Layer.Layer<LocalSecretStore> => {
  const fail = Effect.fail(new SecretStoreError({ reason: "adapter_unavailable" }));
  return Layer.succeed(
    LocalSecretStore,
    LocalSecretStore.of({
      set: () => fail,
      get: () => fail,
      delete: () => fail,
      presence: () => fail,
      custody: () => fail,
    }),
  );
};
