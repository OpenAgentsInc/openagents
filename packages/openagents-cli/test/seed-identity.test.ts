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

import { chmodSync, mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  deriveSovereignIdentityPublic,
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
} from "@openagentsinc/sovereign-identity/contract";
import { afterEach, describe, expect, it } from "vitest";

import {
  deriveSeedIdentity,
  forgetSeedPhrase,
  generateSeedPhrase,
  isValidSeedPhrase,
  readSeedPhrase,
  seedPath,
  seedPresent,
  writeSeedPhrase,
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
  it("writes the phrase 0600 and reads it back unchanged", () => {
    isolatedIdentityDirectory();
    expect(seedPresent()).toBe(false);
    const path = writeSeedPhrase(`  ${TEST_PHRASE}  `);
    expect(path).toBe(seedPath());
    expect(seedPresent()).toBe(true);
    expect(statSync(path).mode & 0o777).toBe(0o600);
    expect(readSeedPhrase()).toBe(TEST_PHRASE);
    expect(deriveSeedIdentity(readSeedPhrase() ?? "").npub).toBe(FROZEN.npub);
  });

  it("restores 0600 when the file on disk was left readable", () => {
    isolatedIdentityDirectory();
    const path = writeSeedPhrase(TEST_PHRASE);
    chmodSync(path, 0o644);
    writeSeedPhrase(TEST_PHRASE);
    expect(statSync(path).mode & 0o777).toBe(0o600);
  });

  it("writes nothing when the phrase is not a valid mnemonic", () => {
    isolatedIdentityDirectory();
    expect(() => writeSeedPhrase("not a seed phrase at all")).toThrow(/valid English BIP-39/);
    expect(seedPresent()).toBe(false);
  });

  it("reports no seed for an absent or empty file, and forgets idempotently", () => {
    const directory = isolatedIdentityDirectory();
    expect(readSeedPhrase()).toBeUndefined();
    expect(forgetSeedPhrase()).toBe(false);
    writeFileSync(join(directory, "seed"), "   \n", { mode: 0o600 });
    expect(readSeedPhrase()).toBeUndefined();
    writeSeedPhrase(TEST_PHRASE);
    expect(forgetSeedPhrase()).toBe(true);
    expect(seedPresent()).toBe(false);
    expect(forgetSeedPhrase()).toBe(false);
  });

  it("keeps the seed out of everything except the seed file", () => {
    const directory = isolatedIdentityDirectory();
    writeSeedPhrase(TEST_PHRASE);
    const identity = deriveSeedIdentity(TEST_PHRASE);
    expect(readFileSync(join(directory, "seed"), "utf8")).toContain(TEST_PHRASE);
    expect(JSON.stringify(identity)).not.toContain("abandon");
    expect(JSON.stringify(identity)).not.toContain("nsec");
  });
});
