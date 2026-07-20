/**
 * IDR-03 decode fixtures.
 *
 * One `decodeCandidate` input per historical format, all derived from the PUBLIC
 * TEST mnemonic (see `../contract/vectors.ts`). None is a real secret. The three
 * encrypted formats (Compute `identity.enc`, Wallet keyring, Pylon backup) are
 * SEALED here with the exact legacy KDF + AEAD layout from `legacy-crypto.ts`, so
 * the matching decoder restores them by a real round trip. The KDF cost is
 * reduced for a fast test and stored IN the envelope, exactly as a real store
 * persisted its parameters, so the decoder reads them faithfully.
 *
 * Building the sealed bytes in memory writes NO file. The fixtures never read,
 * copy, or move a real secret store.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import {
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
  PUBLIC_TEST_MNEMONIC_BASE64,
} from "../contract/index.ts";
import type { DecodeCandidateInput } from "./decode-candidate.ts";
import {
  aes256GcmSeal,
  type Argon2idParams,
  chacha20Poly1305Seal,
  deriveArgon2idKey,
  deriveScryptKey,
  type ScryptParams,
  xchacha20Poly1305Seal,
} from "./legacy-crypto.ts";

/** The public fixture password. It is NOT a real secret and never a user password. */
export const FIXTURE_PASSWORD = "idr03-fixture-password-not-a-real-secret";

/** The one public identity every decodable fixture must yield after derivation. */
export const FIXTURE_EXPECTED_IDENTITY = PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE;

/** Reduced Argon2id cost for a fast test. Stored in the envelope, read by the decoder. */
const FIXTURE_ARGON2: Argon2idParams = { memoryKib: 8192, iterations: 2, parallelism: 1 };

/** Reduced scrypt cost for a fast test. Stored in the manifest, read by the decoder. */
const FIXTURE_SCRYPT: ScryptParams = { N: 16384, r: 8, p: 1 };

const utf8 = (value: string): Uint8Array => new TextEncoder().encode(value);
const toB64 = (bytes: Uint8Array): string => Buffer.from(bytes).toString("base64");

/** Deterministic non-secret bytes for a fixture salt or nonce. */
const fixtureBytes = (length: number, seed: number): Uint8Array =>
  Uint8Array.from({ length }, (_unused, index) => (seed + index * 31) & 0xff);

// ---------------------------------------------------------------------------
// 1. Plain BIP-39 mnemonic file
// ---------------------------------------------------------------------------

export const plainMnemonicFixtureInput: DecodeCandidateInput = {
  format: "plain_mnemonic_file",
  contentUtf8: `${PUBLIC_TEST_MNEMONIC}\n`,
};

// ---------------------------------------------------------------------------
// 2. Compute identity.enc (Argon2id + AES-256-GCM)
// ---------------------------------------------------------------------------

const computeSalt = fixtureBytes(16, 0x11);
const computeNonce = fixtureBytes(12, 0x22);
const computeCiphertext = aes256GcmSeal(
  deriveArgon2idKey(utf8(FIXTURE_PASSWORD), computeSalt, FIXTURE_ARGON2),
  computeNonce,
  utf8(PUBLIC_TEST_MNEMONIC),
);

/** The Compute `identity.enc` envelope object, exported for negative decode tests. */
export const computeIdentityEncEnvelopeFixture = {
  version: 1,
  kdf: "argon2id",
  argon2: FIXTURE_ARGON2,
  cipher: "aes-256-gcm",
  saltB64: toB64(computeSalt),
  nonceB64: toB64(computeNonce),
  ciphertextB64: toB64(computeCiphertext),
} as const;

export const computeIdentityEncFixtureInput: DecodeCandidateInput = {
  format: "compute_identity_enc",
  password: FIXTURE_PASSWORD,
  envelope: computeIdentityEncEnvelopeFixture,
};

// ---------------------------------------------------------------------------
// 3. Wallet keyring envelope (Argon2id + ChaCha20-Poly1305)
// ---------------------------------------------------------------------------

const keyringSalt = fixtureBytes(16, 0x33);
const keyringNonce = fixtureBytes(12, 0x44);
const keyringCiphertext = chacha20Poly1305Seal(
  deriveArgon2idKey(utf8(FIXTURE_PASSWORD), keyringSalt, FIXTURE_ARGON2),
  keyringNonce,
  utf8(PUBLIC_TEST_MNEMONIC),
);

export const walletKeyringFixtureInput: DecodeCandidateInput = {
  format: "wallet_keyring_envelope",
  password: FIXTURE_PASSWORD,
  envelope: {
    version: 1,
    keyringService: "openagents-wallet",
    keyringAccount: "mnemonic",
    kdf: "argon2id",
    argon2: FIXTURE_ARGON2,
    cipher: "chacha20poly1305",
    saltB64: toB64(keyringSalt),
    nonceB64: toB64(keyringNonce),
    ciphertextB64: toB64(keyringCiphertext),
  },
};

// ---------------------------------------------------------------------------
// 4. Electron desktop safe-storage record
// ---------------------------------------------------------------------------

/** Insecure development mode: base64 of the raw UTF-8 phrase. It decodes offline. */
export const electronInsecureFixtureInput: DecodeCandidateInput = {
  format: "electron_safe_storage_record",
  envelope: {
    schemaVersion: 1,
    key: "spark.wallet.mnemonic",
    encoding: "base64",
    insecureMode: true,
    valueBase64: PUBLIC_TEST_MNEMONIC_BASE64,
  },
};

/** Normal mode: platform-opaque OS safeStorage ciphertext. It needs an owner-attended run. */
export const electronOpaqueFixtureInput: DecodeCandidateInput = {
  format: "electron_safe_storage_record",
  envelope: {
    schemaVersion: 1,
    key: "spark.wallet.mnemonic",
    encoding: "base64",
    insecureMode: false,
    valueBase64: toB64(utf8("OS_SAFE_STORAGE_CIPHERTEXT_OPAQUE")),
  },
};

// ---------------------------------------------------------------------------
// 5. Encrypted Pylon backup (scrypt + XChaCha20-Poly1305)
// ---------------------------------------------------------------------------

const backupSalt = fixtureBytes(16, 0x55);
const backupNonce = fixtureBytes(24, 0x66);
const backupPlaintext = JSON.stringify({
  identity_mnemonic: PUBLIC_TEST_MNEMONIC,
  wallet: { note: "public-safe fixture wallet state" },
});
const backupCiphertext = xchacha20Poly1305Seal(
  deriveScryptKey(utf8(FIXTURE_PASSWORD), backupSalt, FIXTURE_SCRYPT),
  backupNonce,
  utf8(backupPlaintext),
);

const pylonBackupManifest = {
  manifestVersion: 1,
  cipher: "xchacha20poly1305",
  kdf: "scrypt",
  scrypt: FIXTURE_SCRYPT,
  identity_mnemonic_included: true,
} as const;

export const encryptedPylonBackupFixtureInput: DecodeCandidateInput = {
  format: "encrypted_pylon_backup",
  password: FIXTURE_PASSWORD,
  manifest: pylonBackupManifest,
  payload: {
    saltB64: toB64(backupSalt),
    nonceB64: toB64(backupNonce),
    ciphertextB64: toB64(backupCiphertext),
  },
};

/** The public manifest alone, with no payload: discovery reads only this. */
export const encryptedPylonBackupManifestOnlyInput: DecodeCandidateInput = {
  format: "encrypted_pylon_backup",
  manifest: pylonBackupManifest,
};

// ---------------------------------------------------------------------------
// 6. Sovereign agent TOML
// ---------------------------------------------------------------------------

export const sovereignAgentTomlFixtureInput: DecodeCandidateInput = {
  format: "sovereign_agent_toml",
  tomlText: [
    `npub = "${PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE.npub}"`,
    `mnemonic_encrypted = "${PUBLIC_TEST_MNEMONIC}"`,
    "",
  ].join("\n"),
};

/**
 * One decodable fixture input per historical format, in the frozen order. Each
 * yields the one public identity `FIXTURE_EXPECTED_IDENTITY` after derivation.
 * The Electron entry is the offline-decodable insecure-mode variant.
 */
export const DECODE_FIXTURE_INPUTS: ReadonlyArray<DecodeCandidateInput> = [
  plainMnemonicFixtureInput,
  computeIdentityEncFixtureInput,
  walletKeyringFixtureInput,
  electronInsecureFixtureInput,
  encryptedPylonBackupFixtureInput,
  sovereignAgentTomlFixtureInput,
];
