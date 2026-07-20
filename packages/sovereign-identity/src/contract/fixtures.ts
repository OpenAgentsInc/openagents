/**
 * IDR-00 historical-format fixtures.
 *
 * One fixture per historical secret format the audit lists. Every fixture is
 * derived from the PUBLIC TEST mnemonic (see `vectors.ts`); none is a real
 * secret. Each fixture carries the PUBLIC identity it should yield, so the later
 * decode packet (IDR-03) and reconcile packet (IDR-04) have an exact target.
 *
 * A fixture is one of two kinds:
 *
 * - `decodable_now`: the envelope carries the mnemonic in a form this repository
 *   can already decode deterministically (plain text, base64, TOML field). The
 *   test extracts the mnemonic and derives the expected identity.
 * - `envelope_shape_only`: the real byte layout needs a legacy cipher/KDF that
 *   IDR-03 owns. The fixture freezes the ENVELOPE SHAPE plus the expected public
 *   identity as the decode target. `idr03Gap` names the missing work.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { normalizeMnemonic } from "./derivation.ts";
import {
  PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE,
  PUBLIC_TEST_MNEMONIC,
  PUBLIC_TEST_MNEMONIC_BASE64,
  type FrozenPublicIdentityVector,
} from "./vectors.ts";

export const HISTORICAL_FORMAT_IDS = [
  "plain_mnemonic_file",
  "compute_identity_enc",
  "wallet_keyring_envelope",
  "electron_safe_storage_record",
  "encrypted_pylon_backup",
  "sovereign_agent_toml",
] as const;

export type HistoricalFormatId = (typeof HISTORICAL_FORMAT_IDS)[number];

export type FixtureDecodability = "decodable_now" | "envelope_shape_only";

export interface HistoricalFormatFixture {
  /** The historical format identifier. */
  readonly format: HistoricalFormatId;
  /** A short public description of the source. */
  readonly label: string;
  /** Whether this repository can decode the envelope today. */
  readonly decodability: FixtureDecodability;
  /** A PUBLIC path label for the source. Never a real private path. */
  readonly sourcePathLabel: string;
  /** The frozen format version string. */
  readonly formatVersion: string;
  /**
   * The fixture envelope. It is test-mnemonic-derived and NOT a real secret. For
   * an `envelope_shape_only` fixture, cipher fields are shape placeholders and
   * do not decode to the mnemonic.
   */
  readonly envelope: Record<string, unknown>;
  /** The public identity this fixture must yield after decode and derivation. */
  readonly expectedPublicIdentity: FrozenPublicIdentityVector;
  /** The decode work left for IDR-03, when the exact bytes are not frozen here. */
  readonly idr03Gap?: string;
}

const EXPECTED = PUBLIC_TEST_IDENTITY_EMPTY_PASSPHRASE;

/** 1. Plain BIP-39 mnemonic file (`identity.mnemonic`). */
export const plainMnemonicFileFixture: HistoricalFormatFixture = {
  format: "plain_mnemonic_file",
  label: "Plain BIP-39 mnemonic file",
  decodability: "decodable_now",
  sourcePathLabel: "~/.openagents/pylon/identity.mnemonic",
  formatVersion: "plain-text-v1",
  envelope: {
    // The file stored the raw phrase plus a trailing newline.
    contentUtf8: `${PUBLIC_TEST_MNEMONIC}\n`,
  },
  expectedPublicIdentity: EXPECTED,
};

/** 2. Compute encrypted seed store (`identity.enc`), AES-256-GCM + Argon2. */
export const computeIdentityEncFixture: HistoricalFormatFixture = {
  format: "compute_identity_enc",
  label: "Compute identity.enc (AES-256-GCM, Argon2-derived key)",
  decodability: "envelope_shape_only",
  sourcePathLabel: "<config_dir>/openagents/compute/identity.enc",
  formatVersion: "compute-identity-enc-v1",
  envelope: {
    version: 1,
    kdf: "argon2id",
    argon2: { memoryKib: 65536, iterations: 3, parallelism: 1 },
    cipher: "aes-256-gcm",
    saltB64: "PLACEHOLDER_SALT_NOT_REAL",
    nonceB64: "PLACEHOLDER_NONCE_NOT_REAL",
    ciphertextB64: "PLACEHOLDER_CIPHERTEXT_NOT_REAL",
    placeholder: true,
  },
  expectedPublicIdentity: EXPECTED,
  idr03Gap:
    "IDR-03 must reproduce the exact Argon2 parameters and AES-256-GCM layout to decode a real identity.enc; the frozen ciphertext here is a shape placeholder.",
};

/** 3. Wallet keyring envelope (Argon2 + ChaCha20-Poly1305). */
export const walletKeyringEnvelopeFixture: HistoricalFormatFixture = {
  format: "wallet_keyring_envelope",
  label: "Wallet keyring envelope (Argon2, ChaCha20-Poly1305)",
  decodability: "envelope_shape_only",
  sourcePathLabel: "OS keyring service openagents-wallet, account mnemonic",
  formatVersion: "wallet-keyring-envelope-v1",
  envelope: {
    version: 1,
    keyringService: "openagents-wallet",
    keyringAccount: "mnemonic",
    kdf: "argon2id",
    argon2: { memoryKib: 65536, iterations: 3, parallelism: 1 },
    cipher: "chacha20poly1305",
    saltB64: "PLACEHOLDER_SALT_NOT_REAL",
    nonceB64: "PLACEHOLDER_NONCE_NOT_REAL",
    ciphertextB64: "PLACEHOLDER_CIPHERTEXT_NOT_REAL",
    placeholder: true,
  },
  expectedPublicIdentity: EXPECTED,
  idr03Gap:
    "IDR-03 must reproduce the Argon2 + ChaCha20-Poly1305 keyring envelope; a raw-phrase keyring variant also exists. This entry is unproven until Nostr and Spark match (audit Phase 1/2).",
};

/**
 * 4. Electron desktop safe-storage record (`desktop-secure-storage.json`).
 *
 * The normal record holds OS `safeStorage` ciphertext in base64, which is
 * platform-opaque and owner-attended. The insecure development flag
 * (`OA_DESKTOP_ALLOW_INSECURE_SECRET_STORAGE=1`) stored raw UTF-8 in base64.
 * This fixture freezes the DECODABLE insecure-mode variant plus the opaque
 * shape.
 */
export const electronSafeStorageRecordFixture: HistoricalFormatFixture = {
  format: "electron_safe_storage_record",
  label: "Electron desktop safe-storage record (spark.wallet.mnemonic)",
  decodability: "decodable_now",
  sourcePathLabel: "<Electron-userData>/secure/desktop-secure-storage.json",
  formatVersion: "desktop-secure-storage-v1",
  envelope: {
    schemaVersion: 1,
    key: "spark.wallet.mnemonic",
    encoding: "base64",
    // Insecure development mode: base64 of the raw UTF-8 phrase.
    insecureMode: true,
    valueBase64: PUBLIC_TEST_MNEMONIC_BASE64,
    // The normal mode instead holds opaque OS safeStorage ciphertext here.
    normalModeShape: { encoding: "base64", value: "OS_SAFE_STORAGE_CIPHERTEXT_OPAQUE" },
  },
  expectedPublicIdentity: EXPECTED,
  idr03Gap:
    "IDR-03/IDR-05 must decode the normal OS safeStorage ciphertext in an owner-attended run; only the insecure-mode base64 variant is decodable offline.",
};

/**
 * 5. Encrypted portable Pylon backup (XChaCha20-Poly1305 + scrypt).
 *
 * A discovery tool inspects only the PUBLIC manifest first. This fixture freezes
 * that public manifest, with `identity_mnemonic_included` true. The encrypted
 * payload decode is an authorized IDR-03 action.
 */
export const encryptedPylonBackupFixture: HistoricalFormatFixture = {
  format: "encrypted_pylon_backup",
  label: "Encrypted Pylon wallet backup (public manifest)",
  decodability: "envelope_shape_only",
  sourcePathLabel: "user-selected wallet backup export path",
  formatVersion: "pylon-wallet-backup-v1",
  envelope: {
    manifestVersion: 1,
    cipher: "xchacha20poly1305",
    kdf: "scrypt",
    identity_mnemonic_included: true,
    createdAtLabel: "backup-manifest-timestamp",
    // The encrypted payload is not present in a public-safe fixture.
    encryptedPayloadPresent: false,
    placeholder: true,
  },
  expectedPublicIdentity: EXPECTED,
  idr03Gap:
    "IDR-03 must reproduce the scrypt + XChaCha20-Poly1305 backup payload decode; discovery reads only this public manifest until an authorized recovery action.",
};

/** 6. Sovereign agent TOML (`<npub>.toml`), raw phrase in `mnemonic_encrypted`. */
export const sovereignAgentTomlFixture: HistoricalFormatFixture = {
  format: "sovereign_agent_toml",
  label: "Sovereign agent TOML (mnemonic_encrypted holds the raw phrase)",
  decodability: "decodable_now",
  sourcePathLabel: "<config_dir>/openagents/agents/<npub>.toml",
  formatVersion: "sovereign-agent-toml-v1",
  envelope: {
    // Despite the field name, the audit records that this field held the raw
    // phrase. IDR-03 reads the field and normalizes it.
    tomlField: "mnemonic_encrypted",
    tomlText: [
      `npub = "${EXPECTED.npub}"`,
      `mnemonic_encrypted = "${PUBLIC_TEST_MNEMONIC}"`,
      "",
    ].join("\n"),
  },
  expectedPublicIdentity: EXPECTED,
};

/**
 * Extract the mnemonic from a `decodable_now` fixture using only deterministic,
 * offline decoding (plain text, base64, or a TOML field). It returns `null` for
 * an `envelope_shape_only` fixture, whose decode belongs to IDR-03.
 */
export function extractDecodableMnemonic(fixture: HistoricalFormatFixture): string | null {
  if (fixture.decodability !== "decodable_now") return null;
  switch (fixture.format) {
    case "plain_mnemonic_file":
      return normalizeMnemonic(String(fixture.envelope.contentUtf8));
    case "electron_safe_storage_record":
      return normalizeMnemonic(atob(String(fixture.envelope.valueBase64)));
    case "sovereign_agent_toml": {
      const match = String(fixture.envelope.tomlText).match(/mnemonic_encrypted\s*=\s*"([^"]+)"/);
      return match ? normalizeMnemonic(match[1]) : null;
    }
    default:
      return null;
  }
}

/** Every historical-format fixture, in the frozen order. */
export const HISTORICAL_FORMAT_FIXTURES: ReadonlyArray<HistoricalFormatFixture> = [
  plainMnemonicFileFixture,
  computeIdentityEncFixture,
  walletKeyringEnvelopeFixture,
  electronSafeStorageRecordFixture,
  encryptedPylonBackupFixture,
  sovereignAgentTomlFixture,
];
