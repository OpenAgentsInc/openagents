/**
 * IDR-01 sovereign-identity service, composed over injected ports.
 *
 * The service is composed over an injected `LocalSecretStore` (the platform
 * secret store) and an injected `ManifestStore` (the public-data store). It
 * bridges the frozen IDR-00 secret-store identifiers to the neutral
 * `LocalSecretStore` locator.
 *
 * SIGNER-BOUNDARY DISCIPLINE. The service reads secret CUSTODY and PRESENCE
 * only. It never calls `LocalSecretStore.get`, so it never reads the secret
 * bytes. A signer (IDR-06) owns the only secret-consuming surface.
 *
 * FAIL-CLOSED DISCIPLINE. The service has no create-on-missing path. It writes a
 * public manifest and a public receipt from explicit inputs, and it reads
 * custody. Opening a root secret and creating a root secret are separate IDR-02
 * operations; this service exposes neither.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Context, Effect, Layer } from "effect";
import {
  LocalSecretStore,
  type SecretCustodyState,
  type SecretLocator,
  type SecretStoreError,
  decodeSecretLocator,
  inMemoryLocalSecretStoreLayer,
} from "@openagentsinc/local-secret-store";
import {
  type IdentityRef,
  type LocalIdentityManifest,
  type LocalIdentityMigrationReceipt,
  SECRET_STORE_SERVICE,
  secretStoreAccount,
} from "../contract/index.ts";
import { ManifestStore, type ManifestStoreError, inMemoryManifestStoreLayer } from "./manifest.ts";

/**
 * The canonical secret-store locator for one identity's shared root. It bridges
 * the frozen IDR-00 service and account identifiers to the neutral locator.
 */
export const rootSecretLocator = (identityRef: IdentityRef): SecretLocator =>
  decodeSecretLocator({
    service: SECRET_STORE_SERVICE,
    account: secretStoreAccount(identityRef),
  });

/** The sovereign-identity service surface at IDR-01. */
export interface SovereignIdentityInterface {
  /** A presence-only lookup of the shared-root secret. It never reads the bytes. */
  readonly hasRootSecret: (identityRef: IdentityRef) => Effect.Effect<boolean, SecretStoreError>;
  /** The custody state of the shared-root secret. It never reads the bytes. */
  readonly rootCustody: (
    identityRef: IdentityRef,
  ) => Effect.Effect<SecretCustodyState, SecretStoreError>;
  /** Write a public identity manifest. */
  readonly writeManifest: (
    manifest: LocalIdentityManifest,
  ) => Effect.Effect<void, ManifestStoreError>;
  /** Read the public identity manifest for an identity, or `null` when absent. */
  readonly manifestFor: (
    identityRef: IdentityRef,
  ) => Effect.Effect<LocalIdentityManifest | null, ManifestStoreError>;
  /** Record a public-safe migration receipt. */
  readonly recordMigrationReceipt: (
    receipt: LocalIdentityMigrationReceipt,
  ) => Effect.Effect<void, ManifestStoreError>;
  /** Read a public-safe migration receipt by reference, or `null` when absent. */
  readonly receiptFor: (
    receiptRef: string,
  ) => Effect.Effect<LocalIdentityMigrationReceipt | null, ManifestStoreError>;
}

/** The `SovereignIdentity` service tag. */
export class SovereignIdentity extends Context.Service<
  SovereignIdentity,
  SovereignIdentityInterface
>()("sovereign-identity.SovereignIdentity") {}

/**
 * The `SovereignIdentity` layer, composed over the injected `LocalSecretStore`
 * and `ManifestStore` ports. It requires both ports; a host or test supplies
 * them.
 */
export const sovereignIdentityLayer = Layer.effect(
  SovereignIdentity,
  Effect.gen(function* () {
    const secrets = yield* LocalSecretStore;
    const manifests = yield* ManifestStore;

    const hasRootSecret = Effect.fn("SovereignIdentity.hasRootSecret")(function* (
      identityRef: IdentityRef,
    ) {
      return yield* secrets.presence(rootSecretLocator(identityRef));
    });

    const rootCustody = Effect.fn("SovereignIdentity.rootCustody")(function* (
      identityRef: IdentityRef,
    ) {
      return yield* secrets.custody(rootSecretLocator(identityRef));
    });

    return SovereignIdentity.of({
      hasRootSecret,
      rootCustody,
      writeManifest: (manifest) => manifests.writeManifest(manifest),
      manifestFor: (identityRef) => manifests.readManifest(identityRef),
      recordMigrationReceipt: (receipt) => manifests.writeReceipt(receipt),
      receiptFor: (receiptRef) => manifests.readReceipt(receiptRef),
    });
  }),
);

/**
 * A test composition of the sovereign-identity service over the in-memory secret
 * store and the in-memory manifest store. It uses no platform store, no
 * network, and no real secret.
 */
export const inMemorySovereignIdentityLayer: Layer.Layer<SovereignIdentity> =
  sovereignIdentityLayer.pipe(
    Layer.provide(Layer.mergeAll(inMemoryLocalSecretStoreLayer, inMemoryManifestStoreLayer)),
  );
