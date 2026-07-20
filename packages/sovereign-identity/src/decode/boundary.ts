/**
 * IDR-03 bounded recovered-secret boundary.
 *
 * A decoder that restores a mnemonic never returns the raw phrase to a caller.
 * It returns a `RecoveredSecret`: a bounded holder that exposes the mnemonic
 * ONLY inside a `use` callback, so the material stays in a narrow scope. This
 * matches the `SparkSecretMaterial.withSeedMaterial` pattern from the signer
 * boundary (IDR-01): the callback returns a RESULT, never the secret, so the
 * phrase never escapes.
 *
 * The holder is a hard tripwire against accidental secret logging. The mnemonic
 * lives in a private class field, so it is never an enumerable property. Its
 * `toJSON`, `toString`, and Node inspect hooks all return a fixed redaction
 * marker. `JSON.stringify`, string coercion, and `console.log` of a
 * `RecoveredSecret` therefore never reveal the phrase.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect } from "effect";
import type { HistoricalFormatId } from "../contract/index.ts";
import { deriveSovereignIdentityPublic, type SovereignIdentityPublic } from "../contract/index.ts";

/** The fixed marker a redacted secret renders as. It never contains phrase bytes. */
export const REDACTED_SECRET_MARKER = "[redacted sovereign secret]";

/**
 * A bounded recovered mnemonic. The phrase is reachable ONLY through `use`. The
 * holder carries the public format label and version so a caller can describe
 * the source without the secret.
 */
export class RecoveredSecret {
  readonly #mnemonic: string;

  /** The historical format the mnemonic was decoded from. */
  readonly format: HistoricalFormatId;
  /** The decoded format version string. */
  readonly formatVersion: string;

  constructor(mnemonic: string, format: HistoricalFormatId, formatVersion: string) {
    this.#mnemonic = mnemonic;
    this.format = format;
    this.formatVersion = formatVersion;
  }

  /**
   * Run `use` with the recovered mnemonic inside a bounded scope. The callback
   * returns a derived RESULT (for example the public identity), never the phrase.
   * IDR-04 uses this to derive and compare without the secret leaving the scope.
   */
  use<A, E, R>(use: (mnemonic: string) => Effect.Effect<A, E, R>): Effect.Effect<A, E, R> {
    return Effect.suspend(() => use(this.#mnemonic));
  }

  /** A redacted JSON projection. It never carries the phrase. */
  toJSON(): {
    readonly format: HistoricalFormatId;
    readonly formatVersion: string;
    readonly secret: string;
  } {
    return {
      format: this.format,
      formatVersion: this.formatVersion,
      secret: REDACTED_SECRET_MARKER,
    };
  }

  /** A redacted string coercion. It never carries the phrase. */
  toString(): string {
    return `RecoveredSecret(${this.format}) ${REDACTED_SECRET_MARKER}`;
  }

  /** The Node inspect hook. It renders the redaction marker, never the phrase. */
  [Symbol.for("nodejs.util.inspect.custom")](): string {
    return this.toString();
  }
}

/**
 * Derive the PUBLIC sovereign identity from a bounded secret. The mnemonic never
 * leaves the `use` scope; only the public identifiers return. This is the
 * "public identifiers once derived" surface IDR-03 exposes for IDR-04.
 */
export const derivePublicIdentity = Effect.fn("SovereignIdentity.derivePublicIdentity")(function* (
  secret: RecoveredSecret,
) {
  return yield* secret.use((mnemonic) =>
    Effect.sync((): SovereignIdentityPublic => deriveSovereignIdentityPublic(mnemonic)),
  );
});
