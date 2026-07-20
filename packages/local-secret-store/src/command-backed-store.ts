/**
 * IDR-05 command-backed `LocalSecretStore` core.
 *
 * A desktop platform secret store is reached through its OS tool. This module
 * turns a `PlatformCommandSpec` (how to build and interpret the tool commands
 * for one platform) plus an injected `SecretStoreCommandRunnerInterface` into a
 * real `LocalSecretStore`. It knows no Nostr or Spark rule; the payload stays
 * opaque bytes.
 *
 * Because the runner is injected, the exact SAME orchestration is proven two
 * ways: a test injects a deterministic in-memory fake runner (no OS tool runs),
 * and an owner-attended run injects the real `nodeSpawnSecretStoreCommandRunner`.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect, Layer } from "effect";
import type { SecretLocator, SecretStorePlatformKind } from "./locator.ts";
import type { SecretStoreCommand, SecretStoreCommandRunnerInterface } from "./command-runner.ts";
import {
  LocalSecretStore,
  type LocalSecretStoreInterface,
  SecretNotFound,
  SecretStoreError,
} from "./secret-store.ts";

/** The interpretation of a read command result: found bytes or a definite absence. */
export type ReadInterpretation =
  | { readonly _tag: "found"; readonly payload: Uint8Array }
  | {
      readonly _tag: "not_found";
    };

/**
 * A platform command spec. It builds each OS command from a locator and payload
 * and interprets each result. It places the secret only on `stdin`, never in
 * `argv`. Every builder is a pure function, so a test asserts the argv never
 * contains the secret without running any tool.
 */
export interface PlatformCommandSpec {
  /** The platform kind this spec drives. */
  readonly platformKind: Exclude<SecretStorePlatformKind, "in_memory_test">;
  /** Build the write command. The payload rides on `stdin`. */
  readonly buildSet: (locator: SecretLocator, payload: Uint8Array) => SecretStoreCommand;
  /** Build the read command. */
  readonly buildGet: (locator: SecretLocator) => SecretStoreCommand;
  /** Build the delete command. */
  readonly buildDelete: (locator: SecretLocator) => SecretStoreCommand;
  /** Build the presence command. */
  readonly buildPresence: (locator: SecretLocator) => SecretStoreCommand;
  /** Interpret a write result: `true` for success, `false` for a tool failure. */
  readonly interpretSet: (result: { readonly code: number }) => boolean;
  /** Interpret a read result into found bytes or a definite absence. */
  readonly interpretGet: (result: {
    readonly code: number;
    readonly stdout: Uint8Array;
  }) => ReadInterpretation | "error";
  /** Interpret a delete result. A missing entry is a success (idempotent). */
  readonly interpretDelete: (result: { readonly code: number }) => boolean;
  /** Interpret a presence result: whether the entry exists. */
  readonly interpretPresence: (result: { readonly code: number }) => boolean;
}

/**
 * Build the command-backed `LocalSecretStore` implementation from a spec and an
 * injected runner. The store reports `platform_protected` custody for its
 * platform kind, so a caller can never mistake it for the in-memory adapter.
 */
export const commandBackedLocalSecretStore = (
  spec: PlatformCommandSpec,
  runner: SecretStoreCommandRunnerInterface,
): LocalSecretStoreInterface => {
  const set = Effect.fn("LocalSecretStore.set")(function* (
    locator: SecretLocator,
    payload: Uint8Array,
  ) {
    const result = yield* runner.run(spec.buildSet(locator, payload));
    if (!spec.interpretSet(result)) {
      return yield* new SecretStoreError({ reason: "write_failed" });
    }
  });

  const get = Effect.fn("LocalSecretStore.get")(function* (locator: SecretLocator) {
    const result = yield* runner.run(spec.buildGet(locator));
    const interpreted = spec.interpretGet(result);
    if (interpreted === "error")
      return yield* new SecretStoreError({ reason: "storage_unavailable" });
    if (interpreted._tag === "not_found") return yield* new SecretNotFound({ locator });
    return interpreted.payload;
  });

  const remove = Effect.fn("LocalSecretStore.delete")(function* (locator: SecretLocator) {
    const result = yield* runner.run(spec.buildDelete(locator));
    if (!spec.interpretDelete(result)) {
      return yield* new SecretStoreError({ reason: "delete_failed" });
    }
  });

  const presence = Effect.fn("LocalSecretStore.presence")(function* (locator: SecretLocator) {
    const result = yield* runner.run(spec.buildPresence(locator));
    return spec.interpretPresence(result);
  });

  const custody = Effect.fn("LocalSecretStore.custody")(function* (locator: SecretLocator) {
    const present = yield* presence(locator);
    return {
      locator,
      present,
      platformKind: spec.platformKind,
      protection: "platform_protected" as const,
    };
  });

  return { set, get, delete: remove, presence, custody };
};

/** Build the command-backed `LocalSecretStore` layer from a spec and a runner. */
export const commandBackedLocalSecretStoreLayer = (
  spec: PlatformCommandSpec,
  runner: SecretStoreCommandRunnerInterface,
): Layer.Layer<LocalSecretStore> =>
  Layer.succeed(LocalSecretStore, LocalSecretStore.of(commandBackedLocalSecretStore(spec, runner)));
