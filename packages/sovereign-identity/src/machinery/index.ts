/**
 * IDR-01 sovereign-identity machinery.
 *
 * This barrel adds the runtime machinery on top of the IDR-00 frozen contract:
 * the signer boundary ports, the recovery state machine (types and transitions
 * only), the public manifest writer contract and migration receipt production,
 * and the `SovereignIdentity` service composed over the injected
 * `LocalSecretStore` and `ManifestStore` ports.
 *
 * The fail-closed open and create operations arrive in IDR-02. The narrowed
 * secret-returning-free signer implementation and its static import test arrive
 * in IDR-06.
 */
export * from "./signer.ts";
export * from "./recovery-state.ts";
export * from "./manifest.ts";
export * from "./service.ts";
