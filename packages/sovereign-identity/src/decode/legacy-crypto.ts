/**
 * IDR-03 legacy crypto primitives.
 *
 * This module reproduces the exact key-derivation and authenticated-cipher
 * layouts the historical OpenAgents secret stores used, so the IDR-03 decoders
 * can restore a real fixture. It uses only Node built-in crypto and the audited
 * `@noble/hashes` KDFs already pinned by this package. It never touches the
 * filesystem, the network, or a platform key store.
 *
 * Formats covered:
 *
 * - Compute `identity.enc`: Argon2id key + AES-256-GCM (audit "Compute secure
 *   storage").
 * - Wallet keyring envelope: Argon2id key + ChaCha20-Poly1305 (audit "Possible
 *   imported-root candidates").
 * - Encrypted Pylon backup: scrypt key + XChaCha20-Poly1305 (audit "Encrypted
 *   backup files").
 *
 * XChaCha20-Poly1305 is not a Node built-in, so this module builds it from the
 * standard construction: an HChaCha20 subkey over the first 16 nonce bytes plus
 * Node's `chacha20-poly1305` over a 12-byte nonce `00000000 || nonce[16..24]`.
 * The HChaCha20 core is validated against the draft-irtf-cfrg-xchacha20poly1305
 * test vector by `legacy-crypto.test.ts`.
 *
 * Source of truth:
 * `docs/sol/2026-07-20-pylon-bip39-nostr-spark-identity-recovery-audit.md`.
 */
import { createCipheriv, createDecipheriv } from "node:crypto";
import { argon2id } from "@noble/hashes/argon2";
import { scrypt } from "@noble/hashes/scrypt";

/** The Argon2id parameters a historical envelope stores alongside its ciphertext. */
export interface Argon2idParams {
  /** Memory cost in KiB. */
  readonly memoryKib: number;
  /** Time cost (iterations / passes). */
  readonly iterations: number;
  /** Parallelism (lanes). */
  readonly parallelism: number;
}

/** The scrypt parameters a historical backup stores alongside its ciphertext. */
export interface ScryptParams {
  /** CPU/memory cost `N` (a power of two). */
  readonly N: number;
  /** Block size `r`. */
  readonly r: number;
  /** Parallelization `p`. */
  readonly p: number;
}

/** The AEAD tag length every legacy format uses, in bytes. */
export const AEAD_TAG_LENGTH = 16;

/** The derived symmetric key length every legacy format uses, in bytes. */
export const DERIVED_KEY_LENGTH = 32;

/** Derive a 32-byte key with Argon2id, exactly as the legacy envelopes did. */
export function deriveArgon2idKey(
  password: Uint8Array,
  salt: Uint8Array,
  params: Argon2idParams,
): Uint8Array {
  return argon2id(password, salt, {
    t: params.iterations,
    m: params.memoryKib,
    p: params.parallelism,
    dkLen: DERIVED_KEY_LENGTH,
  });
}

/** Derive a 32-byte key with scrypt, exactly as the legacy backup did. */
export function deriveScryptKey(
  password: Uint8Array,
  salt: Uint8Array,
  params: ScryptParams,
): Uint8Array {
  return scrypt(password, salt, {
    N: params.N,
    r: params.r,
    p: params.p,
    dkLen: DERIVED_KEY_LENGTH,
  });
}

// ---------------------------------------------------------------------------
// AES-256-GCM (Compute identity.enc)
// ---------------------------------------------------------------------------

/** AES-256-GCM seal. It returns `ciphertext || tag`, the layout the store persisted. */
export function aes256GcmSeal(
  key: Uint8Array,
  nonce: Uint8Array,
  plaintext: Uint8Array,
): Uint8Array {
  const cipher = createCipheriv("aes-256-gcm", key, nonce);
  const body = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  return Buffer.concat([body, cipher.getAuthTag()]);
}

/** AES-256-GCM open of a `ciphertext || tag` buffer. It throws on an auth failure. */
export function aes256GcmOpen(
  key: Uint8Array,
  nonce: Uint8Array,
  ciphertextAndTag: Uint8Array,
): Uint8Array {
  const split = ciphertextAndTag.length - AEAD_TAG_LENGTH;
  const body = ciphertextAndTag.subarray(0, split);
  const tag = ciphertextAndTag.subarray(split);
  const decipher = createDecipheriv("aes-256-gcm", key, nonce);
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(body), decipher.final()]);
}

// ---------------------------------------------------------------------------
// ChaCha20-Poly1305 (Wallet keyring envelope)
// ---------------------------------------------------------------------------

/** IETF ChaCha20-Poly1305 seal (12-byte nonce). It returns `ciphertext || tag`. */
export function chacha20Poly1305Seal(
  key: Uint8Array,
  nonce: Uint8Array,
  plaintext: Uint8Array,
): Uint8Array {
  const cipher = createCipheriv("chacha20-poly1305", key, nonce, {
    authTagLength: AEAD_TAG_LENGTH,
  });
  const body = Buffer.concat([cipher.update(plaintext), cipher.final()]);
  return Buffer.concat([body, cipher.getAuthTag()]);
}

/** IETF ChaCha20-Poly1305 open of a `ciphertext || tag` buffer. It throws on an auth failure. */
export function chacha20Poly1305Open(
  key: Uint8Array,
  nonce: Uint8Array,
  ciphertextAndTag: Uint8Array,
): Uint8Array {
  const split = ciphertextAndTag.length - AEAD_TAG_LENGTH;
  const body = ciphertextAndTag.subarray(0, split);
  const tag = ciphertextAndTag.subarray(split);
  const decipher = createDecipheriv("chacha20-poly1305", key, nonce, {
    authTagLength: AEAD_TAG_LENGTH,
  });
  decipher.setAuthTag(tag);
  return Buffer.concat([decipher.update(body), decipher.final()]);
}

// ---------------------------------------------------------------------------
// XChaCha20-Poly1305 (Encrypted Pylon backup) via HChaCha20 + Node ChaCha20-Poly1305
// ---------------------------------------------------------------------------

const CHACHA_SIGMA = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574] as const;

const rotl32 = (x: number, n: number): number => ((x << n) | (x >>> (32 - n))) >>> 0;

const readLe32 = (bytes: Uint8Array, offset: number): number =>
  (bytes[offset] |
    (bytes[offset + 1] << 8) |
    (bytes[offset + 2] << 16) |
    (bytes[offset + 3] << 24)) >>>
  0;

const quarterRound = (state: Uint32Array, a: number, b: number, c: number, d: number): void => {
  state[a] = (state[a] + state[b]) >>> 0;
  state[d] = rotl32(state[d] ^ state[a], 16);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotl32(state[b] ^ state[c], 12);
  state[a] = (state[a] + state[b]) >>> 0;
  state[d] = rotl32(state[d] ^ state[a], 8);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotl32(state[b] ^ state[c], 7);
};

/**
 * HChaCha20 subkey derivation over a 32-byte key and a 16-byte nonce. It returns
 * the 32-byte subkey (state words 0..3 and 12..15) with NO feed-forward add, per
 * the XChaCha construction. `legacy-crypto.test.ts` asserts the RFC test vector.
 */
export function hchacha20(key: Uint8Array, nonce16: Uint8Array): Uint8Array {
  const state = new Uint32Array(16);
  state[0] = CHACHA_SIGMA[0];
  state[1] = CHACHA_SIGMA[1];
  state[2] = CHACHA_SIGMA[2];
  state[3] = CHACHA_SIGMA[3];
  for (let i = 0; i < 8; i += 1) state[4 + i] = readLe32(key, i * 4);
  for (let i = 0; i < 4; i += 1) state[12 + i] = readLe32(nonce16, i * 4);

  for (let round = 0; round < 10; round += 1) {
    quarterRound(state, 0, 4, 8, 12);
    quarterRound(state, 1, 5, 9, 13);
    quarterRound(state, 2, 6, 10, 14);
    quarterRound(state, 3, 7, 11, 15);
    quarterRound(state, 0, 5, 10, 15);
    quarterRound(state, 1, 6, 11, 12);
    quarterRound(state, 2, 7, 8, 13);
    quarterRound(state, 3, 4, 9, 14);
  }

  const out = new Uint8Array(32);
  const words = [
    state[0],
    state[1],
    state[2],
    state[3],
    state[12],
    state[13],
    state[14],
    state[15],
  ];
  for (let i = 0; i < 8; i += 1) {
    const word = words[i];
    out[i * 4] = word & 0xff;
    out[i * 4 + 1] = (word >>> 8) & 0xff;
    out[i * 4 + 2] = (word >>> 16) & 0xff;
    out[i * 4 + 3] = (word >>> 24) & 0xff;
  }
  return out;
}

/** Build the (subkey, 12-byte inner nonce) pair a 24-byte XChaCha nonce implies. */
function xchachaInner(
  key: Uint8Array,
  nonce24: Uint8Array,
): {
  readonly subkey: Uint8Array;
  readonly innerNonce: Uint8Array;
} {
  const subkey = hchacha20(key, nonce24.subarray(0, 16));
  const innerNonce = new Uint8Array(12);
  innerNonce.set(nonce24.subarray(16, 24), 4);
  return { subkey, innerNonce };
}

/** XChaCha20-Poly1305 seal (24-byte nonce). It returns `ciphertext || tag`. */
export function xchacha20Poly1305Seal(
  key: Uint8Array,
  nonce24: Uint8Array,
  plaintext: Uint8Array,
): Uint8Array {
  const { subkey, innerNonce } = xchachaInner(key, nonce24);
  return chacha20Poly1305Seal(subkey, innerNonce, plaintext);
}

/** XChaCha20-Poly1305 open (24-byte nonce). It throws on an auth failure. */
export function xchacha20Poly1305Open(
  key: Uint8Array,
  nonce24: Uint8Array,
  ciphertextAndTag: Uint8Array,
): Uint8Array {
  const { subkey, innerNonce } = xchachaInner(key, nonce24);
  return chacha20Poly1305Open(subkey, innerNonce, ciphertextAndTag);
}
