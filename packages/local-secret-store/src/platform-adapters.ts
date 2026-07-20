/**
 * IDR-01 platform secret-store adapter contracts.
 *
 * Each descriptor names one version-one platform secret store and its custody
 * protection class. These are typed ports only. This package ships NO
 * implementation that touches a real platform store yet; the real adapters
 * (macOS Keychain, Windows Credential Manager, Linux Secret Service, iOS
 * Keychain, Android Keystore) arrive in IDR-05 and run in an owner-attended run.
 *
 * `unimplementedPlatformSecretStoreLayer` builds a fail-closed `LocalSecretStore`
 * for a platform whose real adapter does not exist yet. Every operation fails
 * `adapter_unavailable`. It touches no platform store, so an autonomous run can
 * compose the graph without opening a Keychain dialog.
 */
import { Effect, Layer } from "effect";
import type { SecretStorePlatformKind } from "./locator.ts";
import { LocalSecretStore, SecretStoreError } from "./secret-store.ts";

/** A typed description of one platform secret-store adapter contract. */
export interface PlatformSecretStoreAdapterDescriptor {
  /** The platform kind this adapter serves. */
  readonly platformKind: Exclude<SecretStorePlatformKind, "in_memory_test">;
  /** A short public label for the platform store. */
  readonly serviceLabel: string;
  /** The custody protection a real adapter for this platform gives. */
  readonly protection: "platform_protected";
  /** Whether a real adapter exists in this package yet. It is `false` at IDR-01. */
  readonly implemented: boolean;
}

export const MACOS_KEYCHAIN_ADAPTER: PlatformSecretStoreAdapterDescriptor = {
  platformKind: "macos_keychain",
  serviceLabel: "macOS Keychain",
  protection: "platform_protected",
  implemented: false,
};

export const WINDOWS_CREDENTIAL_MANAGER_ADAPTER: PlatformSecretStoreAdapterDescriptor = {
  platformKind: "windows_credential_manager",
  serviceLabel: "Windows Credential Manager",
  protection: "platform_protected",
  implemented: false,
};

export const LINUX_SECRET_SERVICE_ADAPTER: PlatformSecretStoreAdapterDescriptor = {
  platformKind: "linux_secret_service",
  serviceLabel: "Linux Secret Service",
  protection: "platform_protected",
  implemented: false,
};

export const IOS_KEYCHAIN_ADAPTER: PlatformSecretStoreAdapterDescriptor = {
  platformKind: "ios_keychain",
  serviceLabel: "iOS Keychain",
  protection: "platform_protected",
  implemented: false,
};

export const ANDROID_KEYSTORE_ADAPTER: PlatformSecretStoreAdapterDescriptor = {
  platformKind: "android_keystore",
  serviceLabel: "Android Keystore-backed encrypted storage",
  protection: "platform_protected",
  implemented: false,
};

/** Every version-one platform adapter contract, in a stable order. */
export const PLATFORM_SECRET_STORE_ADAPTERS: ReadonlyArray<PlatformSecretStoreAdapterDescriptor> = [
  MACOS_KEYCHAIN_ADAPTER,
  WINDOWS_CREDENTIAL_MANAGER_ADAPTER,
  LINUX_SECRET_SERVICE_ADAPTER,
  IOS_KEYCHAIN_ADAPTER,
  ANDROID_KEYSTORE_ADAPTER,
];

/**
 * Build a fail-closed `LocalSecretStore` layer for a platform whose real adapter
 * does not exist yet. Every operation fails `adapter_unavailable`. This lets a
 * host wire the platform kind into the graph while proving no real store is
 * touched at IDR-01.
 */
export const unimplementedPlatformSecretStoreLayer = (
  descriptor: PlatformSecretStoreAdapterDescriptor,
): Layer.Layer<LocalSecretStore> => {
  const fail = Effect.fail(new SecretStoreError({ reason: "adapter_unavailable" }));
  void descriptor;
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
