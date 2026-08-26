/**
 * The vectors that make the CLI's identity permanent.
 *
 * A derivation bug is not a normal regression. Nobody sees a stack trace: the
 * CLI keeps working, derives a different `npub` and a different address from the
 * same phrase, and every identity, every signature anyone verified, and every
 * payment sent to the old address belongs to a stranger. The only defence is a
 * fixed phrase with fixed answers, asserted literally, so a refactor that
 * changes anyone's identity fails here instead of in the field.
 *
 * The literals below are not hand-typed. They match, field for field, the frozen
 * IDR-00 vectors in `packages/sovereign-identity/src/contract/vectors.ts`, and
 * the last test re-derives them through that package to prove the two surfaces
 * still agree — one seed, one identity, wherever it is derived.
 *
 * `walletAddress` has an independent witness: `1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA`
 * is the published first BIP-44 address of this mnemonic, quoted in BIP-39
 * tooling everywhere. If this file ever disagrees with it, this file is wrong.
 */

import {
  chmodSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  deriveSovereignIdentityPublic,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
} from "@openagentsinc/sovereign-identity/contract";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
  describeSeedProtection,
  deriveSeedIdentity,
  forgetSeedPhrase,
  generateSeedPhrase,
  identityKeychainCommandFor,
  IDENTITY_KEYCHAIN_SERVICE,
  inMemoryKeyStore,
  isValidSeedPhrase,
  loadSeed,
  noKeyStore,
  protectSeed,
  readSeedPhrase,
  seedEncryptedAtRest,
  seedPath,
  seedPresent,
  seedProtectionOnDisk,
  storeSeedPhrase,
  writeSeedPhrase,
  type SeedKeyStore,
} from "../src/seed-identity.js";

/**
 * The canonical published BIP-39 test phrase. It is not a secret and never was;
 * it exists so a deterministic answer can be committed.
 */
const TEST_PHRASE =
  "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

/** What `TEST_PHRASE` must always derive. Changing any line reissues identities. */
const FROZEN = {
  profile: "openagents.legacy_unified_nostr_spark.v1",
  npub: "npub1az708q3kd9zy6z6f44zav5ygvdwelkzspf6mtusttx47lft2z38sghk0w7",
  nostrPublicKeyHex: "e8bcf3823669444d0b49ad45d65088635d9fd8500a75b5f20b59abefa56a144f",
  nostrDerivationPath: "m/44'/1237'/0'/0/0",
  walletPublicKeyHex: "03aaeb52dd7494c361049de67cc680e83ebcbbbdbeb13637d92cd845f70308af5e",
  walletFingerprintHex: "d986ed01",
  walletAddress: "1LqBGSKuX5yYUonjxT5qGfpUsXKYYWeabA",
  walletDerivationPath: "m/44'/0'/0'/0/0",
} as const;

const isolatedIdentityDirectory = () => {
  const directory = mkdtempSync(join(tmpdir(), "openagents-identity-"));
  process.env["OPENAGENTS_IDENTITY_DIR"] = directory;
  return directory;
};

afterEach(() => {
  delete process.env["OPENAGENTS_IDENTITY_DIR"];
});

describe("seed derivation", () => {
  it("derives the frozen identity and wallet from the published test phrase", () => {
    expect(deriveSeedIdentity(TEST_PHRASE)).toEqual(FROZEN);
  });

  it("is insensitive to surrounding whitespace but not to the words", () => {
    expect(deriveSeedIdentity(`  ${TEST_PHRASE.replace(/ /g, "  ")}\n`).npub).toBe(FROZEN.npub);
  });

  it("refuses a phrase whose checksum does not hold", () => {
    const wrongChecksum = TEST_PHRASE.replace(/about$/, "abandon");
    expect(isValidSeedPhrase(wrongChecksum)).toBe(false);
    expect(() => deriveSeedIdentity(wrongChecksum)).toThrow(/valid English BIP-39/);
  });

  it("gives every generated phrase its own identity and wallet", () => {
    const first = deriveSeedIdentity(generateSeedPhrase(12));
    const second = deriveSeedIdentity(generateSeedPhrase(24));
    expect(first.npub).not.toBe(second.npub);
    expect(first.walletAddress).not.toBe(second.walletAddress);
    expect(generateSeedPhrase(24).split(" ")).toHaveLength(24);
  });

  it("agrees with the frozen sovereign-identity contract, field for field", () => {
    const cli = deriveSeedIdentity(TEST_PHRASE);
    const frozen = deriveSovereignIdentityPublic(TEST_PHRASE);
    expect(cli.npub).toBe(frozen.npub);
    expect(cli.nostrPublicKeyHex).toBe(frozen.nostrPublicKeyHex);
    expect(cli.nostrDerivationPath).toBe(frozen.nostrDerivationPath);
    expect(cli.walletPublicKeyHex).toBe(frozen.sparkPublicKeyHex);
    expect(cli.walletFingerprintHex).toBe(frozen.sparkBip32FingerprintHex);
    expect(cli.walletDerivationPath).toBe(frozen.sparkDerivationPath);
    expect(cli.profile).toBe(frozen.derivationProfile);
    // And the package's own committed vector, so neither side can drift alone.
    expect(cli.npub).toBe(PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub);
  });
});

describe("seed storage", () => {
  /**
   * A wrapping key that lives for the length of one test. Nothing here reaches
   * the developer's own OS keychain, and nothing depends on the machine running
   * the tests having one.
   */
  let keys: SeedKeyStore;

  beforeEach(() => {
    keys = inMemoryKeyStore();
  });

  it("writes the phrase 0600 and reads it back unchanged", () => {
    isolatedIdentityDirectory();
    expect(seedPresent()).toBe(false);
    const path = writeSeedPhrase(`  ${TEST_PHRASE}  `, keys);
    expect(path).toBe(seedPath());
    expect(seedPresent()).toBe(true);
    expect(statSync(path).mode & 0o777).toBe(0o600);
    expect(readSeedPhrase(keys)).toBe(TEST_PHRASE);
    expect(deriveSeedIdentity(readSeedPhrase(keys) ?? "").npub).toBe(FROZEN.npub);
  });

  it("restores 0600 when the file on disk was left readable", () => {
    isolatedIdentityDirectory();
    const path = writeSeedPhrase(TEST_PHRASE, keys);
    chmodSync(path, 0o644);
    writeSeedPhrase(TEST_PHRASE, keys);
    expect(statSync(path).mode & 0o777).toBe(0o600);
  });

  it("writes nothing when the phrase is not a valid mnemonic", () => {
    isolatedIdentityDirectory();
    expect(() => writeSeedPhrase("not a seed phrase at all", keys)).toThrow(/valid English BIP-39/);
    expect(seedPresent()).toBe(false);
  });

  it("reports no seed for an absent or empty file, and forgets idempotently", () => {
    const directory = isolatedIdentityDirectory();
    expect(readSeedPhrase(keys)).toBeUndefined();
    expect(forgetSeedPhrase(keys)).toBe(false);
    writeFileSync(join(directory, "seed"), "   \n", { mode: 0o600 });
    expect(readSeedPhrase(keys)).toBeUndefined();
    writeSeedPhrase(TEST_PHRASE, keys);
    expect(keys.get()).toBeDefined();
    expect(forgetSeedPhrase(keys)).toBe(true);
    expect(seedPresent()).toBe(false);
    // Forget takes the wrapping key with it. A key left behind is a keychain
    // record for an identity that no longer exists.
    expect(keys.get()).toBeUndefined();
    expect(forgetSeedPhrase(keys)).toBe(false);
  });

  it("keeps the seed out of the derived identity", () => {
    isolatedIdentityDirectory();
    writeSeedPhrase(TEST_PHRASE, keys);
    const identity = deriveSeedIdentity(TEST_PHRASE);
    expect(JSON.stringify(identity)).not.toContain("abandon");
    expect(JSON.stringify(identity)).not.toContain("nsec");
  });
});

describe("seed protection at rest", () => {
  /**
   * The claim under test is not "encryption was called". It is that the bytes a
   * backup tool, a sync client, or an agent reading `$HOME` would carry away are
   * not the phrase, and not any word of it.
   */
  it("leaves no word of the phrase in the seed file", () => {
    const directory = isolatedIdentityDirectory();
    const keys = inMemoryKeyStore();
    const { path, protection } = storeSeedPhrase(TEST_PHRASE, keys);

    const onDisk = readFileSync(path, "utf8");
    expect(onDisk).not.toContain(TEST_PHRASE);
    expect(onDisk).not.toContain("abandon");
    expect(onDisk).not.toContain("about");

    // And it is the sealed envelope, not some other encoding of the same words:
    // a base64 or hex of the phrase would pass the checks above.
    expect(onDisk).toContain("chacha20-poly1305");
    expect(onDisk).toContain("openagents.cli_identity_seed.v1");
    expect(protection).toBe("os_keychain");
    expect(seedEncryptedAtRest(protection)).toBe(true);
    expect(seedProtectionOnDisk()).toBe("os_keychain");

    // The wrapping key is not in the identity directory. If it were, the file
    // and the key would travel together and the encryption would be theatre.
    expect(readdirSync(directory)).toEqual(["seed"]);
    expect(readSeedPhrase(keys)).toBe(TEST_PHRASE);
  });

  it("uses a fresh nonce for every seal", () => {
    isolatedIdentityDirectory();
    const keys = inMemoryKeyStore();
    const { path } = storeSeedPhrase(TEST_PHRASE, keys);
    const first = readFileSync(path, "utf8");
    storeSeedPhrase(TEST_PHRASE, keys);
    const second = readFileSync(path, "utf8");

    expect(second).not.toBe(first);
    expect(readSeedPhrase(keys)).toBe(TEST_PHRASE);
  });

  /**
   * A sealed seed whose key is gone must say so. Reporting "no seed" would read
   * as an identity that vanished, and the next command would offer a new one.
   */
  it("treats a sealed seed without its key as an error, not an absence", () => {
    isolatedIdentityDirectory();
    const keys = inMemoryKeyStore();
    storeSeedPhrase(TEST_PHRASE, keys);
    keys.delete();

    expect(seedPresent()).toBe(true);
    expect(() => readSeedPhrase(keys)).toThrow(/encrypted/);
    try {
      readSeedPhrase(keys);
    } catch (cause) {
      expect(String(cause)).not.toContain("abandon");
    }
  });

  it("refuses a key that does not open the envelope rather than returning rubbish", () => {
    isolatedIdentityDirectory();
    const keys = inMemoryKeyStore();
    storeSeedPhrase(TEST_PHRASE, keys);
    keys.put(new Uint8Array(32).fill(7));

    expect(() => readSeedPhrase(keys)).toThrow(/does not open it/);
  });

  /**
   * The headless case, stated rather than assumed: with no keychain the phrase
   * is on disk as text, and the module says exactly that so the CLI can print
   * it.
   */
  it("says the seed is plaintext when there is no keychain", () => {
    isolatedIdentityDirectory();
    const { path, protection } = storeSeedPhrase(TEST_PHRASE, noKeyStore);

    expect(protection).toBe("plaintext_file");
    expect(seedEncryptedAtRest(protection)).toBe(false);
    expect(readFileSync(path, "utf8")).toContain(TEST_PHRASE);
    expect(readSeedPhrase(noKeyStore)).toBe(TEST_PHRASE);
    expect(seedProtectionOnDisk()).toBe("plaintext_file");

    // The sentence a person sees must name the file and say what is not covered.
    const described = describeSeedProtection(protection, path);
    expect(described).toContain(path);
    expect(described).toContain("readable text");
    expect(described).toContain("backup tool");
    expect(described).not.toContain(TEST_PHRASE);
  });

  it("names the keychain service in the sentence for the encrypted store", () => {
    const described = describeSeedProtection("os_keychain", "/tmp/seed");
    expect(described).toContain(IDENTITY_KEYCHAIN_SERVICE);
    expect(described).toContain("chacha20-poly1305");
  });

  /**
   * The migration. Start from a seed file written by the CLI that could not
   * encrypt one, and prove both halves: the identity is unchanged, and the
   * plaintext is gone.
   */
  it("migrates an existing plaintext seed and leaves no plaintext behind", () => {
    const directory = isolatedIdentityDirectory();
    const path = join(directory, "seed");

    // Exactly what the previous CLI wrote: the phrase, one line, mode 0600.
    writeFileSync(path, `${TEST_PHRASE}\n`, { mode: 0o600 });
    const before = deriveSeedIdentity(TEST_PHRASE);
    expect(seedProtectionOnDisk()).toBe("plaintext_file");

    const keys = inMemoryKeyStore();
    expect(protectSeed(keys)).toBe("os_keychain");

    // The identity did not move.
    const after = deriveSeedIdentity(readSeedPhrase(keys) ?? "");
    expect(after).toEqual(before);
    expect(after.npub).toBe(FROZEN.npub);

    // The plaintext is gone, from that file and from every other file the
    // migration could have left in the directory.
    expect(readFileSync(path, "utf8")).not.toContain("abandon");
    for (const entry of readdirSync(directory)) {
      expect(readFileSync(join(directory, entry), "utf8")).not.toContain("abandon");
    }
    expect(statSync(path).mode & 0o777).toBe(0o600);

    // Migrating twice is not a second identity, and not a second file.
    expect(protectSeed(keys)).toBe("os_keychain");
    expect(readdirSync(directory)).toEqual(["seed"]);
    expect(loadSeed(keys)?.phrase).toBe(TEST_PHRASE);
  });

  it("reports plaintext rather than faking a migration on a headless machine", () => {
    const directory = isolatedIdentityDirectory();
    const path = join(directory, "seed");
    writeFileSync(path, `${TEST_PHRASE}\n`, { mode: 0o600 });

    expect(protectSeed(noKeyStore)).toBe("plaintext_file");
    expect(readFileSync(path, "utf8")).toContain(TEST_PHRASE);
    expect(readSeedPhrase(noKeyStore)).toBe(TEST_PHRASE);
  });

  /**
   * A seed sealed by one CLI opens in the other. Both write the same envelope
   * under the same key at the same path, so this asserts the format rather than
   * the language. The Rust side pins the same property in
   * `crates/openagents-cli/tests/identity_test.rs`.
   */
  it("opens an envelope from a second store holding the same key", () => {
    isolatedIdentityDirectory();
    const key = new Uint8Array(32).fill(42);

    const writer = inMemoryKeyStore();
    writer.put(key);
    storeSeedPhrase(TEST_PHRASE, writer);

    const reader = inMemoryKeyStore();
    reader.put(key);
    expect(readSeedPhrase(reader)).toBe(TEST_PHRASE);
    expect(deriveSeedIdentity(readSeedPhrase(reader) ?? "").npub).toBe(FROZEN.npub);
  });

  /**
   * The keychain the two CLIs address. The service name, the account key, and
   * the operations must match `crates/openagents-cli/src/identity.rs`, or one
   * CLI mints a second key and orphans the other's seed.
   */
  it("addresses the keychain under a service of its own, keyed by identity directory", () => {
    const macos = identityKeychainCommandFor("darwin", "get", "/home/a/.openagents/identity");
    expect(macos).toEqual({
      command: "security",
      args: [
        "find-generic-password",
        "-a",
        "/home/a/.openagents/identity",
        "-s",
        "openagents-cli-identity",
        "-w",
      ],
    });

    const linux = identityKeychainCommandFor(
      "linux",
      "put",
      "/home/a/.openagents/identity",
      "ab".repeat(32),
    );
    expect(linux?.command).toBe("secret-tool");
    expect(linux?.args).toContain("openagents-cli-identity");
    // Linux takes the key on stdin; the account tokens keep their own service.
    expect(linux?.input).toBe("ab".repeat(32));
    expect(linux?.args).not.toContain("openagents-cli");
    expect(linux?.args).not.toContain("openagents-cli-computer");

    // No keychain on Windows, which is what selects the plaintext store there.
    expect(identityKeychainCommandFor("win32", "get", "C:/identity")).toBeUndefined();
  });
});
