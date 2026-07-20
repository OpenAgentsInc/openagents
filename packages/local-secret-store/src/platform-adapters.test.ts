import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { decodeSecretLocator } from "./locator.ts";
import {
  MACOS_KEYCHAIN_ADAPTER,
  PLATFORM_SECRET_STORE_ADAPTERS,
  unimplementedPlatformSecretStoreLayer,
} from "./platform-adapters.ts";
import { LocalSecretStore, SecretStoreError } from "./secret-store.ts";

const locator = decodeSecretLocator({
  service: "com.openagents.identity.root.v1",
  account: "identity:test-ref",
});

describe("platform secret-store adapter contracts", () => {
  test("all five version-one platforms are present and unimplemented at IDR-01", () => {
    expect(PLATFORM_SECRET_STORE_ADAPTERS.map((a) => a.platformKind)).toEqual([
      "macos_keychain",
      "windows_credential_manager",
      "linux_secret_service",
      "ios_keychain",
      "android_keystore",
    ]);
    expect(PLATFORM_SECRET_STORE_ADAPTERS.every((a) => a.implemented === false)).toBe(true);
    expect(PLATFORM_SECRET_STORE_ADAPTERS.every((a) => a.protection === "platform_protected")).toBe(
      true,
    );
  });

  test("the unimplemented platform layer fails adapter_unavailable and touches no store", async () => {
    const error = await Effect.runPromise(
      Effect.provide(
        Effect.flip(
          Effect.gen(function* () {
            const store = yield* LocalSecretStore;
            return yield* store.presence(locator);
          }),
        ),
        unimplementedPlatformSecretStoreLayer(MACOS_KEYCHAIN_ADAPTER),
      ),
    );
    expect(error).toBeInstanceOf(SecretStoreError);
    expect(error.reason).toBe("adapter_unavailable");
  });
});
