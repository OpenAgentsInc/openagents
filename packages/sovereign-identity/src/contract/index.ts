/**
 * IDR-00 frozen sovereign-identity contract.
 *
 * This module re-exports the frozen derivation profile, secret-store
 * identifiers, Effect Schema contract, public test vectors, and
 * historical-format fixtures. It defines no runtime behavior beyond the pure,
 * deterministic reference derivation used to produce and verify the vectors.
 */
export * from "./derivation.ts";
export * from "./secret-store.ts";
export * from "./schemas.ts";
export * from "./vectors.ts";
export * from "./fixtures.ts";
