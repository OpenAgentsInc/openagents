import { createCipheriv } from "node:crypto";
import { hexToBytes } from "@noble/hashes/utils";
import { describe, expect, test } from "vite-plus/test";
import {
  aes256GcmOpen,
  aes256GcmSeal,
  chacha20Poly1305Open,
  chacha20Poly1305Seal,
  hchacha20,
  xchacha20Poly1305Open,
  xchacha20Poly1305Seal,
} from "./legacy-crypto.ts";

const bytesToHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

/**
 * The XChaCha20-Poly1305 construction is not a Node built-in, so it is the one
 * hand-rolled primitive. It is validated against the authoritative
 * draft-irtf-cfrg-xchacha20poly1305-03 AEAD test vector, which exercises the
 * HChaCha20 subkey, the 12-byte inner nonce, and the Poly1305 tag end to end.
 */
describe("XChaCha20-Poly1305 matches the RFC-draft AEAD test vector", () => {
  test("HChaCha20 + inner ChaCha20-Poly1305 reproduces the vector ciphertext and tag", () => {
    const key = hexToBytes("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
    const nonce24 = hexToBytes("404142434445464748494a4b4c4d4e4f5051525354555657");
    const aad = hexToBytes("50515253c0c1c2c3c4c5c6c7");
    const plaintext = new TextEncoder().encode(
      "Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.",
    );

    // Reproduce the XChaCha construction inline with AAD to hit the vector.
    const subkey = hchacha20(key, nonce24.subarray(0, 16));
    const innerNonce = new Uint8Array(12);
    innerNonce.set(nonce24.subarray(16, 24), 4);
    const cipher = createCipheriv(
      "chacha20-poly1305",
      Buffer.from(subkey),
      Buffer.from(innerNonce),
      {
        authTagLength: 16,
      },
    );
    cipher.setAAD(Buffer.from(aad), { plaintextLength: plaintext.length });
    const ciphertext = Buffer.concat([cipher.update(Buffer.from(plaintext)), cipher.final()]);
    const tag = cipher.getAuthTag();

    expect(bytesToHex(new Uint8Array(ciphertext))).toBe(
      "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff921f9664c97637da9768812f615c68b13b52e",
    );
    expect(bytesToHex(new Uint8Array(tag))).toBe("c0875924c1c7987947deafd8780acf49");
  });
});

describe("legacy AEAD round trips", () => {
  const key = hexToBytes("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
  const plaintext = new TextEncoder().encode("round-trip plaintext, not a secret");

  test("AES-256-GCM seals and opens", () => {
    const nonce = new Uint8Array(12).fill(7);
    const sealed = aes256GcmSeal(key, nonce, plaintext);
    expect(new TextDecoder().decode(aes256GcmOpen(key, nonce, sealed))).toBe(
      "round-trip plaintext, not a secret",
    );
  });

  test("ChaCha20-Poly1305 seals and opens", () => {
    const nonce = new Uint8Array(12).fill(9);
    const sealed = chacha20Poly1305Seal(key, nonce, plaintext);
    expect(new TextDecoder().decode(chacha20Poly1305Open(key, nonce, sealed))).toBe(
      "round-trip plaintext, not a secret",
    );
  });

  test("XChaCha20-Poly1305 seals and opens", () => {
    const nonce = new Uint8Array(24).fill(5);
    const sealed = xchacha20Poly1305Seal(key, nonce, plaintext);
    expect(new TextDecoder().decode(xchacha20Poly1305Open(key, nonce, sealed))).toBe(
      "round-trip plaintext, not a secret",
    );
  });

  test("a tampered ciphertext fails the auth check", () => {
    const nonce = new Uint8Array(12).fill(3);
    const sealed = aes256GcmSeal(key, nonce, plaintext);
    sealed[0] ^= 0xff;
    expect(() => aes256GcmOpen(key, nonce, sealed)).toThrow();
  });
});
