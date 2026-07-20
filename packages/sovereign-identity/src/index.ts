/**
 * `@openagentsinc/sovereign-identity` — the neutral root package for the
 * OpenAgents local sovereign identity (one BIP-39 mnemonic that produces a
 * Nostr identity key and a Spark wallet key).
 *
 * IDR-00 delivers the FROZEN contract only: derivation profiles, public test
 * vectors, secret/manifest/receipt schemas, and historical-format fixtures. The
 * signer ports, secret-store adapters, recovery state machine, and migration
 * runtime arrive in later IDR packets.
 *
 * This package imports no Pylon, Desktop, React, Electron, or wallet SDK. See
 * `src/boundary.test.ts`.
 */
export * from "./contract/index.ts";
