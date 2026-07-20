/**
 * `@openagentsinc/local-secret-store` — the neutral platform secret-store port.
 *
 * This package defines how a platform secret store keeps ONE opaque encrypted
 * payload per locator: write, read, delete, a presence-only lookup, and custody
 * state. It knows nothing about Nostr, Spark, or any derivation rule; the bytes
 * are opaque. A higher package (`@openagentsinc/sovereign-identity`) gives the
 * bytes meaning.
 *
 * It ships one real adapter: the in-memory test adapter (`./in-memory`). The
 * platform adapter contracts here have no implementation that touches a real
 * store yet. This package imports no app, no Pylon or Desktop code, no React or
 * Electron, no wallet SDK, no cloud client, and no Nostr or Spark primitive. See
 * `src/boundary.test.ts`.
 */
export * from "./locator.ts";
export * from "./secret-store.ts";
export * from "./platform-adapters.ts";
export * from "./in-memory.ts";
// IDR-05 real and typed platform adapters, composed over an injected OS-command
// runner (desktop) or native bridge (mobile). The real OS path runs only behind
// an explicit owner-attended gate; see each module.
export * from "./command-runner.ts";
export * from "./command-backed-store.ts";
export * from "./macos-keychain.ts";
export * from "./windows-linux-adapters.ts";
export * from "./native-secret-store.ts";
