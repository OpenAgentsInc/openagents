/**
 * IDR-08 — the mobile custody path for the ONE identity service.
 *
 * Desktop composes the `SovereignIdentity` service over an OS-command-backed
 * secret store (macOS Keychain / Windows Credential Manager / Linux Secret
 * Service). A phone cannot be reached that way, so IDR-05 added the native-bridge
 * adapters: an async `NativeSecretStoreBridge` port a React Native / Expo native
 * module implements (iOS Keychain Services, Android Keystore-backed
 * `EncryptedSharedPreferences`). This module wires those adapters into the SAME
 * `SovereignIdentity` service, so mobile is a consumer of the one service, not a
 * second identity path.
 *
 * The RN/Expo module seam is an INTERFACE seam: this package defines the port and
 * the composition; the mobile host supplies the real native module. It is
 * buildable and testable here with an in-memory fake bridge — real device
 * custody proof is an owner-attended run.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Layer } from "effect";
import {
  androidKeystoreBridgeLayer,
  iosKeychainBridgeLayer,
  type NativeSecretStoreBridge,
  type NativeSecretStorePlatformKind,
} from "@openagentsinc/local-secret-store";
import { ManifestStore } from "./manifest.ts";
import { SovereignIdentity, sovereignIdentityLayer } from "./service.ts";

export type { NativeSecretStoreBridge, NativeSecretStorePlatformKind };

/** The mobile custody platform kinds the one identity service accepts. */
export type MobileCustodyPlatform = NativeSecretStorePlatformKind;

/**
 * Build the ONE `SovereignIdentity` service over a mobile platform secret store.
 * The mobile host injects its native bridge (the RN/Expo module) and a
 * `ManifestStore` for the public identity manifest. The resulting service is the
 * SAME `SovereignIdentity` tag the desktop and test compositions use, so the
 * `identityRef` custody and public manifest are one contract across surfaces.
 */
export const mobileSovereignIdentityLayer = (input: {
  readonly platform: MobileCustodyPlatform;
  readonly bridge: NativeSecretStoreBridge;
  readonly manifest: Layer.Layer<ManifestStore>;
}): Layer.Layer<SovereignIdentity> => {
  const secretLayer =
    input.platform === "ios_keychain"
      ? iosKeychainBridgeLayer(input.bridge)
      : androidKeystoreBridgeLayer(input.bridge);
  return sovereignIdentityLayer.pipe(
    Layer.provide(Layer.mergeAll(secretLayer, input.manifest)),
  );
};

/** The iOS Keychain composition of the one identity service. */
export const iosSovereignIdentityLayer = (input: {
  readonly bridge: NativeSecretStoreBridge;
  readonly manifest: Layer.Layer<ManifestStore>;
}): Layer.Layer<SovereignIdentity> =>
  mobileSovereignIdentityLayer({ platform: "ios_keychain", ...input });

/** The Android Keystore composition of the one identity service. */
export const androidSovereignIdentityLayer = (input: {
  readonly bridge: NativeSecretStoreBridge;
  readonly manifest: Layer.Layer<ManifestStore>;
}): Layer.Layer<SovereignIdentity> =>
  mobileSovereignIdentityLayer({ platform: "android_keystore", ...input });
