/**
 * IDR-03 historical secret-store decoders.
 *
 * This barrel exposes the decode surface on top of the IDR-00 frozen contract
 * and the IDR-02 discovery machinery: the bounded recovered-secret boundary, the
 * public-safe decode result and typed error, the per-format decoders, the exact
 * legacy KDF + AEAD primitives, and the typed `decodeCandidate` seam.
 *
 * It deliberately does NOT re-export `./fixtures.ts`. Those fixtures seal
 * ciphertext at module load, so a consumer imports them explicitly rather than
 * pulling that work into every import of the package.
 */
export * from "./boundary.ts";
export * from "./result.ts";
export * from "./legacy-crypto.ts";
export * from "./formats.ts";
export * from "./decode-candidate.ts";
