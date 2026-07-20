/**
 * IDR-01 local secret-store port.
 *
 * `LocalSecretStore` is the platform secret-store INTERFACE. It stores an opaque
 * encrypted payload by locator, reads it back, deletes it, answers a
 * presence-only lookup, and reports custody state. It has NO knowledge of Nostr,
 * Spark, or any derivation rule. The bytes are opaque to this package.
 *
 * A real platform adapter (IDR-05) supplies encryption and access control. This
 * package ships one real adapter only: the in-memory test adapter. The platform
 * adapter contracts here have no implementation that touches a real store yet.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Context, type Effect, Schema as S } from "effect";
import type { SecretCustodyState, SecretLocator } from "./locator.ts";

/**
 * A typed secret-store failure. `adapter_unavailable` means the platform adapter
 * has no real implementation yet or the platform store is not reachable.
 */
export class SecretStoreError extends S.TaggedErrorClass<SecretStoreError>()(
  "local-secret-store.SecretStoreError",
  {
    reason: S.Literals([
      "storage_unavailable",
      "adapter_unavailable",
      "invalid_locator",
      "write_failed",
      "delete_failed",
    ]),
  },
) {}

/** A typed "no entry at this locator" failure for a read that expected one. */
export class SecretNotFound extends S.TaggedErrorClass<SecretNotFound>()(
  "local-secret-store.SecretNotFound",
  { locator: S.Struct({ service: S.String, account: S.String }) },
) {}

/**
 * The local secret-store interface. Every method takes a locator. The stored
 * payload is opaque bytes; this package never reads its structure.
 */
export interface LocalSecretStoreInterface {
  /** Write an opaque encrypted payload at the locator, replacing any existing entry. */
  readonly set: (
    locator: SecretLocator,
    payload: Uint8Array,
  ) => Effect.Effect<void, SecretStoreError>;
  /** Read the opaque payload. It fails `SecretNotFound` when no entry exists. */
  readonly get: (
    locator: SecretLocator,
  ) => Effect.Effect<Uint8Array, SecretNotFound | SecretStoreError>;
  /** Delete the entry. Deleting an absent entry succeeds and is idempotent. */
  readonly delete: (locator: SecretLocator) => Effect.Effect<void, SecretStoreError>;
  /** A presence-only lookup. It returns whether an entry exists, never the bytes. */
  readonly presence: (locator: SecretLocator) => Effect.Effect<boolean, SecretStoreError>;
  /** The custody state of the locator: presence, platform kind, and protection. */
  readonly custody: (locator: SecretLocator) => Effect.Effect<SecretCustodyState, SecretStoreError>;
}

/**
 * The `LocalSecretStore` service tag. A host composes one platform adapter layer
 * or the in-memory adapter behind this tag.
 */
export class LocalSecretStore extends Context.Service<
  LocalSecretStore,
  LocalSecretStoreInterface
>()("local-secret-store.LocalSecretStore") {}
