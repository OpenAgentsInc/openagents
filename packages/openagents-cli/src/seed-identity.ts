/**
 * One seed, one identity, one wallet.
 *
 * The CLI holds a single BIP-39 mnemonic and derives everything else from it:
 * the Nostr identity that signs, and the wallet branch that receives. There is
 * no second secret to back up and no way for the two to drift apart, because
 * neither is stored — only the seed is, and both are recomputed from it.
 *
 * The derivation is the frozen OpenAgents profile, not a new one. The paths,
 * the English word list, and the empty BIP-39 passphrase come from
 * `packages/sovereign-identity/src/contract/derivation.ts`, which froze what
 * Pylon shipped (`docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`).
 * `test/seed-identity.test.ts` re-derives that package's public vectors through
 * this module, so a refactor here that changes anyone's `npub` fails the build
 * rather than silently reissuing every identity.
 *
 * Why the derivation lives here rather than importing that package: the CLI
 * publishes as a plain `tsc` build with no bundler, so every runtime import has
 * to resolve from npm for an installed user. `@openagentsinc/sovereign-identity`
 * ships TypeScript sources and depends on a Git-tarball `nostr-effect`, so it is
 * a test-time cross-check here, not a runtime dependency. The crypto underneath
 * is the same pinned stack both sides use: `@noble/curves` 1.8.1,
 * `@noble/hashes` 1.7.1, `@scure/bip32` 1.6.2, `@scure/bip39` 1.5.4.
 *
 * SECRETS. This module returns the mnemonic from exactly one function,
 * {@link readSeedPhrase}, and derives from it in memory. It never logs, never
 * returns an `nsec` or a raw private key, and writes the seed file `0600` inside
 * a `0700` directory. The public manifest {@link SeedIdentity} carries public
 * identifiers only and is safe to print, store, and export.
 *
 * The spending rail is deliberately absent. Which rail the wallet spends over —
 * self-custodial MDK/LDK, or the deterministic Spark rail Pylon v1.0 used — is
 * an owner decision that is not recorded yet, so this module derives the wallet
 * branch and stops. Receiving identifiers are rail-independent; spending is not.
 */

import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import { secp256k1 } from "@noble/curves/secp256k1";
import { ripemd160 } from "@noble/hashes/ripemd160";
import { sha256 } from "@noble/hashes/sha256";
import { bytesToHex } from "@noble/hashes/utils";
import { bech32, createBase58check } from "@scure/base";
import { HDKey } from "@scure/bip32";
import { generateMnemonic, mnemonicToSeedSync, validateMnemonic } from "@scure/bip39";
import { wordlist } from "@scure/bip39/wordlists/english";

/** The frozen shared-root profile both the CLI and Pylon derive under. */
export const DERIVATION_PROFILE_ID = "openagents.legacy_unified_nostr_spark.v1";

/** Nostr identity path: NIP-06 account zero. */
export const NOSTR_DERIVATION_PATH = "m/44'/1237'/0'/0/0";

/** Wallet path: BIP-44 Bitcoin account zero, first external key. */
export const WALLET_DERIVATION_PATH = "m/44'/0'/0'/0/0";

/**
 * The frozen BIP-39 passphrase. It is empty, and a non-empty one produces a
 * different identity, so it is a constant here rather than an option.
 */
const BIP39_PASSPHRASE = "";

const base58check = createBase58check(sha256);

/** Mainnet pay-to-public-key-hash version byte, the standard BIP-44 pairing. */
const P2PKH_VERSION = 0x00;

/**
 * The public half of one seed: what a manifest, a receipt, or a tip request may
 * carry. Nothing here can spend, sign, or reconstruct the seed.
 */
export interface SeedIdentity {
  /** The frozen derivation profile these identifiers were produced under. */
  readonly profile: typeof DERIVATION_PROFILE_ID;
  /** The NIP-19 `npub`, and the one cross-surface name for this identity. */
  readonly npub: string;
  /** The x-only 32-byte Nostr public key as hex. */
  readonly nostrPublicKeyHex: string;
  readonly nostrDerivationPath: typeof NOSTR_DERIVATION_PATH;
  /** The compressed 33-byte wallet public key as hex. */
  readonly walletPublicKeyHex: string;
  /** The BIP-32 key fingerprint, `HASH160(pubkey)[0..4]`, as hex. */
  readonly walletFingerprintHex: string;
  /** The mainnet P2PKH receive address for the wallet path. */
  readonly walletAddress: string;
  readonly walletDerivationPath: typeof WALLET_DERIVATION_PATH;
}

/** Trim and collapse whitespace without changing the words themselves. */
const normalize = (phrase: string): string => phrase.trim().split(/\s+/).join(" ");

/** True when the phrase is a valid English BIP-39 mnemonic with a good checksum. */
export const isValidSeedPhrase = (phrase: string): boolean =>
  validateMnemonic(normalize(phrase), wordlist);

/** Generate a fresh mnemonic. 12 words is 128 bits of entropy, 24 words is 256. */
export const generateSeedPhrase = (words: 12 | 24 = 12): string =>
  generateMnemonic(wordlist, words === 24 ? 256 : 128);

/**
 * Derive the public identity and wallet from one mnemonic.
 *
 * Deterministic and side-effect free: the same phrase always yields the same
 * `npub` and the same wallet address, on every machine and every version.
 * Throws when the phrase is not valid BIP-39 English, because deriving from a
 * mistyped phrase would hand back a plausible identity nobody can recover.
 */
export const deriveSeedIdentity = (phrase: string): SeedIdentity => {
  const normalized = normalize(phrase);
  if (!validateMnemonic(normalized, wordlist)) {
    throw new Error("The seed phrase is not a valid English BIP-39 mnemonic.");
  }
  const master = HDKey.fromMasterSeed(mnemonicToSeedSync(normalized, BIP39_PASSPHRASE));
  try {
    const nostrNode = master.derive(NOSTR_DERIVATION_PATH);
    if (nostrNode.privateKey === null) throw new Error("The Nostr key could not be derived.");
    const nostrPublicKey = secp256k1.getPublicKey(nostrNode.privateKey, true).slice(1);

    const walletNode = master.derive(WALLET_DERIVATION_PATH);
    if (walletNode.privateKey === null) throw new Error("The wallet key could not be derived.");
    const walletPublicKey = secp256k1.getPublicKey(walletNode.privateKey, true);
    const walletHash160 = ripemd160(sha256(walletPublicKey));
    const addressPayload = new Uint8Array(21);
    addressPayload[0] = P2PKH_VERSION;
    addressPayload.set(walletHash160, 1);

    nostrNode.wipePrivateData();
    walletNode.wipePrivateData();

    return {
      profile: DERIVATION_PROFILE_ID,
      npub: bech32.encode("npub", bech32.toWords(nostrPublicKey)),
      nostrPublicKeyHex: bytesToHex(nostrPublicKey),
      nostrDerivationPath: NOSTR_DERIVATION_PATH,
      walletPublicKeyHex: bytesToHex(walletPublicKey),
      walletFingerprintHex: bytesToHex(walletHash160.slice(0, 4)),
      walletAddress: base58check.encode(addressPayload),
      walletDerivationPath: WALLET_DERIVATION_PATH,
    };
  } finally {
    master.wipePrivateData();
  }
};

/**
 * Where the seed lives. `OPENAGENTS_IDENTITY_DIR` moves it, which is how tests
 * get an isolated identity without touching the developer's own.
 */
export const seedDirectory = (): string => {
  const override = process.env["OPENAGENTS_IDENTITY_DIR"];
  return override !== undefined && override.trim().length > 0
    ? override
    : join(homedir(), ".openagents", "identity");
};

/** The seed file itself: one line, the mnemonic, mode `0600`. */
export const seedPath = (): string => join(seedDirectory(), "seed");

/** True when a seed is already stored. Presence only; the bytes stay on disk. */
export const seedPresent = (): boolean => existsSync(seedPath());

/**
 * Read the stored mnemonic. This is the only function that returns secret
 * material, and every caller of it either derives from it or hands it to the
 * reader who asked for a backup. Returns `undefined` when no seed is stored.
 */
export const readSeedPhrase = (): string | undefined => {
  const path = seedPath();
  if (!existsSync(path)) return undefined;
  const phrase = normalize(readFileSync(path, "utf8"));
  return phrase.length === 0 ? undefined : phrase;
};

/**
 * Write the mnemonic, `0600` inside a `0700` directory, after validating it.
 * The validation is not politeness: an unwritable-back phrase stored here would
 * be an identity that cannot be recovered from its own backup.
 */
export const writeSeedPhrase = (phrase: string): string => {
  const normalized = normalize(phrase);
  if (!validateMnemonic(normalized, wordlist)) {
    throw new Error("The seed phrase is not a valid English BIP-39 mnemonic.");
  }
  const directory = seedDirectory();
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  const path = seedPath();
  writeFileSync(path, `${normalized}\n`, { mode: 0o600 });
  chmodSync(path, 0o600);
  return path;
};

/** Remove the stored seed. Idempotent, and it deletes nothing else. */
export const forgetSeedPhrase = (): boolean => {
  const path = seedPath();
  if (!existsSync(path)) return false;
  rmSync(path);
  return true;
};
