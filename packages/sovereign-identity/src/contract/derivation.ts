/**
 * IDR-00 frozen derivation profile for the OpenAgents sovereign identity.
 *
 * The historical OpenAgents identity used one BIP-39 mnemonic as a shared root.
 * The root produced two separate child keys: a Nostr identity key and a Spark
 * wallet key. Both branches used the English word list and an EMPTY BIP-39
 * passphrase. This module freezes those exact rules and provides a pure,
 * deterministic reference derivation of the PUBLIC identifiers only.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 *
 * This module derives public identifiers only. It never returns the mnemonic,
 * `nsec`, raw private key, or BIP-39 seed. Private signer material is the job of
 * the later signer boundary (IDR-06), not the frozen contract.
 */
import { secp256k1 } from "@noble/curves/secp256k1";
import { ripemd160 } from "@noble/hashes/ripemd160";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import { bech32 } from "@scure/base";
import { HDKey } from "@scure/bip32";
import { mnemonicToSeedSync, validateMnemonic } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english";

/** Nostr identity path (NIP-06 account zero). */
export const NOSTR_DERIVATION_PATH = "m/44'/1237'/0'/0/0";

/** Rust Spark signer path (Bitcoin BIP-44 account zero). */
export const SPARK_DERIVATION_PATH = "m/44'/0'/0'/0/0";

/** The frozen BIP-39 word list for the shared root. */
export const BIP39_LANGUAGE = "english";

/**
 * The frozen BIP-39 passphrase rule for the shared root. The historical
 * passphrase was empty. A non-empty passphrase produces DIFFERENT keys and must
 * never be added silently.
 */
export const BIP39_PASSPHRASE_MODE = "empty";

/** The empty BIP-39 passphrase literal used by both branches. */
export const EMPTY_BIP39_PASSPHRASE = "";

/** The stable identifier for the shared-root derivation profile. */
export const DERIVATION_PROFILE_ID = "openagents.legacy_unified_nostr_spark.v1";

/**
 * The public identifiers a shared-root mnemonic produces under the frozen
 * profile. These are safe to store in a manifest, a receipt, or a test vector.
 */
export interface SovereignIdentityPublic {
  /** The active derivation profile. */
  readonly derivationProfile: typeof DERIVATION_PROFILE_ID;
  /** The Nostr NIP-06 public key as x-only 32-byte hex. */
  readonly nostrPublicKeyHex: string;
  /** The Nostr NIP-19 `npub`. */
  readonly npub: string;
  /** The Nostr NIP-06 derivation path. */
  readonly nostrDerivationPath: typeof NOSTR_DERIVATION_PATH;
  /** The Rust Spark BIP-44 compressed public key as 33-byte hex. */
  readonly sparkPublicKeyHex: string;
  /**
   * The standard BIP-32 key fingerprint of the Spark public key
   * (`HASH160(pubkey)[0..4]`), as 4-byte hex. This is a provisional public
   * anchor. The EXACT Rust/Breez Spark wallet fingerprint is the job of the
   * IDR-04 reconciliation adapter; the canonical frozen Spark vector is
   * `sparkPublicKeyHex`.
   */
  readonly sparkBip32FingerprintHex: string;
  /** The Rust Spark BIP-44 derivation path. */
  readonly sparkDerivationPath: typeof SPARK_DERIVATION_PATH;
}

/** Normalize BIP-39 whitespace without changing the words. */
export function normalizeMnemonic(value: string): string {
  return value.trim().split(/\s+/).join(" ");
}

/** Validate an English BIP-39 mnemonic checksum and word count. */
export function isValidEnglishMnemonic(mnemonic: string): boolean {
  return validateMnemonic(normalizeMnemonic(mnemonic), wordlist);
}

function encodeBech32(prefix: "npub" | "nsec", bytes: Uint8Array): string {
  return bech32.encode(prefix, bech32.toWords(bytes));
}

/**
 * Derive the PUBLIC sovereign identity from a mnemonic under the frozen
 * empty-passphrase profile. This is the deterministic reference used to produce
 * and verify every IDR-00 test vector.
 *
 * The optional `passphrase` exists ONLY to prove empty-versus-non-empty
 * divergence in tests. Production recovery always uses the empty passphrase.
 */
export function deriveSovereignIdentityPublic(
  mnemonic: string,
  passphrase: string = EMPTY_BIP39_PASSPHRASE,
): SovereignIdentityPublic {
  const normalized = normalizeMnemonic(mnemonic);
  if (!validateMnemonic(normalized, wordlist)) {
    throw new Error("sovereign identity mnemonic is not valid BIP-39 English");
  }
  const seed = mnemonicToSeedSync(normalized, passphrase);
  const master = HDKey.fromMasterSeed(seed);

  const nostrNode = master.derive(NOSTR_DERIVATION_PATH);
  if (!nostrNode.privateKey) throw new Error("failed to derive Nostr private key");
  const nostrPublicKeyXonly = secp256k1.getPublicKey(nostrNode.privateKey, true).slice(1);

  const sparkNode = master.derive(SPARK_DERIVATION_PATH);
  if (!sparkNode.privateKey) throw new Error("failed to derive Spark private key");
  const sparkPublicKeyCompressed = secp256k1.getPublicKey(sparkNode.privateKey, true);
  const sparkFingerprint = ripemd160(sha256(sparkPublicKeyCompressed)).slice(0, 4);

  return {
    derivationProfile: DERIVATION_PROFILE_ID,
    nostrPublicKeyHex: bytesToHex(nostrPublicKeyXonly),
    npub: encodeBech32("npub", nostrPublicKeyXonly),
    nostrDerivationPath: NOSTR_DERIVATION_PATH,
    sparkPublicKeyHex: bytesToHex(sparkPublicKeyCompressed),
    sparkBip32FingerprintHex: bytesToHex(sparkFingerprint),
    sparkDerivationPath: SPARK_DERIVATION_PATH,
  };
}
