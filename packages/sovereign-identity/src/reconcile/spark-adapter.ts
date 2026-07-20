/**
 * IDR-04 Spark comparison adapters.
 *
 * A reconciliation run must compare a decoded candidate's PUBLIC Spark identity
 * against the public local records. The audit is explicit that different Spark
 * runtimes derive DIFFERENT public fingerprints from the same mnemonic, so a
 * recovery tool must never assume one fingerprint proves another. Each runtime
 * therefore has its own typed comparison adapter:
 *
 * - `rust_spark` — the EXACT frozen reference (`m/44'/0'/0'/0/0`, empty
 *   passphrase). It is offline-derivable, so this package owns it fully.
 * - `breez_spark` — the EXACT Breez Spark comparison profile. The Breez SDK owns
 *   its internal wallet derivation, and this neutral package must not link a
 *   wallet SDK (see `boundary.test.ts`). The adapter therefore takes a BOUNDED
 *   seed-material deriver that the composition root injects (the real Breez SDK
 *   in IDR-07; a deterministic stand-in in tests). The seed reaches the deriver
 *   only inside the bounded `use` scope and its buffer is cleared afterwards.
 * - `ldk` — a declared-but-DEFERRED seam. The later LDK branch is a different
 *   profile (LDK node entropy) that cannot restore the shared Nostr/Spark root,
 *   so the adapter is registered but not yet wired; it never touches the secret.
 *
 * Every adapter yields ONLY a public `SparkAdapterFingerprint`. The mnemonic and
 * the seed never leave the bounded scope.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { mnemonicToSeedSync } from "@scure/bip39";
import { Effect, Schema as S } from "effect";
import {
  deriveSovereignIdentityPublic,
  EMPTY_BIP39_PASSPHRASE,
  normalizeMnemonic,
  type SparkAdapterFingerprint,
  type SparkAdapterKind,
} from "../contract/index.ts";
import type { RecoveredSecret } from "../decode/boundary.ts";

/** Whether a comparison adapter can derive a fingerprint now, or is deferred. */
export const SparkAdapterAvailability = S.Literals(["exact", "deferred"]);
export type SparkAdapterAvailability = typeof SparkAdapterAvailability.Type;

/**
 * A typed Spark-comparison failure. It carries the adapter kind and a coarse
 * reason ONLY. It never carries the mnemonic, the seed, or any private material,
 * so a logged error can never leak the secret.
 *
 * - `deferred`: the adapter is registered but not yet wired (LDK). The engine
 *   records the deferral and continues rather than treating it as a mismatch.
 * - `derive_failed`: the injected deriver or the reference derivation failed.
 */
export class SparkComparisonError extends S.TaggedErrorClass<SparkComparisonError>()(
  "sovereign-identity.SparkComparisonError",
  {
    adapter: S.Literals(["rust_spark", "breez_spark", "ldk"]),
    reason: S.Literals(["deferred", "derive_failed"]),
  },
) {}

/**
 * A Spark comparison adapter. It derives the PUBLIC Spark fingerprint for one
 * candidate under its profile, inside the bounded secret scope. It exposes no
 * private material.
 */
export interface SparkComparisonAdapter {
  /** The Spark runtime this adapter compares against. */
  readonly kind: SparkAdapterKind;
  /** Whether the adapter can derive now (`exact`) or is a deferred seam. */
  readonly availability: SparkAdapterAvailability;
  /**
   * Derive this adapter's public fingerprint for a candidate. A deferred adapter
   * fails with `SparkComparisonError(reason: "deferred")`; the engine records
   * that and moves on rather than failing the whole run.
   */
  readonly deriveFingerprint: (
    secret: RecoveredSecret,
  ) => Effect.Effect<SparkAdapterFingerprint, SparkComparisonError>;
}

/**
 * The EXACT Rust Spark comparison adapter. It derives the frozen reference
 * fingerprint (`sparkBip32FingerprintHex`) and public key from the bounded
 * secret. The mnemonic never leaves the `use` scope.
 */
export const rustSparkComparisonAdapter: SparkComparisonAdapter = {
  kind: "rust_spark",
  availability: "exact",
  deriveFingerprint: Effect.fn("SovereignIdentity.rustSparkComparisonAdapter.deriveFingerprint")(
    function* (secret) {
      return yield* secret.use((mnemonic) =>
        Effect.try({
          try: (): SparkAdapterFingerprint => {
            const pub = deriveSovereignIdentityPublic(mnemonic);
            return {
              adapter: "rust_spark",
              fingerprintHex: pub.sparkBip32FingerprintHex,
              publicKeyHex: pub.sparkPublicKeyHex,
            };
          },
          catch: () => new SparkComparisonError({ adapter: "rust_spark", reason: "derive_failed" }),
        }),
      );
    },
  ),
};

/**
 * The public output of a bounded Breez seed deriver. It carries public
 * identifiers only. In production the Breez SDK produces these inside a bounded
 * callback; the seed bytes never appear in the result.
 */
export interface BreezSeedFingerprint {
  /** The public Breez Spark wallet fingerprint hex. */
  readonly fingerprintHex: string;
  /** The optional public key hex, when the runtime exposes one. */
  readonly publicKeyHex?: string;
}

/**
 * The bounded Breez seed-material deriver the composition root injects. It
 * receives the BIP-39 seed bytes (empty passphrase) inside the bounded scope and
 * returns ONLY the public fingerprint. It must not persist the seed or copy it
 * into a process-wide object. The real implementation is the Breez SDK (IDR-07);
 * a test injects a deterministic stand-in.
 */
export interface BreezSeedFingerprintDeriver {
  readonly deriveSeedFingerprint: (
    seed: Uint8Array,
  ) => Effect.Effect<BreezSeedFingerprint, unknown>;
}

/**
 * Build the EXACT Breez Spark comparison adapter over an injected bounded seed
 * deriver. The adapter computes the BIP-39 seed inside the bounded `use` scope,
 * hands it to the deriver, maps its public output to a `SparkAdapterFingerprint`,
 * and clears the seed buffer afterwards. The mnemonic and the seed never escape.
 */
export const makeBreezSparkComparisonAdapter = (
  deriver: BreezSeedFingerprintDeriver,
): SparkComparisonAdapter => ({
  kind: "breez_spark",
  availability: "exact",
  deriveFingerprint: Effect.fn("SovereignIdentity.breezSparkComparisonAdapter.deriveFingerprint")(
    function* (secret) {
      return yield* secret.use((mnemonic) => {
        const seed = mnemonicToSeedSync(normalizeMnemonic(mnemonic), EMPTY_BIP39_PASSPHRASE);
        return deriver.deriveSeedFingerprint(seed).pipe(
          Effect.map(
            (out): SparkAdapterFingerprint => ({
              adapter: "breez_spark",
              fingerprintHex: out.fingerprintHex,
              ...(out.publicKeyHex === undefined ? {} : { publicKeyHex: out.publicKeyHex }),
            }),
          ),
          Effect.mapError(
            () => new SparkComparisonError({ adapter: "breez_spark", reason: "derive_failed" }),
          ),
          // Clear the seed buffer regardless of success or failure.
          Effect.ensuring(Effect.sync(() => seed.fill(0))),
        );
      });
    },
  ),
});

/**
 * The declared-but-DEFERRED LDK comparison adapter. The LDK branch is a different
 * profile (later LDK node entropy) and cannot restore the shared Nostr/Spark
 * root, so a real LDK comparison is a later packet. The adapter never touches the
 * secret; it fails with the `deferred` reason so the engine records it and moves
 * on.
 */
export const ldkSparkComparisonAdapter: SparkComparisonAdapter = {
  kind: "ldk",
  availability: "deferred",
  deriveFingerprint: Effect.fn("SovereignIdentity.ldkSparkComparisonAdapter.deriveFingerprint")(
    function* (_secret) {
      return yield* Effect.fail(new SparkComparisonError({ adapter: "ldk", reason: "deferred" }));
    },
  ),
};

/**
 * The default offline comparison adapters: the exact Rust Spark reference and the
 * deferred LDK seam. The Breez adapter needs an injected SDK-backed deriver, so a
 * composition root adds it with `makeBreezSparkComparisonAdapter`.
 */
export const defaultComparisonAdapters: ReadonlyArray<SparkComparisonAdapter> = [
  rustSparkComparisonAdapter,
  ldkSparkComparisonAdapter,
];
