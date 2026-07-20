/**
 * IDR-09 retire plaintext compatibility — SAFELY, owner-gated, fail-closed.
 *
 * This is the FINAL packet of the sovereign-identity recovery program. It makes
 * it possible to retire the legacy plaintext identity file WITHOUT ever risking
 * the owner's recoverability, and WITHOUT ever deleting a real secret
 * autonomously. It ships three capabilities:
 *
 * 1. `makeReadOnlyLegacyPlaintextGuard` — during the verification period, legacy
 *    plaintext files are READ-ONLY. The guard refuses every write or delete of a
 *    protected legacy file, so recovery can be confirmed before anything is
 *    retired. It performs real filesystem work only for a NON-protected path or
 *    a read.
 * 2. `verifyRemainingBackupRestore` — on an ISOLATED profile with a FIXTURE, it
 *    proves an encrypted backup restores to the EXPECTED public identity. The
 *    proof is what "a verified remaining backup" means. The mnemonic never
 *    leaves the bounded `use` scope; only public identifiers return.
 * 3. `retirePlaintextCompatibility` — a FAIL-CLOSED gated deletion. It removes
 *    plaintext compatibility ONLY after three gates pass, IN ORDER, with NO
 *    filesystem mutation before all three succeed:
 *      (a) an explicit typed owner-confirmation token that names this identity,
 *      (b) a verified remaining backup that restores the SAME public `npub`,
 *      (c) a custody restore that re-derives the SAME public identity.
 *    Only then does it invoke the injected `LegacyPlaintextRemover` and write an
 *    exact PUBLIC-SAFE retirement receipt. It is NEVER invoked autonomously: no
 *    discovery, open, import, or boot path calls it, and normal caller code
 *    cannot reach it (see `retire.test.ts` — the "never auto-invoked" proof).
 *
 * SAFETY. Nothing here reads, copies, moves, or deletes the owner's LIVE secret
 * in development. The real filesystem remover exists so the machinery is
 * complete, but it runs ONLY behind all three gates and, in this repository, is
 * exercised ONLY against fixtures in a temporary directory. No receipt, result,
 * or error ever carries the mnemonic, `nsec`, raw key, seed, decrypted backup
 * data, a private path, or the owner confirmation token.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { rm, stat, writeFile } from "node:fs/promises";
import { Context, Effect, Layer, Schema as S } from "effect";
import { SecretNotFound } from "@openagentsinc/local-secret-store";
import type { IdentityRef, IsoTimestamp, Npub } from "../contract/index.ts";
import { derivePublicIdentity } from "../decode/boundary.ts";
import { decodeEncryptedPylonBackup } from "../decode/formats.ts";
import { restoreRootSecretPublicIdentity } from "./import.ts";
import { ManifestStore, buildRetirementReceipt } from "./manifest.ts";

/**
 * A typed retirement failure. It NEVER carries secret material and never carries
 * a raw private path — only a public reason.
 */
export class LegacyRetirementError extends S.TaggedErrorClass<LegacyRetirementError>()(
  "sovereign-identity.LegacyRetirementError",
  {
    reason: S.Literals([
      // The read-only guard refused a write or delete during the verification period.
      "read_only_during_verification",
      // No typed owner-confirmation token, or it does not name this identity.
      "owner_confirmation_missing",
      // No verified remaining backup, or the backup restored a different identity.
      "backup_not_verified",
      // The imported custody re-derived a DIFFERENT public identity than expected.
      "custody_restore_mismatch",
      // No secret is present in custody, so recoverability cannot be proven.
      "custody_absent",
      // The injected remover could not remove a plaintext source.
      "removal_failed",
      // A guarded filesystem write of a non-protected path failed.
      "write_failed",
    ]),
  },
) {}

// ---------------------------------------------------------------------------
// 1. Read-only-during-verification guard
// ---------------------------------------------------------------------------

/** A legacy plaintext file to protect: a PUBLIC label plus its private path. */
export interface ProtectedLegacyFile {
  /** A stable PUBLIC label for the source, never leaked into an error. */
  readonly sourceLabel: string;
  /** The private absolute path. It never appears in a result or error. */
  readonly absolutePath: string;
}

/** Existence-only metadata a read returns. It never carries file content. */
export interface LegacyFileMetadata {
  readonly present: boolean;
  readonly sizeBytes: number;
}

/**
 * The read-only guard surface. During the verification period it refuses any
 * mutation of a protected legacy plaintext file, so recovery can be confirmed
 * before anything is retired.
 */
export interface ReadOnlyLegacyPlaintextGuardInterface {
  /** Whether the verification period is active (legacy files stay read-only). */
  readonly verificationActive: boolean;
  /** The PUBLIC labels of the protected legacy files. */
  readonly protectedLabels: ReadonlyArray<string>;
  /** Read existence-only metadata of a path. Never refused. */
  readonly readMetadata: (
    absolutePath: string,
  ) => Effect.Effect<LegacyFileMetadata, LegacyRetirementError>;
  /** Write bytes. Refused for a protected legacy file while verification is active. */
  readonly writeFile: (
    absolutePath: string,
    bytes: Uint8Array,
  ) => Effect.Effect<void, LegacyRetirementError>;
  /** Delete a path. Refused for a protected legacy file while verification is active. */
  readonly deleteFile: (absolutePath: string) => Effect.Effect<void, LegacyRetirementError>;
}

/**
 * Build a read-only legacy guard over a fixed set of protected files. While
 * `verificationActive` is true, a write or delete of any protected path is
 * refused fail-closed and touches NO filesystem, so the legacy file is provably
 * unchanged. A read, or an operation on a non-protected path, performs real work.
 */
export const makeReadOnlyLegacyPlaintextGuard = (options: {
  readonly verificationActive: boolean;
  readonly protectedFiles: ReadonlyArray<ProtectedLegacyFile>;
}): ReadOnlyLegacyPlaintextGuardInterface => {
  const protectedPaths = new Set(options.protectedFiles.map((file) => file.absolutePath));
  const isProtected = (absolutePath: string): boolean =>
    options.verificationActive && protectedPaths.has(absolutePath);

  return {
    verificationActive: options.verificationActive,
    protectedLabels: options.protectedFiles.map((file) => file.sourceLabel),
    readMetadata: (absolutePath) =>
      Effect.tryPromise({
        try: async (): Promise<LegacyFileMetadata> => {
          try {
            const info = await stat(absolutePath);
            return { present: true, sizeBytes: info.size };
          } catch (error) {
            if ((error as { code?: string }).code === "ENOENT") {
              return { present: false, sizeBytes: 0 };
            }
            throw error;
          }
        },
        catch: () => new LegacyRetirementError({ reason: "write_failed" }),
      }),
    writeFile: (absolutePath, bytes) =>
      isProtected(absolutePath)
        ? Effect.fail(new LegacyRetirementError({ reason: "read_only_during_verification" }))
        : Effect.tryPromise({
            try: () => writeFile(absolutePath, bytes),
            catch: () => new LegacyRetirementError({ reason: "write_failed" }),
          }),
    deleteFile: (absolutePath) =>
      isProtected(absolutePath)
        ? Effect.fail(new LegacyRetirementError({ reason: "read_only_during_verification" }))
        : Effect.tryPromise({
            try: () => rm(absolutePath, { force: true }),
            catch: () => new LegacyRetirementError({ reason: "removal_failed" }),
          }),
  };
};

// ---------------------------------------------------------------------------
// 2. Isolated-profile remaining-backup restore proof
// ---------------------------------------------------------------------------

/** The encrypted backup a restore proof reads. All fields are public-safe inputs. */
export interface RemainingBackupInput {
  /** A PUBLIC label for the backup source, never a raw private path. */
  readonly backupLabel: string;
  /** The PUBLIC backup manifest (untrusted; schema-decoded by the decoder). */
  readonly manifest: unknown;
  /** The encrypted backup payload (untrusted; schema-decoded by the decoder). */
  readonly payload: unknown;
  /** The backup password. It is consumed only inside the decoder's bounded scope. */
  readonly password: string;
  /** The PUBLIC identity the backup MUST restore for the proof to hold. */
  readonly expectedNpub: Npub;
}

/**
 * The PUBLIC-safe result of a remaining-backup restore proof. It carries public
 * identifiers only. `verified` is true only when the backup restored EXACTLY the
 * expected `npub`.
 */
export interface VerifiedRemainingBackup {
  readonly verified: boolean;
  readonly backupLabel: string;
  readonly restoredNpub: string;
  readonly restoredNostrPublicKeyHex: string;
}

/**
 * Prove an encrypted backup restores to the EXPECTED public identity, on an
 * ISOLATED profile with a FIXTURE. It decodes the backup through the frozen
 * decoder, derives the PUBLIC identity inside the bounded secret scope, and
 * compares the `npub` to the expected one. The mnemonic never leaves the scope;
 * only public identifiers return.
 */
export const verifyRemainingBackupRestore = Effect.fn(
  "SovereignIdentity.verifyRemainingBackupRestore",
)(function* (input: RemainingBackupInput) {
  const decoded = yield* decodeEncryptedPylonBackup({
    manifest: input.manifest,
    payload: input.payload,
    password: input.password,
    sourcePathLabel: input.backupLabel,
  });
  // A backup that carries no shared-root phrase cannot prove recoverability.
  if (decoded.secret === null) {
    return {
      verified: false,
      backupLabel: input.backupLabel,
      restoredNpub: "",
      restoredNostrPublicKeyHex: "",
    } satisfies VerifiedRemainingBackup;
  }
  const restored = yield* derivePublicIdentity(decoded.secret);
  return {
    verified: restored.npub === input.expectedNpub,
    backupLabel: input.backupLabel,
    restoredNpub: restored.npub,
    restoredNostrPublicKeyHex: restored.nostrPublicKeyHex,
  } satisfies VerifiedRemainingBackup;
});

// ---------------------------------------------------------------------------
// 3. Fail-closed gated deletion + exact public-safe receipt
// ---------------------------------------------------------------------------

/**
 * The exact typed owner-confirmation phrase a retirement REQUIRES. It is a fixed
 * public phrase the owner types in an attended session; it is NOT a secret. The
 * confirmation token itself never appears in a receipt — only its public ref.
 */
export const OWNER_RETIREMENT_CONFIRMATION_PHRASE = "RETIRE OPENAGENTS PLAINTEXT IDENTITY";

/**
 * A typed owner-confirmation token. Retirement refuses unless this token is
 * present, names THIS identity, and carries the exact confirmation phrase.
 */
export interface OwnerRetirementConfirmation {
  /** The identity this confirmation authorizes. It must equal the retirement target. */
  readonly identityRef: IdentityRef;
  /** The exact confirmation phrase. It must equal `OWNER_RETIREMENT_CONFIRMATION_PHRASE`. */
  readonly confirmationPhrase: string;
  /** A PUBLIC reference to the owner confirmation event, recorded on the receipt. */
  readonly confirmationRef: string;
}

/** True only for a present token that names `identityRef` with the exact phrase. */
export const isOwnerRetirementConfirmed = (
  confirmation: OwnerRetirementConfirmation | null,
  identityRef: IdentityRef,
): confirmation is OwnerRetirementConfirmation =>
  confirmation !== null &&
  confirmation.identityRef === identityRef &&
  confirmation.confirmationPhrase === OWNER_RETIREMENT_CONFIRMATION_PHRASE;

/** The remover port. A host implements it; retirement injects it and calls it LAST. */
export interface LegacyPlaintextRemoverInterface {
  /** Remove ONE plaintext source. It returns the PUBLIC label of what it removed. */
  readonly remove: (
    file: ProtectedLegacyFile,
  ) => Effect.Effect<{ readonly sourceLabel: string }, LegacyRetirementError>;
}

/**
 * The `LegacyPlaintextRemover` service tag. It is an INJECTED port, so retirement
 * can never remove a file the composition root did not explicitly wire, and a
 * test can provide a temp-only remover.
 */
export class LegacyPlaintextRemover extends Context.Service<
  LegacyPlaintextRemover,
  LegacyPlaintextRemoverInterface
>()("sovereign-identity.LegacyPlaintextRemover") {}

/**
 * The real Node filesystem remover. It unlinks the private path. It is reachable
 * ONLY through `retirePlaintextCompatibility`, which runs it after all three
 * gates pass, and in this repository it is exercised only against fixtures in a
 * temporary directory. A missing file is treated as already-removed.
 */
export const nodeFsLegacyPlaintextRemoverLayer: Layer.Layer<LegacyPlaintextRemover> =
  Layer.succeed(
    LegacyPlaintextRemover,
    LegacyPlaintextRemover.of({
      remove: (file) =>
        Effect.tryPromise({
          try: async () => {
            await rm(file.absolutePath, { force: true });
            return { sourceLabel: file.sourceLabel };
          },
          catch: () => new LegacyRetirementError({ reason: "removal_failed" }),
        }),
    }),
  );

/** The public expected identity a retirement re-proves before it removes anything. */
export interface RetirementExpectedIdentity {
  readonly npub: Npub;
  readonly nostrPublicKeyHex: string;
  readonly sparkPublicKeyHex: string;
  readonly sparkBip32FingerprintHex: string;
}

/** The public inputs a retirement needs. Every field is public; no secret is passed. */
export interface RetirePlaintextCompatibilityInput {
  readonly identityRef: IdentityRef;
  /** The public identity the custody restore MUST re-derive. */
  readonly expected: RetirementExpectedIdentity;
  /** The typed owner-confirmation token, or `null` when absent. Gate (a). */
  readonly ownerConfirmation: OwnerRetirementConfirmation | null;
  /** The verified remaining-backup proof from `verifyRemainingBackupRestore`. Gate (b). */
  readonly verifiedBackup: VerifiedRemainingBackup;
  /** The legacy plaintext sources to retire, by PUBLIC label and private path. */
  readonly plaintextSources: ReadonlyArray<ProtectedLegacyFile>;
  /** The public reference for the retirement receipt this run records. */
  readonly receiptRef: string;
  /** When this retirement ran (public). */
  readonly retiredAt: IsoTimestamp;
}

/** The public-safe outcome of a retirement. It carries no secret material. */
export interface RetirePlaintextCompatibilityOutcome {
  readonly identityRef: IdentityRef;
  readonly npub: string;
  readonly retiredSourceLabels: ReadonlyArray<string>;
  readonly receiptRef: string;
}

/**
 * Retire plaintext compatibility, FAIL-CLOSED. It removes plaintext ONLY after
 * gate (a) owner confirmation, gate (b) a verified remaining backup, and gate (c)
 * a custody restore of the SAME public identity all pass — in that order, with NO
 * filesystem mutation before all three succeed. It requires the injected
 * `LocalSecretStore`, `ManifestStore`, and `LegacyPlaintextRemover`.
 */
export const retirePlaintextCompatibility = Effect.fn(
  "SovereignIdentity.retirePlaintextCompatibility",
)(function* (input: RetirePlaintextCompatibilityInput) {
  // Gate (a): a typed owner-confirmation token that names THIS identity. No
  // filesystem work happens before this passes.
  if (!isOwnerRetirementConfirmed(input.ownerConfirmation, input.identityRef)) {
    return yield* new LegacyRetirementError({ reason: "owner_confirmation_missing" });
  }
  const ownerConfirmation = input.ownerConfirmation;

  // Gate (b): a verified remaining backup that restored the SAME public npub.
  if (!input.verifiedBackup.verified || input.verifiedBackup.restoredNpub !== input.expected.npub) {
    return yield* new LegacyRetirementError({ reason: "backup_not_verified" });
  }

  // Gate (c): read the imported custody back and re-derive the public identity.
  // A missing secret is `custody_absent`; a mismatch is `custody_restore_mismatch`.
  const restored = yield* restoreRootSecretPublicIdentity(input.identityRef).pipe(
    Effect.mapError(
      (error) =>
        new LegacyRetirementError({
          reason: error instanceof SecretNotFound ? "custody_absent" : "custody_restore_mismatch",
        }),
    ),
  );
  if (
    restored.npub !== input.expected.npub ||
    restored.nostrPublicKeyHex !== input.expected.nostrPublicKeyHex ||
    restored.sparkPublicKeyHex !== input.expected.sparkPublicKeyHex ||
    restored.sparkBip32FingerprintHex !== input.expected.sparkBip32FingerprintHex
  ) {
    return yield* new LegacyRetirementError({ reason: "custody_restore_mismatch" });
  }

  // Every gate passed. ONLY NOW may plaintext compatibility be removed.
  const remover = yield* LegacyPlaintextRemover;
  const manifests = yield* ManifestStore;
  const retiredSourceLabels: string[] = [];
  for (const source of input.plaintextSources) {
    const removed = yield* remover.remove(source);
    retiredSourceLabels.push(removed.sourceLabel);
  }

  // Record the exact PUBLIC-SAFE retirement receipt.
  const receipt = yield* buildRetirementReceipt({
    receiptRef: input.receiptRef,
    identityRef: input.identityRef,
    npub: input.expected.npub,
    nostrPublicKeyHex: input.expected.nostrPublicKeyHex,
    retiredSourceLabels,
    verifiedBackupLabel: input.verifiedBackup.backupLabel,
    ownerConfirmationRef: ownerConfirmation.confirmationRef,
    retiredAt: input.retiredAt,
  });
  yield* manifests.writeRetirementReceipt(receipt);

  return {
    identityRef: input.identityRef,
    npub: input.expected.npub,
    retiredSourceLabels,
    receiptRef: input.receiptRef,
  } satisfies RetirePlaintextCompatibilityOutcome;
});
