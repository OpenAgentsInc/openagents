/**
 * IDR-01 in-memory secret-store adapter.
 *
 * This is the one real `LocalSecretStore` implementation in this package. It
 * keeps opaque payloads in process memory only. It gives NO platform protection
 * and reports `in_memory_unprotected` custody, so a test can never mistake it
 * for real Keychain custody. It touches no platform store and no network.
 *
 * The adapter copies every payload on write and on read, so a caller cannot
 * mutate the stored bytes through a shared buffer reference.
 */
import { Effect, Layer, Ref } from "effect";
import { type SecretLocator, secretLocatorKey } from "./locator.ts";
import { LocalSecretStore, SecretNotFound } from "./secret-store.ts";

const copyBytes = (payload: Uint8Array): Uint8Array => Uint8Array.from(payload);

/**
 * The in-memory `LocalSecretStore` layer. It stores a copy of each payload in a
 * `Ref`-guarded map keyed by locator, so concurrent effects share one state.
 */
export const inMemoryLocalSecretStoreLayer = Layer.effect(
  LocalSecretStore,
  Effect.gen(function* () {
    const store = yield* Ref.make(new Map<string, Uint8Array>());

    const set = Effect.fn("LocalSecretStore.set")(function* (
      locator: SecretLocator,
      payload: Uint8Array,
    ) {
      const key = secretLocatorKey(locator);
      yield* Ref.update(store, (map) => new Map(map).set(key, copyBytes(payload)));
    });

    const get = Effect.fn("LocalSecretStore.get")(function* (locator: SecretLocator) {
      const map = yield* Ref.get(store);
      const stored = map.get(secretLocatorKey(locator));
      if (stored === undefined) return yield* new SecretNotFound({ locator });
      return copyBytes(stored);
    });

    const remove = Effect.fn("LocalSecretStore.delete")(function* (locator: SecretLocator) {
      const key = secretLocatorKey(locator);
      yield* Ref.update(store, (map) => {
        const next = new Map(map);
        next.delete(key);
        return next;
      });
    });

    const presence = Effect.fn("LocalSecretStore.presence")(function* (locator: SecretLocator) {
      const map = yield* Ref.get(store);
      return map.has(secretLocatorKey(locator));
    });

    const custody = Effect.fn("LocalSecretStore.custody")(function* (locator: SecretLocator) {
      const map = yield* Ref.get(store);
      return {
        locator,
        present: map.has(secretLocatorKey(locator)),
        platformKind: "in_memory_test" as const,
        protection: "in_memory_unprotected" as const,
      };
    });

    return LocalSecretStore.of({ set, get, delete: remove, presence, custody });
  }),
);
