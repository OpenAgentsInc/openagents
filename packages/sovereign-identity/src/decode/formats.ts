/**
 * IDR-03 per-format decoders.
 *
 * One decoder per historical secret format the audit lists. Each decoder takes
 * an admitted candidate that discovery surfaced (IDR-02) and produces a
 * `DecodedCandidate`: a bounded `RecoveredSecret` plus a PUBLIC-safe result. A
 * decoder NEVER writes, creates, or overwrites a file; it works only on the
 * in-memory admitted input.
 *
 * The decodable-offline formats are the plain mnemonic file, the sovereign agent
 * TOML, and the Electron insecure-mode base64 record. The encrypted formats
 * (Compute `identity.enc`, Wallet keyring, Pylon backup) reproduce their exact
 * legacy KDF + AEAD layout through `legacy-crypto.ts`. A normal Electron OS
 * `safeStorage` record is platform-opaque and returns `owner_attended_required`.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { Effect, Schema as S } from "effect";
import {
  type HistoricalFormatId,
  isValidEnglishMnemonic,
  normalizeMnemonic,
} from "../contract/index.ts";
import { RecoveredSecret } from "./boundary.ts";
import {
  aes256GcmOpen,
  chacha20Poly1305Open,
  deriveArgon2idKey,
  deriveScryptKey,
  xchacha20Poly1305Open,
} from "./legacy-crypto.ts";
import {
  type CandidateDecodeResult,
  DecodeCandidateError,
  type DecodedCandidate,
  type DecodeFailureReason,
} from "./result.ts";

const utf8Bytes = (value: string): Uint8Array => new TextEncoder().encode(value);
const utf8Text = (bytes: Uint8Array): string => new TextDecoder().decode(bytes);
const base64Bytes = (value: string): Uint8Array => new Uint8Array(Buffer.from(value, "base64"));

/** A shared Argon2id-parameter schema for the encrypted envelopes. */
const Argon2idParamsSchema = S.Struct({
  memoryKib: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  iterations: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  parallelism: S.Number.check(S.isInt(), S.isGreaterThan(0)),
});

/** A shared scrypt-parameter schema for the encrypted backup. */
const ScryptParamsSchema = S.Struct({
  N: S.Number.check(S.isInt(), S.isGreaterThan(1)),
  r: S.Number.check(S.isInt(), S.isGreaterThan(0)),
  p: S.Number.check(S.isInt(), S.isGreaterThan(0)),
});

const Base64String = S.String.check(S.isMinLength(1));

/** The Compute `identity.enc` envelope: Argon2id key + AES-256-GCM. */
export const ComputeIdentityEncEnvelope = S.Struct({
  version: S.Number.check(S.isInt()),
  kdf: S.Literal("argon2id"),
  argon2: Argon2idParamsSchema,
  cipher: S.Literal("aes-256-gcm"),
  saltB64: Base64String,
  nonceB64: Base64String,
  /** Base64 of `ciphertext || tag`. */
  ciphertextB64: Base64String,
});

/** The Wallet keyring envelope: Argon2id key + ChaCha20-Poly1305. */
export const WalletKeyringEnvelope = S.Struct({
  version: S.Number.check(S.isInt()),
  keyringService: S.String,
  keyringAccount: S.String,
  kdf: S.Literal("argon2id"),
  argon2: Argon2idParamsSchema,
  cipher: S.Literal("chacha20poly1305"),
  saltB64: Base64String,
  nonceB64: Base64String,
  /** Base64 of `ciphertext || tag`. */
  ciphertextB64: Base64String,
});

/** The Electron desktop safe-storage record. */
export const ElectronSafeStorageEnvelope = S.Struct({
  schemaVersion: S.Number.check(S.isInt()),
  key: S.String,
  encoding: S.Literal("base64"),
  /** Insecure development mode stores base64 of the raw UTF-8 phrase. */
  insecureMode: S.Boolean,
  valueBase64: Base64String,
});

/** The PUBLIC Pylon backup manifest a discovery tool inspects first. */
export const PylonBackupManifest = S.Struct({
  manifestVersion: S.Number.check(S.isInt()),
  cipher: S.Literal("xchacha20poly1305"),
  kdf: S.Literal("scrypt"),
  scrypt: ScryptParamsSchema,
  identity_mnemonic_included: S.Boolean,
});

/** The encrypted Pylon backup payload: scrypt key + XChaCha20-Poly1305. */
export const PylonBackupPayload = S.Struct({
  saltB64: Base64String,
  /** Base64 of the 24-byte XChaCha nonce. */
  nonceB64: Base64String,
  /** Base64 of `ciphertext || tag`. */
  ciphertextB64: Base64String,
});

// ---------------------------------------------------------------------------
// Per-format inputs
// ---------------------------------------------------------------------------

/** The plain BIP-39 mnemonic file input. */
export interface PlainMnemonicInput {
  readonly contentUtf8: string;
  readonly sourcePathLabel?: string;
}

/** The Compute `identity.enc` input. `envelope` is untrusted and schema-decoded. */
export interface ComputeIdentityEncInput {
  readonly envelope: unknown;
  readonly password: string;
  readonly sourcePathLabel?: string;
}

/** The Wallet keyring input. `envelope` is untrusted and schema-decoded. */
export interface WalletKeyringInput {
  readonly envelope: unknown;
  readonly password: string;
  readonly sourcePathLabel?: string;
}

/** The Electron safe-storage input. `envelope` is untrusted and schema-decoded. */
export interface ElectronSafeStorageInput {
  readonly envelope: unknown;
  readonly sourcePathLabel?: string;
}

/** The encrypted Pylon backup input. The manifest is inspected before the payload. */
export interface EncryptedPylonBackupInput {
  readonly manifest: unknown;
  readonly payload?: unknown;
  readonly password?: string;
  readonly sourcePathLabel?: string;
}

/** The sovereign agent TOML input. */
export interface SovereignAgentTomlInput {
  readonly tomlText: string;
  readonly sourcePathLabel?: string;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

const DEFAULT_SOURCE_LABEL: Readonly<Record<HistoricalFormatId, string>> = {
  plain_mnemonic_file: "~/.openagents/pylon/identity.mnemonic",
  compute_identity_enc: "<config_dir>/openagents/compute/identity.enc",
  wallet_keyring_envelope: "OS keyring service openagents-wallet, account mnemonic",
  electron_safe_storage_record: "<Electron-userData>/secure/desktop-secure-storage.json",
  encrypted_pylon_backup: "user-selected wallet backup export path",
  sovereign_agent_toml: "<config_dir>/openagents/agents/<npub>.toml",
};

const FORMAT_LABEL: Readonly<Record<HistoricalFormatId, string>> = {
  plain_mnemonic_file: "Plain BIP-39 mnemonic file",
  compute_identity_enc: "Compute identity.enc (AES-256-GCM, Argon2-derived key)",
  wallet_keyring_envelope: "Wallet keyring envelope (Argon2, ChaCha20-Poly1305)",
  electron_safe_storage_record: "Electron desktop safe-storage record (spark.wallet.mnemonic)",
  encrypted_pylon_backup: "Encrypted Pylon wallet backup (XChaCha20-Poly1305, scrypt)",
  sovereign_agent_toml: "Sovereign agent TOML (mnemonic_encrypted holds the raw phrase)",
};

const fail = (format: HistoricalFormatId, reason: DecodeFailureReason) =>
  Effect.fail(new DecodeCandidateError({ format, reason }));

/** Decode an untrusted envelope with a schema, mapping a parse error to `malformed_envelope`. */
const decodeEnvelope = <Sch extends S.Top>(
  format: HistoricalFormatId,
  schema: Sch,
  input: unknown,
): Effect.Effect<Sch["Type"], DecodeCandidateError, Sch["DecodingServices"]> =>
  S.decodeUnknownEffect(schema)(input).pipe(
    Effect.mapError(() => new DecodeCandidateError({ format, reason: "malformed_envelope" })),
  );

/**
 * Validate a recovered phrase and wrap it in a bounded `RecoveredSecret` plus a
 * public-safe `decoded` result. An invalid BIP-39 phrase fails closed WITHOUT
 * echoing the phrase into the error.
 */
const finalizeDecoded = (
  format: HistoricalFormatId,
  formatVersion: string,
  sourcePathLabel: string,
  rawMnemonic: string,
): Effect.Effect<DecodedCandidate, DecodeCandidateError> => {
  const mnemonic = normalizeMnemonic(rawMnemonic);
  if (!isValidEnglishMnemonic(mnemonic)) return fail(format, "invalid_mnemonic");
  const secret = new RecoveredSecret(mnemonic, format, formatVersion);
  const result: CandidateDecodeResult = {
    format,
    formatLabel: FORMAT_LABEL[format],
    formatVersion,
    sourcePathLabel,
    status: "decoded",
    decoded: true,
    publicIdentity: null,
  };
  return Effect.succeed({ result, secret });
};

/** Build an `owner_attended_required` result with no bounded secret. */
const ownerAttended = (
  format: HistoricalFormatId,
  formatVersion: string,
  sourcePathLabel: string,
  note: string,
): DecodedCandidate => ({
  result: {
    format,
    formatLabel: FORMAT_LABEL[format],
    formatVersion,
    sourcePathLabel,
    status: "owner_attended_required",
    decoded: false,
    publicIdentity: null,
    note,
  },
  secret: null,
});

// ---------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------

/** 1. Decode a plain BIP-39 mnemonic file (`identity.mnemonic`). */
export const decodePlainMnemonicFile = Effect.fn("SovereignIdentity.decodePlainMnemonicFile")(
  function* (input: PlainMnemonicInput) {
    return yield* finalizeDecoded(
      "plain_mnemonic_file",
      "plain-text-v1",
      input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.plain_mnemonic_file,
      input.contentUtf8,
    );
  },
);

/** 2. Decode a Compute `identity.enc` record (Argon2id key + AES-256-GCM). */
export const decodeComputeIdentityEnc = Effect.fn("SovereignIdentity.decodeComputeIdentityEnc")(
  function* (input: ComputeIdentityEncInput) {
    if (input.password.length === 0) return yield* fail("compute_identity_enc", "missing_password");
    const envelope = yield* decodeEnvelope(
      "compute_identity_enc",
      ComputeIdentityEncEnvelope,
      input.envelope,
    );
    const key = deriveArgon2idKey(
      utf8Bytes(input.password),
      base64Bytes(envelope.saltB64),
      envelope.argon2,
    );
    const plaintext = yield* Effect.try({
      try: () =>
        aes256GcmOpen(key, base64Bytes(envelope.nonceB64), base64Bytes(envelope.ciphertextB64)),
      catch: () =>
        new DecodeCandidateError({ format: "compute_identity_enc", reason: "decrypt_failed" }),
    });
    return yield* finalizeDecoded(
      "compute_identity_enc",
      "compute-identity-enc-v1",
      input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.compute_identity_enc,
      utf8Text(plaintext),
    );
  },
);

/** 3. Decode a Wallet keyring envelope (Argon2id key + ChaCha20-Poly1305). */
export const decodeWalletKeyringEnvelope = Effect.fn(
  "SovereignIdentity.decodeWalletKeyringEnvelope",
)(function* (input: WalletKeyringInput) {
  if (input.password.length === 0)
    return yield* fail("wallet_keyring_envelope", "missing_password");
  const envelope = yield* decodeEnvelope(
    "wallet_keyring_envelope",
    WalletKeyringEnvelope,
    input.envelope,
  );
  const key = deriveArgon2idKey(
    utf8Bytes(input.password),
    base64Bytes(envelope.saltB64),
    envelope.argon2,
  );
  const plaintext = yield* Effect.try({
    try: () =>
      chacha20Poly1305Open(
        key,
        base64Bytes(envelope.nonceB64),
        base64Bytes(envelope.ciphertextB64),
      ),
    catch: () =>
      new DecodeCandidateError({ format: "wallet_keyring_envelope", reason: "decrypt_failed" }),
  });
  return yield* finalizeDecoded(
    "wallet_keyring_envelope",
    "wallet-keyring-envelope-v1",
    input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.wallet_keyring_envelope,
    utf8Text(plaintext),
  );
});

/**
 * 4. Decode an Electron desktop safe-storage record. Insecure development mode
 * stores base64 of the raw UTF-8 phrase and decodes offline. A normal record
 * holds platform-opaque OS `safeStorage` ciphertext and returns
 * `owner_attended_required`.
 */
export const decodeElectronSafeStorageRecord = Effect.fn(
  "SovereignIdentity.decodeElectronSafeStorageRecord",
)(function* (input: ElectronSafeStorageInput) {
  const envelope = yield* decodeEnvelope(
    "electron_safe_storage_record",
    ElectronSafeStorageEnvelope,
    input.envelope,
  );
  const sourcePathLabel =
    input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.electron_safe_storage_record;
  if (!envelope.insecureMode) {
    return ownerAttended(
      "electron_safe_storage_record",
      "desktop-secure-storage-v1",
      sourcePathLabel,
      "Normal Electron OS safeStorage ciphertext is platform-opaque; an owner-attended decode is required.",
    );
  }
  return yield* finalizeDecoded(
    "electron_safe_storage_record",
    "desktop-secure-storage-v1",
    sourcePathLabel,
    utf8Text(base64Bytes(envelope.valueBase64)),
  );
});

/**
 * 5. Decode an encrypted Pylon backup (scrypt key + XChaCha20-Poly1305). Discovery
 * inspects the PUBLIC manifest first; the payload decode is this authorized
 * action. A manifest that reports no included identity mnemonic, or a call with no
 * payload, returns `owner_attended_required` rather than a decode.
 */
export const decodeEncryptedPylonBackup = Effect.fn("SovereignIdentity.decodeEncryptedPylonBackup")(
  function* (input: EncryptedPylonBackupInput) {
    const manifest = yield* decodeEnvelope(
      "encrypted_pylon_backup",
      PylonBackupManifest,
      input.manifest,
    );
    const sourcePathLabel = input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.encrypted_pylon_backup;

    if (!manifest.identity_mnemonic_included) {
      return ownerAttended(
        "encrypted_pylon_backup",
        "pylon-wallet-backup-v1",
        sourcePathLabel,
        "The backup manifest reports identity_mnemonic_included=false; this backup carries no shared-root phrase.",
      );
    }
    if (input.payload === undefined) {
      return ownerAttended(
        "encrypted_pylon_backup",
        "pylon-wallet-backup-v1",
        sourcePathLabel,
        "Only the public manifest is present; supply the encrypted payload for an authorized decode.",
      );
    }
    if (input.password === undefined || input.password.length === 0) {
      return yield* fail("encrypted_pylon_backup", "missing_password");
    }

    const payload = yield* decodeEnvelope(
      "encrypted_pylon_backup",
      PylonBackupPayload,
      input.payload,
    );
    const key = deriveScryptKey(
      utf8Bytes(input.password),
      base64Bytes(payload.saltB64),
      manifest.scrypt,
    );
    const plaintext = yield* Effect.try({
      try: () =>
        xchacha20Poly1305Open(
          key,
          base64Bytes(payload.nonceB64),
          base64Bytes(payload.ciphertextB64),
        ),
      catch: () =>
        new DecodeCandidateError({ format: "encrypted_pylon_backup", reason: "decrypt_failed" }),
    });

    const decodedPayload = yield* Effect.try({
      try: () => JSON.parse(utf8Text(plaintext)) as unknown,
      catch: () =>
        new DecodeCandidateError({ format: "encrypted_pylon_backup", reason: "decrypt_failed" }),
    });
    const phrase =
      typeof decodedPayload === "object" &&
      decodedPayload !== null &&
      typeof (decodedPayload as { identity_mnemonic?: unknown }).identity_mnemonic === "string"
        ? (decodedPayload as { identity_mnemonic: string }).identity_mnemonic
        : null;
    if (phrase === null) return yield* fail("encrypted_pylon_backup", "malformed_envelope");

    return yield* finalizeDecoded(
      "encrypted_pylon_backup",
      "pylon-wallet-backup-v1",
      sourcePathLabel,
      phrase,
    );
  },
);

/**
 * 6. Decode a sovereign agent TOML candidate. The audit records that, despite the
 * `mnemonic_encrypted` field name, the field held the raw phrase. Only after the
 * format is selected does this read the one bounded field with a deterministic
 * pattern.
 */
export const decodeSovereignAgentToml = Effect.fn("SovereignIdentity.decodeSovereignAgentToml")(
  function* (input: SovereignAgentTomlInput) {
    const match = input.tomlText.match(/^\s*mnemonic_encrypted\s*=\s*"([^"]+)"\s*$/mu);
    if (match === null) return yield* fail("sovereign_agent_toml", "malformed_envelope");
    return yield* finalizeDecoded(
      "sovereign_agent_toml",
      "sovereign-agent-toml-v1",
      input.sourcePathLabel ?? DEFAULT_SOURCE_LABEL.sovereign_agent_toml,
      match[1],
    );
  },
);
