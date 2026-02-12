import { Effect } from "effect";
import { describe, expect, it } from "@effect/vitest";

import type { L402Credential } from "@openagentsinc/lightning-effect";

import { DesktopSecureStorageInMemoryTestLayer } from "../src/main/desktopSecureStorage";
import { makeL402CredentialCacheStore } from "../src/main/l402CredentialCacheStore";

const sampleCredential = (overrides?: Partial<L402Credential>): L402Credential => ({
  host: "seller.example.com",
  scope: "default",
  macaroon: "macaroon",
  preimageHex: "ab".repeat(32),
  amountMsats: 2_500,
  issuedAtMs: 1_000,
  ...(overrides ?? {}),
});

describe("desktop L402 credential cache store", () => {
  it.effect("persists entries with TTL + invalidation", () =>
    Effect.gen(function* () {
      const store1 = yield* makeL402CredentialCacheStore;

      yield* store1.putByHost(
        "Seller.Example.Com",
        "default",
        sampleCredential(),
        { ttlMs: 1_000 },
      );

      const hit = yield* store1.getByHost("seller.example.com", "default", 1_500);
      expect(hit._tag).toBe("hit");

      const store2 = yield* makeL402CredentialCacheStore;
      const hit2 = yield* store2.getByHost("seller.example.com", "default", 1_500);
      expect(hit2._tag).toBe("hit");

      const stale = yield* store2.getByHost("seller.example.com", "default", 2_500);
      expect(stale._tag).toBe("stale");

      yield* store2.markInvalid("seller.example.com", "default");
      const miss = yield* store2.getByHost("seller.example.com", "default", 1_500);
      expect(miss._tag).toBe("miss");
    }).pipe(Effect.provide(DesktopSecureStorageInMemoryTestLayer)),
  );
});

