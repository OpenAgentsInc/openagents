import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { inMemoryLocalSecretStoreLayer } from "./in-memory.ts";
import { decodeSecretLocator } from "./locator.ts";
import { LocalSecretStore, SecretNotFound } from "./secret-store.ts";

/**
 * The in-memory adapter proves the get/set/delete/presence/custody port. No
 * real secret is used: every payload here is synthetic test bytes.
 */

const locator = decodeSecretLocator({
  service: "com.openagents.identity.root.v1",
  account: "identity:test-ref",
});

const other = decodeSecretLocator({
  service: "com.openagents.identity.root.v1",
  account: "identity:absent-ref",
});

const payload = new Uint8Array([1, 2, 3, 4, 5]);

const run = <A, E>(effect: Effect.Effect<A, E, LocalSecretStore>): Promise<A> =>
  Effect.runPromise(Effect.provide(effect, inMemoryLocalSecretStoreLayer));

describe("in-memory local secret store", () => {
  test("set then get round-trips the opaque payload", async () => {
    const result = await run(
      Effect.gen(function* () {
        const store = yield* LocalSecretStore;
        yield* store.set(locator, payload);
        return yield* store.get(locator);
      }),
    );
    expect([...result]).toEqual([1, 2, 3, 4, 5]);
  });

  test("get returns a copy the caller cannot use to mutate stored state", async () => {
    const second = await run(
      Effect.gen(function* () {
        const store = yield* LocalSecretStore;
        yield* store.set(locator, payload);
        const first = yield* store.get(locator);
        first[0] = 99;
        return yield* store.get(locator);
      }),
    );
    expect(second[0]).toBe(1);
  });

  test("presence is true after set and false for an absent locator", async () => {
    const [here, absent] = await run(
      Effect.gen(function* () {
        const store = yield* LocalSecretStore;
        yield* store.set(locator, payload);
        return [yield* store.presence(locator), yield* store.presence(other)] as const;
      }),
    );
    expect(here).toBe(true);
    expect(absent).toBe(false);
  });

  test("delete removes the entry and is idempotent", async () => {
    const [afterDelete, secondDelete] = await run(
      Effect.gen(function* () {
        const store = yield* LocalSecretStore;
        yield* store.set(locator, payload);
        yield* store.delete(locator);
        const afterDelete = yield* store.presence(locator);
        yield* store.delete(locator);
        return [afterDelete, yield* store.presence(locator)] as const;
      }),
    );
    expect(afterDelete).toBe(false);
    expect(secondDelete).toBe(false);
  });

  test("get on an absent locator fails SecretNotFound", async () => {
    const error = await run(
      Effect.flip(
        Effect.gen(function* () {
          const store = yield* LocalSecretStore;
          return yield* store.get(other);
        }),
      ),
    );
    expect(error).toBeInstanceOf(SecretNotFound);
  });

  test("custody reports in-memory protection, never platform protection", async () => {
    const custody = await run(
      Effect.gen(function* () {
        const store = yield* LocalSecretStore;
        yield* store.set(locator, payload);
        return yield* store.custody(locator);
      }),
    );
    expect(custody.present).toBe(true);
    expect(custody.platformKind).toBe("in_memory_test");
    expect(custody.protection).toBe("in_memory_unprotected");
  });
});
