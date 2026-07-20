/**
 * `@openagentsinc/sovereign-identity` — the neutral root package for the
 * OpenAgents local sovereign identity (one BIP-39 mnemonic that produces a
 * Nostr identity key and a Spark wallet key).
 *
 * IDR-00 delivers the FROZEN contract: derivation profiles, public test
 * vectors, secret/manifest/receipt schemas, and historical-format fixtures.
 *
 * IDR-01 adds the machinery on top of the frozen contract: the signer boundary
 * ports, the recovery state machine (types and transitions only), the public
 * manifest writer contract and migration receipt production, and the
 * `SovereignIdentity` service composed over the injected `LocalSecretStore` and
 * `ManifestStore` ports. The fail-closed open and create operations arrive in
 * IDR-02.
 *
 * This package imports no Pylon, Desktop, React, Electron, or wallet SDK. It may
 * import the neutral `@openagentsinc/local-secret-store` port. See
 * `src/boundary.test.ts`.
 */
export * from "./contract/index.ts";
export * from "./machinery/index.ts";
