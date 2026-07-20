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
// IDR-06 narrowed real signer, backed by the `nostr-effect` `IdentityKeys`
// façade. The secret-export custody module (`./custody.ts`) is intentionally NOT
// re-exported here, so a normal caller cannot reach the key-export escape
// hatches through the package barrel.
export * from "./local-signer.ts";
export * from "./recovery-state.ts";
export * from "./discovery.ts";
export * from "./open.ts";
export * from "./manifest.ts";
export * from "./service.ts";
// IDR-05 import to platform custody: the confirmed-identity import flow, the
// restart+restore read path, and the atomic file-backed public manifest store.
export * from "./import.ts";
export * from "./file-manifest-store.ts";
// IDR-08 migrate applications to ONE identity service: the single resolved public
// identity projection both surfaces consume, the mobile custody composition over
// the IDR-05 native bridge, and the web signer bridge / NIP-46 seam that carries
// public identity and signer operations only — never a raw key.
export * from "./resolved-identity.ts";
export * from "./mobile-custody.ts";
export * from "./web-signer-bridge.ts";
