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
 * {@link loadSeed}, and derives from it in memory. It never logs and never
 * returns an `nsec` or a raw private key. The public manifest
 * {@link SeedIdentity} carries public identifiers only and is safe to print,
 * store, and export.
 *
 * AT REST. The seed file is `0600` inside a `0700` directory, and on a machine
 * with an OS keychain it holds ciphertext rather than the phrase: a 32-byte
 * ChaCha20-Poly1305 wrapping key lives in the keychain under service
 * `openagents-cli-identity`, and the file holds only the sealed envelope. That is
 * what stops the threats permissions never did — a backup tool, a sync client, an
 * agent with read access to `$HOME`, or a stolen unlocked disk image.
 *
 * Where there is no keychain — CI, a container, an unattended agent host — the
 * phrase is written as plaintext at `0600`, exactly as before, and every read
 * reports {@link SeedProtection} `plaintext_file` so the surfaces that show an
 * identity can say so. A silent fall back to plaintext would be worse than no
 * encryption at all, because it would read as protection that is not there. The
 * key never goes in the file, so the phrase exists in exactly one place either
 * way.
 *
 * The envelope format, the AEAD, the keychain service, and the account key are
 * the same in `crates/openagents-cli/src/identity.rs`. The two CLIs read one file
 * at one path, so a format only one of them understands makes the other a
 * downgrade attack on it.
 *
 * The spending rail is deliberately absent. Which rail the wallet spends over —
 * self-custodial MDK/LDK, or the deterministic Spark rail Pylon v1.0 used — is
 * an owner decision that is not recorded yet, so this module derives the wallet
 * branch and stops. Receiving identifiers are rail-independent; spending is not.
 */

import { spawnSync } from "node:child_process";
import { createCipheriv, createDecipheriv, randomBytes } from "node:crypto";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
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

/**
 * The seed file: a sealed envelope under the OS keychain, or the mnemonic itself
 * where there is no keychain. Mode `0600` either way.
 */
export const seedPath = (): string => join(seedDirectory(), "seed");

/** Where the atomic rewrite stages, so the phrase is never in two files. */
const seedTempPath = (): string => join(seedDirectory(), "seed.tmp");

/** True when a seed is already stored. Presence only; the bytes stay on disk. */
export const seedPresent = (): boolean => existsSync(seedPath());

// ---------------------------------------------------------------------------
// protection at rest
// ---------------------------------------------------------------------------

/**
 * What is actually protecting the stored seed. Every surface that shows an
 * identity reports this, because the difference between the two is the whole
 * security posture of the machine and a person cannot infer it from the path.
 */
export type SeedProtection = "os_keychain" | "plaintext_file";

/** True when the file on disk is ciphertext rather than the phrase. */
export const seedEncryptedAtRest = (protection: SeedProtection): boolean =>
  protection === "os_keychain";

/**
 * The sentence a person reads. It says what is protecting the seed and, for the
 * plaintext store, what that protection does not cover — a fallback nobody is
 * told about is the same defect as a redaction that reports success and leaves
 * the secret in place.
 */
export const describeSeedProtection = (protection: SeedProtection, path: string): string =>
  protection === "os_keychain"
    ? `Protection: OS keychain. The seed at ${path} is encrypted (${SEED_ENVELOPE_ALG}); ` +
      `the key that opens it is held by the OS keychain under service ${IDENTITY_KEYCHAIN_SERVICE}, ` +
      "never in the file and never in a backup of it."
    : `Protection: NONE. The seed phrase is stored as readable text at ${path} (mode 0600). ` +
      "No OS keychain is available here, so file permissions are the whole protection: " +
      "they stop another local user, and they stop nothing that already runs as you — " +
      "a backup tool, a sync client, or an agent that can read your home directory. " +
      "Treat this file the way you would treat the phrase written on paper.";

// ---------------------------------------------------------------------------
// the sealed envelope
// ---------------------------------------------------------------------------

/**
 * The on-disk format both CLIs read and write. Changing any of these three
 * constants makes one CLI unable to open the other's seed.
 */
export const SEED_ENVELOPE_SCHEMA = "openagents.cli_identity_seed.v1";
export const SEED_ENVELOPE_ALG = "chacha20-poly1305";
/** Bound into the AEAD, so an envelope cannot be replayed under another schema. */
const SEED_ENVELOPE_AAD = Buffer.from(SEED_ENVELOPE_SCHEMA, "utf8");
const SEED_NONCE_BYTES = 12;
const SEED_TAG_BYTES = 16;
const SEED_KEY_BYTES = 32;

interface SeedEnvelope {
  readonly schema: string;
  readonly alg: string;
  /** The 12-byte AEAD nonce, hex. Fresh on every write. */
  readonly nonce: string;
  /** Ciphertext with the 16-byte Poly1305 tag appended, hex. */
  readonly ciphertext: string;
}

/**
 * True when the file at hand is a sealed envelope rather than a bare mnemonic. A
 * BIP-39 phrase can never start with `{`, so the two formats cannot be confused
 * and an old plaintext seed is still recognised for migration.
 */
const looksSealed = (text: string): boolean => text.trimStart().startsWith("{");

const sealPhrase = (phrase: string, key: Uint8Array): string => {
  const nonce = randomBytes(SEED_NONCE_BYTES);
  const cipher = createCipheriv(SEED_ENVELOPE_ALG, key, nonce, {
    authTagLength: SEED_TAG_BYTES,
  });
  cipher.setAAD(SEED_ENVELOPE_AAD, { plaintextLength: Buffer.byteLength(phrase, "utf8") });
  const body = Buffer.concat([cipher.update(phrase, "utf8"), cipher.final()]);
  const envelope: SeedEnvelope = {
    schema: SEED_ENVELOPE_SCHEMA,
    alg: SEED_ENVELOPE_ALG,
    nonce: nonce.toString("hex"),
    ciphertext: Buffer.concat([body, cipher.getAuthTag()]).toString("hex"),
  };
  return JSON.stringify(envelope);
};

const undecryptable = (path: string): Error =>
  new Error(
    `The seed at ${path} is encrypted and the key in the OS keychain does not open it. ` +
      "Restore the seed phrase with openagents identity import.",
  );

const openEnvelope = (text: string, key: Uint8Array, path: string): string => {
  let envelope: SeedEnvelope;
  try {
    envelope = JSON.parse(text.trim()) as SeedEnvelope;
  } catch {
    throw undecryptable(path);
  }
  if (envelope.schema !== SEED_ENVELOPE_SCHEMA || envelope.alg !== SEED_ENVELOPE_ALG) {
    throw undecryptable(path);
  }
  const nonce = Buffer.from(envelope.nonce, "hex");
  const sealed = Buffer.from(envelope.ciphertext, "hex");
  if (nonce.length !== SEED_NONCE_BYTES || sealed.length <= SEED_TAG_BYTES)
    throw undecryptable(path);
  const body = sealed.subarray(0, sealed.length - SEED_TAG_BYTES);
  const tag = sealed.subarray(sealed.length - SEED_TAG_BYTES);
  try {
    const decipher = createDecipheriv(SEED_ENVELOPE_ALG, key, nonce, {
      authTagLength: SEED_TAG_BYTES,
    });
    decipher.setAAD(SEED_ENVELOPE_AAD, { plaintextLength: body.length });
    decipher.setAuthTag(tag);
    return normalize(Buffer.concat([decipher.update(body), decipher.final()]).toString("utf8"));
  } catch {
    throw undecryptable(path);
  }
};

// ---------------------------------------------------------------------------
// where the wrapping key lives
// ---------------------------------------------------------------------------

/**
 * The service name the OS keychain files the identity wrapping key under. It is
 * deliberately not `openagents-cli` (account tokens) or `openagents-cli-computer`
 * (machine tokens), so no two of the three can overwrite each other. The Rust CLI
 * uses the same one.
 */
export const IDENTITY_KEYCHAIN_SERVICE = "openagents-cli-identity";

/**
 * Set this to opt out of the keychain and store the phrase as plaintext at
 * `0600`. It exists because a keychain that prompts is worse than no keychain on
 * an unattended host, and because the choice should be stateable rather than
 * discovered. It is never selected implicitly.
 */
export const PLAINTEXT_ENV = "OPENAGENTS_IDENTITY_PLAINTEXT";

/**
 * Where the 32-byte wrapping key lives. One implementation talks to the OS
 * keychain; the others exist so a test exercises the real seal, open, and
 * migration paths without touching the developer's own keychain.
 */
export interface SeedKeyStore {
  /** False when this machine has no keychain, which selects the plaintext file. */
  readonly available: () => boolean;
  /**
   * `undefined` means the store answered and holds no key for this identity
   * directory. A throw must never be read as "no key": minting a second one
   * would orphan the sealed seed.
   */
  readonly get: () => Uint8Array | undefined;
  /**
   * Store the key and prove it by reading it back. A store that reports success
   * without keeping the value would seal a seed nobody can open.
   */
  readonly put: (key: Uint8Array) => void;
  /** Best-effort removal, so a forgotten identity leaves no key behind. */
  readonly delete: () => void;
}

interface KeychainCommand {
  readonly command: string;
  readonly args: ReadonlyArray<string>;
  readonly input?: string;
}

/**
 * The command that reads, writes, or clears the wrapping key. Exported so a test
 * can assert the shape without a keychain, and so the two CLIs can be compared
 * side by side.
 */
export const identityKeychainCommandFor = (
  platform: NodeJS.Platform,
  operation: "get" | "put" | "delete",
  account: string,
  key?: string,
): KeychainCommand | undefined => {
  if (platform === "darwin") {
    if (operation === "get") {
      return {
        command: "security",
        args: ["find-generic-password", "-a", account, "-s", IDENTITY_KEYCHAIN_SERVICE, "-w"],
      };
    }
    if (operation === "put" && key !== undefined) {
      // `security` reads the value from argv, so the wrapping key is briefly
      // visible to `ps`. The seed phrase never is: it goes to the file sealed,
      // and the key alone opens nothing without that file.
      return {
        command: "security",
        args: [
          "add-generic-password",
          "-U",
          "-a",
          account,
          "-s",
          IDENTITY_KEYCHAIN_SERVICE,
          "-w",
          key,
        ],
      };
    }
    if (operation === "delete") {
      return {
        command: "security",
        args: ["delete-generic-password", "-a", account, "-s", IDENTITY_KEYCHAIN_SERVICE],
      };
    }
    return undefined;
  }
  if (platform === "linux") {
    if (operation === "get") {
      return {
        command: "secret-tool",
        args: ["lookup", "service", IDENTITY_KEYCHAIN_SERVICE, "account", account],
      };
    }
    if (operation === "put" && key !== undefined) {
      return {
        command: "secret-tool",
        args: [
          "store",
          "--label=OpenAgents identity",
          "service",
          IDENTITY_KEYCHAIN_SERVICE,
          "account",
          account,
        ],
        input: key,
      };
    }
    if (operation === "delete") {
      return {
        command: "secret-tool",
        args: ["clear", "service", IDENTITY_KEYCHAIN_SERVICE, "account", account],
      };
    }
  }
  return undefined;
};

const HEX_KEY = /^[0-9a-f]{64}$/;

/**
 * The OS keychain: `security` on macOS, `secret-tool` on Linux.
 *
 * The record is keyed by the identity directory, exactly as the credential store
 * keys tokens by origin, so a second identity directory gets a second key and a
 * test with a temporary directory can never reach the developer's own.
 */
export const osKeychainKeyStore = (account: string): SeedKeyStore => {
  const run = (operation: "get" | "put" | "delete", key?: string) => {
    const command = identityKeychainCommandFor(process.platform, operation, account, key);
    if (command === undefined) return undefined;
    const result = spawnSync(command.command, [...command.args], {
      input: command.input,
      encoding: "utf8",
      stdio: ["pipe", "pipe", "ignore"],
    });
    // A `security` or `secret-tool` that will not start is not an empty store:
    // this platform has no keychain, and that is a different answer.
    if (result.error !== undefined) return undefined;
    return result;
  };

  return {
    available: () => run("get") !== undefined,
    get: () => {
      const result = run("get");
      if (result === undefined) return undefined;
      if (result.status !== 0) return undefined;
      const value = (result.stdout ?? "").trim();
      if (value.length === 0) return undefined;
      if (!HEX_KEY.test(value)) {
        // Never regenerate here. A record that is not a wrapping key means
        // something else wrote it, and overwriting it would make the sealed seed
        // permanently unopenable.
        throw new Error(
          `The record under service ${IDENTITY_KEYCHAIN_SERVICE} is not an identity wrapping key.`,
        );
      }
      return Uint8Array.from(Buffer.from(value, "hex"));
    },
    put: (key) => {
      const encoded = Buffer.from(key).toString("hex");
      const result = run("put", encoded);
      if (result === undefined || result.status !== 0) {
        throw new Error("The OS keychain refused to store the identity wrapping key.");
      }
      const readBack = run("get");
      if (readBack === undefined || (readBack.stdout ?? "").trim() !== encoded) {
        throw new Error("The OS keychain did not return the key that was just written.");
      }
    },
    delete: () => {
      run("delete");
    },
  };
};

/**
 * A machine with no keychain: CI, a container, an unattended agent host. This
 * selects the plaintext store and the warning that goes with it.
 */
export const noKeyStore: SeedKeyStore = {
  available: () => false,
  get: () => undefined,
  put: () => {
    throw new Error("This machine has no OS keychain, so there is nowhere to hold a key.");
  },
  delete: () => {},
};

/** True when the environment asks for the plaintext store. */
const plaintextRequested = (): boolean => {
  const value = (process.env[PLAINTEXT_ENV] ?? "").trim().toLowerCase();
  return !(value.length === 0 || value === "0" || value === "false" || value === "no");
};

/**
 * The production key store: the OS keychain, unless {@link PLAINTEXT_ENV} says
 * otherwise. Computed per call because the identity directory is an environment
 * override and may change between calls in a test.
 */
export const defaultSeedKeyStore = (): SeedKeyStore =>
  plaintextRequested() ? noKeyStore : osKeychainKeyStore(seedDirectory());

// ---------------------------------------------------------------------------
// reading and writing the seed
// ---------------------------------------------------------------------------

/** A seed read back off disk, and what was protecting it there. */
export interface StoredSeed {
  /** The mnemonic. Secret. */
  readonly phrase: string;
  readonly protection: SeedProtection;
}

/**
 * What a write would use on this machine right now: the keychain when one
 * answers, the plaintext file when none exists. A keychain that answers with a
 * record that is not a wrapping key throws from {@link SeedKeyStore.get} rather
 * than being downgraded to plaintext here.
 */
export const seedProtectionAvailable = (
  keyStore: SeedKeyStore = defaultSeedKeyStore(),
): SeedProtection => (keyStore.available() ? "os_keychain" : "plaintext_file");

/**
 * What is protecting the seed that is on disk now, without opening it.
 * `undefined` when nothing is stored.
 */
export const seedProtectionOnDisk = (): SeedProtection | undefined => {
  const path = seedPath();
  if (!existsSync(path)) return undefined;
  const text = readFileSync(path, "utf8");
  if (text.trim().length === 0) return undefined;
  return looksSealed(text) ? "os_keychain" : "plaintext_file";
};

/**
 * Read the stored seed and report what was protecting it. It is the only
 * function in this module that returns secret material.
 */
export const loadSeed = (
  keyStore: SeedKeyStore = defaultSeedKeyStore(),
): StoredSeed | undefined => {
  const path = seedPath();
  if (!existsSync(path)) return undefined;
  const text = readFileSync(path, "utf8");
  if (text.trim().length === 0) return undefined;
  if (!looksSealed(text)) return { phrase: normalize(text), protection: "plaintext_file" };
  // Sealed. A keychain that cannot be read is never reported as "no seed": that
  // reads as an identity that vanished, and the next command would offer to make
  // a new one.
  const key = keyStore.get();
  if (key === undefined) {
    throw new Error(
      `The seed at ${path} is encrypted, and the OS keychain holds no key that opens it. ` +
        "The key does not travel with the file and is not in any backup of it. " +
        "Restore the seed phrase with openagents identity import.",
    );
  }
  return { phrase: openEnvelope(text, key, path), protection: "os_keychain" };
};

const writeAtomic = (body: string): string => {
  const directory = seedDirectory();
  mkdirSync(directory, { recursive: true, mode: 0o700 });
  chmodSync(directory, 0o700);
  const path = seedPath();
  const temporary = seedTempPath();
  rmSync(temporary, { force: true });
  try {
    writeFileSync(temporary, body, { mode: 0o600 });
    chmodSync(temporary, 0o600);
    renameSync(temporary, path);
  } catch (cause) {
    rmSync(temporary, { force: true });
    throw cause;
  }
  chmodSync(path, 0o600);
  return path;
};

/**
 * Write the mnemonic under the best protection this machine has, `0600` inside a
 * `0700` directory, after validating it. The validation is not politeness: an
 * unwritable-back phrase stored here would be an identity that cannot be
 * recovered from its own backup.
 *
 * The write is atomic — staged in a sibling file and renamed over the target —
 * so the phrase is never in two files at once and a crash mid-write leaves the
 * previous seed intact rather than half of the new one.
 */
export const storeSeedPhrase = (
  phrase: string,
  keyStore: SeedKeyStore = defaultSeedKeyStore(),
): { readonly path: string; readonly protection: SeedProtection } => {
  const normalized = normalize(phrase);
  if (!validateMnemonic(normalized, wordlist)) {
    throw new Error("The seed phrase is not a valid English BIP-39 mnemonic.");
  }
  const protection = seedProtectionAvailable(keyStore);
  if (protection === "plaintext_file") {
    return { path: writeAtomic(`${normalized}\n`), protection };
  }
  let key = keyStore.get();
  if (key === undefined) {
    // Prove the keychain kept it before anything is sealed under it. Sealing
    // first would produce a file no key opens.
    const fresh = Uint8Array.from(randomBytes(SEED_KEY_BYTES));
    keyStore.put(fresh);
    key = fresh;
  }
  return { path: writeAtomic(`${sealPhrase(normalized, key)}\n`), protection };
};

/**
 * Move a plaintext seed under the OS keychain, and report what is protecting it
 * afterwards. `undefined` when nothing is stored.
 *
 * The rewrite lands on the same path by rename, so there is never a moment with
 * the phrase in two files, and the plaintext is gone the instant the sealed
 * envelope arrives. On a machine with no keychain this changes nothing and
 * reports `plaintext_file`, which is what the caller then has to say out loud.
 */
export const protectSeed = (
  keyStore: SeedKeyStore = defaultSeedKeyStore(),
): SeedProtection | undefined => {
  const onDisk = seedProtectionOnDisk();
  if (onDisk === undefined) return undefined;
  if (onDisk === "os_keychain") return "os_keychain";
  if (seedProtectionAvailable(keyStore) !== "os_keychain") return "plaintext_file";
  const stored = loadSeed(keyStore);
  if (stored === undefined) return undefined;
  return storeSeedPhrase(stored.phrase, keyStore).protection;
};

/**
 * Remove the stored seed, and the wrapping key with it. Idempotent, and it
 * deletes nothing else. Leaving the key behind would leave a keychain record for
 * an identity that no longer exists.
 */
export const forgetSeedPhrase = (keyStore: SeedKeyStore = defaultSeedKeyStore()): boolean => {
  const path = seedPath();
  rmSync(seedTempPath(), { force: true });
  if (!existsSync(path)) {
    keyStore.delete();
    return false;
  }
  rmSync(path);
  keyStore.delete();
  return true;
};
