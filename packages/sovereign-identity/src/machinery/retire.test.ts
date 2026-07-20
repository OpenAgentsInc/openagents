import { existsSync, readFileSync, readdirSync } from "node:fs";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  inMemoryLocalSecretStoreLayerWith,
  secretLocatorKey,
} from "@openagentsinc/local-secret-store";
import { Effect, Layer, Result } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IdentityRef,
  IsoTimestamp,
  LOCAL_IDENTITY_SECRET_SCHEMA,
  Npub,
} from "../contract/index.ts";
import { PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE, PUBLIC_TEST_MNEMONIC } from "../contract/vectors.ts";
import { encryptedPylonBackupFixtureInput } from "../decode/fixtures.ts";
import { ManifestStore, inMemoryManifestStoreLayer } from "./manifest.ts";
import {
  LegacyRetirementError,
  OWNER_RETIREMENT_CONFIRMATION_PHRASE,
  type OwnerRetirementConfirmation,
  type ProtectedLegacyFile,
  type VerifiedRemainingBackup,
  makeReadOnlyLegacyPlaintextGuard,
  nodeFsLegacyPlaintextRemoverLayer,
  retirePlaintextCompatibility,
  verifyRemainingBackupRestore,
} from "./retire.ts";
import { rootSecretLocator } from "./service.ts";

/**
 * IDR-09 retire-plaintext-compatibility tests.
 *
 * SAFETY: every secret is the PUBLIC published BIP-39 TEST mnemonic — never a
 * real secret. Every filesystem operation runs in an ISOLATED temporary
 * directory. No OS keychain call runs (the in-memory adapter reports
 * `in_memory_unprotected`). Every receipt, result, and error is asserted to
 * carry NO mnemonic word.
 */

const IDENTITY_REF = IdentityRef.make("idr09-retire-plaintext");
const RETIRED_AT = IsoTimestamp.make("2026-07-20T00:00:02Z");
const reference = PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE;
const EXPECTED = {
  npub: Npub.make(reference.npub),
  nostrPublicKeyHex: reference.nostrPublicKeyHex,
  sparkPublicKeyHex: reference.sparkPublicKeyHex,
  sparkBip32FingerprintHex: reference.sparkBip32FingerprintHex,
};

// A well-known DIFFERENT valid BIP-39 test mnemonic, for the custody-mismatch gate.
const DIFFERENT_VALID_MNEMONIC =
  "legal winner thank year wave sausage worth useful legal winner thank yellow";

/** The encrypted Pylon backup fixture, narrowed to its public-safe fields. */
const backupFixture = encryptedPylonBackupFixtureInput as unknown as {
  readonly manifest: unknown;
  readonly payload: unknown;
  readonly password: string;
};

const OWNER_CONFIRMATION: OwnerRetirementConfirmation = {
  identityRef: IDENTITY_REF,
  confirmationPhrase: OWNER_RETIREMENT_CONFIRMATION_PHRASE,
  confirmationRef: "owner-confirmation:idr09-attended-2026-07-20",
};

const secretPayloadFor = (mnemonic: string): Uint8Array =>
  new TextEncoder().encode(
    JSON.stringify({
      schema: LOCAL_IDENTITY_SECRET_SCHEMA,
      mnemonic,
      language: "english",
      bip39PassphraseMode: "empty",
    }),
  );

/** Seed the in-memory custody store with a mnemonic, as IDR-05 import would. */
const seedCustody = (backing: Map<string, Uint8Array>, mnemonic: string): void => {
  backing.set(secretLocatorKey(rootSecretLocator(IDENTITY_REF)), secretPayloadFor(mnemonic));
};

const VERIFIED_BACKUP: VerifiedRemainingBackup = {
  verified: true,
  backupLabel: "user-selected wallet backup export path",
  restoredNpub: reference.npub,
  restoredNostrPublicKeyHex: reference.nostrPublicKeyHex,
};

// ---------------------------------------------------------------------------
// 1. Read-only-during-verification guard
// ---------------------------------------------------------------------------

describe("IDR-09 the read-only guard keeps legacy plaintext files read-only during verification", () => {
  test("it REFUSES to write or delete a protected legacy file, and the file is untouched", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "idr09-guard-"));
    const legacyPath = path.join(dir, "identity.mnemonic");
    const original = "PUBLIC-SAFE FIXTURE LEGACY CONTENT";
    await writeFile(legacyPath, original, "utf8");

    const file: ProtectedLegacyFile = {
      sourceLabel: "~/.openagents/pylon/identity.mnemonic",
      absolutePath: legacyPath,
    };
    const guard = makeReadOnlyLegacyPlaintextGuard({
      verificationActive: true,
      protectedFiles: [file],
    });

    const writeResult = await Effect.runPromise(
      Effect.result(guard.writeFile(legacyPath, new TextEncoder().encode("OVERWRITE"))),
    );
    const deleteResult = await Effect.runPromise(Effect.result(guard.deleteFile(legacyPath)));

    expect(Result.isFailure(writeResult)).toBe(true);
    expect(Result.isFailure(deleteResult)).toBe(true);
    for (const result of [writeResult, deleteResult]) {
      if (Result.isFailure(result)) {
        expect(result.failure).toBeInstanceOf(LegacyRetirementError);
        expect((result.failure as LegacyRetirementError).reason).toBe(
          "read_only_during_verification",
        );
      }
    }

    // Fail-closed: the legacy file is BYTE-IDENTICAL and still present.
    expect(existsSync(legacyPath)).toBe(true);
    expect(await readFile(legacyPath, "utf8")).toBe(original);
    expect(guard.protectedLabels).toContain("~/.openagents/pylon/identity.mnemonic");
  });

  test("it performs a real write for a NON-protected path (the guard is not a no-op)", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "idr09-guard-allow-"));
    const legacyPath = path.join(dir, "identity.mnemonic");
    const otherPath = path.join(dir, "not-protected.txt");
    const guard = makeReadOnlyLegacyPlaintextGuard({
      verificationActive: true,
      protectedFiles: [{ sourceLabel: "legacy", absolutePath: legacyPath }],
    });

    await Effect.runPromise(guard.writeFile(otherPath, new TextEncoder().encode("allowed")));
    expect(existsSync(otherPath)).toBe(true);
    expect(await readFile(otherPath, "utf8")).toBe("allowed");
  });

  test("once verification is complete the guard no longer refuses (the read-only period ended)", async () => {
    const dir = await mkdtemp(path.join(tmpdir(), "idr09-guard-done-"));
    const legacyPath = path.join(dir, "identity.mnemonic");
    await writeFile(legacyPath, "fixture", "utf8");
    const guard = makeReadOnlyLegacyPlaintextGuard({
      verificationActive: false,
      protectedFiles: [{ sourceLabel: "legacy", absolutePath: legacyPath }],
    });
    // Deletion here is a plain guard call, NOT the gated retirement path; it only
    // proves the read-only refusal is scoped to the verification period.
    await Effect.runPromise(guard.deleteFile(legacyPath));
    expect(existsSync(legacyPath)).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 2. Isolated-profile remaining-backup restore proof
// ---------------------------------------------------------------------------

describe("IDR-09 an encrypted backup restores to the expected public identity on an ISOLATED profile", () => {
  test("a fixture backup written into an isolated temp profile restores the SAME npub", async () => {
    const profile = await mkdtemp(path.join(tmpdir(), "idr09-isolated-profile-"));
    const manifestPath = path.join(profile, "backup.manifest.json");
    const payloadPath = path.join(profile, "backup.payload.json");
    await writeFile(manifestPath, JSON.stringify(backupFixture.manifest), "utf8");
    await writeFile(payloadPath, JSON.stringify(backupFixture.payload), "utf8");

    const manifest = JSON.parse(await readFile(manifestPath, "utf8")) as unknown;
    const payload = JSON.parse(await readFile(payloadPath, "utf8")) as unknown;

    const proof = await Effect.runPromise(
      verifyRemainingBackupRestore({
        backupLabel: "isolated-profile fixture backup",
        manifest,
        payload,
        password: backupFixture.password,
        expectedNpub: EXPECTED.npub,
      }),
    );

    expect(proof.verified).toBe(true);
    expect(proof.restoredNpub).toBe(reference.npub);
    expect(proof.restoredNostrPublicKeyHex).toBe(reference.nostrPublicKeyHex);
    // Tripwire: the proof carries no secret material.
    const blob = JSON.stringify(proof);
    expect(blob).not.toContain("abandon");
    expect(blob).not.toContain(PUBLIC_TEST_MNEMONIC);
  });

  test("a backup that restores a DIFFERENT identity is not verified", async () => {
    const proof = await Effect.runPromise(
      verifyRemainingBackupRestore({
        backupLabel: "isolated-profile fixture backup",
        manifest: backupFixture.manifest,
        payload: backupFixture.payload,
        password: backupFixture.password,
        expectedNpub: Npub.make(
          "npub1000000000000000000000000000000000000000000000000000000000000",
        ),
      }),
    );
    expect(proof.verified).toBe(false);
  });
});

// ---------------------------------------------------------------------------
// 3. Fail-closed gated deletion + exact public-safe receipt
// ---------------------------------------------------------------------------

/** Build one isolated temp plaintext source and its protected-file descriptor. */
const makeIsolatedPlaintextSource = async (): Promise<ProtectedLegacyFile> => {
  const dir = await mkdtemp(path.join(tmpdir(), "idr09-retire-"));
  const legacyPath = path.join(dir, "identity.mnemonic");
  await writeFile(legacyPath, "PUBLIC-SAFE FIXTURE LEGACY PLAINTEXT", "utf8");
  return { sourceLabel: "~/.openagents/pylon/identity.mnemonic", absolutePath: legacyPath };
};

const runRetire = (options: {
  readonly backing: Map<string, Uint8Array>;
  readonly ownerConfirmation: OwnerRetirementConfirmation | null;
  readonly verifiedBackup: VerifiedRemainingBackup;
  readonly plaintextSources: ReadonlyArray<ProtectedLegacyFile>;
}) =>
  Effect.runPromise(
    Effect.provide(
      Effect.result(
        retirePlaintextCompatibility({
          identityRef: IDENTITY_REF,
          expected: EXPECTED,
          ownerConfirmation: options.ownerConfirmation,
          verifiedBackup: options.verifiedBackup,
          plaintextSources: options.plaintextSources,
          receiptRef: "retirement-receipt:idr09",
          retiredAt: RETIRED_AT,
        }),
      ),
      Layer.mergeAll(
        inMemoryLocalSecretStoreLayerWith(options.backing),
        inMemoryManifestStoreLayer,
        nodeFsLegacyPlaintextRemoverLayer,
      ),
    ),
  );

const expectRefusal = async (
  result: Awaited<ReturnType<typeof runRetire>>,
  reason: string,
  source: ProtectedLegacyFile,
): Promise<void> => {
  expect(Result.isFailure(result)).toBe(true);
  if (Result.isFailure(result)) {
    expect(result.failure).toBeInstanceOf(LegacyRetirementError);
    expect((result.failure as LegacyRetirementError).reason).toBe(reason);
  }
  // Fail-closed: the plaintext source was NOT removed.
  expect(existsSync(source.absolutePath)).toBe(true);
};

describe("IDR-09 the gated deletion is FAIL-CLOSED and never removes plaintext without every gate", () => {
  test("it REFUSES with no owner-confirmation token, and removes nothing", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, PUBLIC_TEST_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: null,
      verifiedBackup: VERIFIED_BACKUP,
      plaintextSources: [source],
    });
    await expectRefusal(result, "owner_confirmation_missing", source);
  });

  test("it REFUSES with the WRONG confirmation phrase", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, PUBLIC_TEST_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: { ...OWNER_CONFIRMATION, confirmationPhrase: "yes please delete it" },
      verifiedBackup: VERIFIED_BACKUP,
      plaintextSources: [source],
    });
    await expectRefusal(result, "owner_confirmation_missing", source);
  });

  test("it REFUSES when the remaining backup is not verified", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, PUBLIC_TEST_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: OWNER_CONFIRMATION,
      verifiedBackup: { ...VERIFIED_BACKUP, verified: false },
      plaintextSources: [source],
    });
    await expectRefusal(result, "backup_not_verified", source);
  });

  test("it REFUSES when the verified backup restored a DIFFERENT npub", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, PUBLIC_TEST_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: OWNER_CONFIRMATION,
      verifiedBackup: { ...VERIFIED_BACKUP, restoredNpub: "npub1differentbackuprestoredidentity" },
      plaintextSources: [source],
    });
    await expectRefusal(result, "backup_not_verified", source);
  });

  test("it REFUSES when no secret is present in custody (custody_absent)", async () => {
    const backing = new Map<string, Uint8Array>(); // custody NOT seeded
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: OWNER_CONFIRMATION,
      verifiedBackup: VERIFIED_BACKUP,
      plaintextSources: [source],
    });
    await expectRefusal(result, "custody_absent", source);
  });

  test("it REFUSES when the custody restore re-derives a DIFFERENT identity", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, DIFFERENT_VALID_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    const result = await runRetire({
      backing,
      ownerConfirmation: OWNER_CONFIRMATION,
      verifiedBackup: VERIFIED_BACKUP,
      plaintextSources: [source],
    });
    await expectRefusal(result, "custody_restore_mismatch", source);
  });

  test("with EVERY gate satisfied it removes plaintext and writes an EXACT public-safe receipt", async () => {
    const backing = new Map<string, Uint8Array>();
    seedCustody(backing, PUBLIC_TEST_MNEMONIC);
    const source = await makeIsolatedPlaintextSource();
    // Pre-condition: the plaintext source EXISTS before the gated retirement.
    expect(existsSync(source.absolutePath)).toBe(true);

    const { result, receipt } = await Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const out = yield* Effect.result(
            retirePlaintextCompatibility({
              identityRef: IDENTITY_REF,
              expected: EXPECTED,
              ownerConfirmation: OWNER_CONFIRMATION,
              verifiedBackup: VERIFIED_BACKUP,
              plaintextSources: [source],
              receiptRef: "retirement-receipt:idr09",
              retiredAt: RETIRED_AT,
            }),
          );
          const manifests = yield* ManifestStore;
          const stored = yield* manifests.readRetirementReceipt("retirement-receipt:idr09");
          return { result: out, receipt: stored } as const;
        }),
        Layer.mergeAll(
          inMemoryLocalSecretStoreLayerWith(backing),
          inMemoryManifestStoreLayer,
          nodeFsLegacyPlaintextRemoverLayer,
        ),
      ),
    );

    expect(Result.isSuccess(result)).toBe(true);
    if (Result.isSuccess(result)) {
      expect(result.success.npub).toBe(reference.npub);
      expect(result.success.retiredSourceLabels).toEqual([source.sourceLabel]);
    }
    // The plaintext source is now removed — only through the fully-gated path.
    expect(existsSync(source.absolutePath)).toBe(false);

    // The exact public-safe retirement receipt exists and proves every gate.
    expect(receipt).not.toBe(null);
    expect(receipt?.schema).toBe("openagents.local_identity_plaintext_retirement_receipt.v1");
    expect(receipt?.npub).toBe(reference.npub);
    expect(receipt?.custodyRestoreVerified).toBe(true);
    expect(receipt?.verifiedBackupLabel).toBe(VERIFIED_BACKUP.backupLabel);
    expect(receipt?.ownerConfirmationRef).toBe(OWNER_CONFIRMATION.confirmationRef);
    expect(receipt?.retiredSourceLabels).toContain(source.sourceLabel);

    // Tripwire: the receipt carries no secret and not even the confirmation token.
    const blob = JSON.stringify(receipt);
    expect(blob).not.toContain("abandon");
    expect(blob).not.toContain(PUBLIC_TEST_MNEMONIC);
    expect(blob).not.toContain(OWNER_RETIREMENT_CONFIRMATION_PHRASE);
  });
});

// ---------------------------------------------------------------------------
// 4. Proof: retirement is NEVER invoked autonomously
// ---------------------------------------------------------------------------

describe("IDR-09 retirement is never invoked autonomously", () => {
  const srcRoot = path.resolve(import.meta.dirname, "..");
  const packagesRoot = path.resolve(srcRoot, "../..");
  const repoRoot = path.resolve(packagesRoot, "..");

  const sourceFiles = (root: string): ReadonlyArray<string> => {
    if (!existsSync(root)) return [];
    return readdirSync(root, { withFileTypes: true }).flatMap((entry) => {
      const candidate = path.join(root, entry.name);
      if (entry.isDirectory()) return sourceFiles(candidate);
      if (!/\.tsx?$/u.test(entry.name)) return [];
      if (/\.test\.tsx?$/u.test(entry.name)) return [];
      return [candidate];
    });
  };

  // The gated deletion entrypoint. Its ONLY legitimate site is its own
  // definition; no discovery, open, import, boot, or app path may call it.
  const RETIREMENT_ENTRYPOINT = "retirePlaintextCompatibility";
  const RETIRE_MODULE = path.join(srcRoot, "machinery", "retire.ts");

  test("no non-test source outside retire.ts references the gated deletion entrypoint", () => {
    const roots = [
      srcRoot,
      path.join(repoRoot, "packages", "pylon-core", "src"),
      path.join(repoRoot, "apps", "openagents-desktop", "src"),
    ];
    const offenders: string[] = [];
    for (const root of roots) {
      for (const file of sourceFiles(root)) {
        if (path.resolve(file) === path.resolve(RETIRE_MODULE)) continue; // its own definition
        if (readFileSync(file, "utf8").includes(RETIREMENT_ENTRYPOINT)) {
          offenders.push(path.relative(repoRoot, file));
        }
      }
    }
    expect(offenders).toEqual([]);
  });

  test("the boot/discovery/open/import machinery never calls retirement", () => {
    const machineryDir = path.join(srcRoot, "machinery");
    const offenders: string[] = [];
    for (const file of sourceFiles(machineryDir)) {
      if (path.resolve(file) === path.resolve(RETIRE_MODULE)) continue;
      if (readFileSync(file, "utf8").includes(RETIREMENT_ENTRYPOINT)) {
        offenders.push(path.relative(repoRoot, file));
      }
    }
    expect(offenders).toEqual([]);
  });
});
