import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import {
  DesktopSecureStorageInMemoryTestLayer,
  DesktopSecureStorageService,
} from "../src/main/desktopSecureStorage";

describe("desktop secure storage", () => {
  it.effect("stores and retrieves secrets without projection", () =>
    Effect.gen(function* () {
      const storage = yield* DesktopSecureStorageService;

      yield* storage.setSecret("wallet.passphrase", "super-secret");
      const secret = yield* storage.getSecret("wallet.passphrase");
      expect(secret).toBe("super-secret");

      const projected = {
        key: "wallet.passphrase",
        stored: secret !== null,
      };
      expect(JSON.stringify(projected)).not.toContain("super-secret");

      yield* storage.deleteSecret("wallet.passphrase");
      const deleted = yield* storage.getSecret("wallet.passphrase");
      expect(deleted).toBeNull();
    }).pipe(Effect.provide(DesktopSecureStorageInMemoryTestLayer)),
  );
});
