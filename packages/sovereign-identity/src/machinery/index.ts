/**
 * IDR-01 sovereign-identity machinery.
 *
 * This barrel adds the runtime machinery on top of the IDR-00 frozen contract:
 * the signer boundary ports, the recovery state machine (types and transitions
 * only), the public manifest writer contract and migration receipt production,
 * and the `SovereignIdentity` service composed over the injected
 * `LocalSecretStore` and `ManifestStore` ports.
 *
 * IDR-02 adds existence-only candidate discovery and the fail-closed open, plus
 * the separate explicit create operation. The narrowed secret-returning-free
 * signer implementation and its static import test arrive in IDR-06.
 */
export * from "./signer.ts";
export * from "./recovery-state.ts";
export * from "./discovery.ts";
export * from "./open.ts";
export * from "./manifest.ts";
export * from "./service.ts";
