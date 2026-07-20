import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import type { NativeSecretStoreBridge } from "@openagentsinc/local-secret-store";

import { IdentityRef } from "../contract/index.ts";
import { inMemoryManifestStoreLayer } from "./manifest.ts";
import {
  androidSovereignIdentityLayer,
  iosSovereignIdentityLayer,
  mobileSovereignIdentityLayer,
  type MobileCustodyPlatform,
} from "./mobile-custody.ts";
import { SovereignIdentity } from "./service.ts";

/**
 * IDR-08 mobile custody seam proof. LOCAL-ONLY, DEVICE-FREE: an in-memory fake
 * bridge stands in for the RN/Expo native module (iOS Keychain / Android
 * Keystore). Real device custody is an owner-attended run. No network, no
 * Keychain probe, no secret in output.
 */

/** A device-free fake of the native module the mobile host would supply. */
const fakeNativeBridge = (): NativeSecretStoreBridge => {
  const store = new Map<string, string>();
  const key = (service: string, account: string) => `${service} ${account}`;
  return {
    setSecret: async (service, account, payloadBase64) => {
      store.set(key(service, account), payloadBase64);
    },
    getSecret: async (service, account) => store.get(key(service, account)) ?? null,
    deleteSecret: async (service, account) => {
      store.delete(key(service, account));
    },
    hasSecret: async (service, account) => store.has(key(service, account)),
  };
};

const identityRef = S.decodeUnknownSync(IdentityRef)("mobile-ref");

const runCustody = (platform: MobileCustodyPlatform) =>
  Effect.runPromise(
    Effect.provide(
      Effect.gen(function* () {
        const identity = yield* SovereignIdentity;
        const before = yield* identity.hasRootSecret(identityRef);
        const custody = yield* identity.rootCustody(identityRef);
        return { before, custody };
      }),
      mobileSovereignIdentityLayer({
        platform,
        bridge: fakeNativeBridge(),
        manifest: inMemoryManifestStoreLayer,
      }),
    ),
  );

describe("IDR-08 the mobile custody path composes the ONE identity service", () => {
  test("iOS Keychain custody reports platform-protected custody", async () => {
    const { before, custody } = await runCustody("ios_keychain");
    expect(before).toBe(false);
    expect(custody.present).toBe(false);
    expect(custody.platformKind).toBe("ios_keychain");
    expect(custody.protection).toBe("platform_protected");
  });

  test("Android Keystore custody reports platform-protected custody", async () => {
    const { custody } = await runCustody("android_keystore");
    expect(custody.platformKind).toBe("android_keystore");
    expect(custody.protection).toBe("platform_protected");
  });

  test("the iOS and Android convenience layers select the right platform kind", async () => {
    const bridge = fakeNativeBridge();
    const iosCustody = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const identity = yield* SovereignIdentity;
          return yield* identity.rootCustody(identityRef);
        }),
        iosSovereignIdentityLayer({ bridge, manifest: inMemoryManifestStoreLayer }),
      ),
    );
    const androidCustody = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const identity = yield* SovereignIdentity;
          return yield* identity.rootCustody(identityRef);
        }),
        androidSovereignIdentityLayer({ bridge, manifest: inMemoryManifestStoreLayer }),
      ),
    );
    expect(iosCustody.platformKind).toBe("ios_keychain");
    expect(androidCustody.platformKind).toBe("android_keystore");
  });
});
