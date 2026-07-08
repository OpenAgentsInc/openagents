/**
 * Shared engine foundation — the dependency-closed infra layer that the
 * custody / presence / wallet / executor services all build on.
 *
 * Moved out of `apps/pylon/src` (issue #8578) so the higher service layers
 * can extract into this package without importing back from the app (which
 * would be a circular `pylon` <-> `pylon-core` dependency). Original
 * `apps/pylon/src` modules are thin re-export shims.
 *
 *   version         — pinned Pylon version + type
 *   wsl-host-detect — WSL host detection
 *   bootstrap       — home resolution, bootstrap summary, platform gate
 *   nostr-identity  — load/create the node's nostr identity
 *   inventory       — host inventory projection
 *   state           — lifecycle/runtime/presence state + public projection
 */

export * from "./version.js"
export * from "./wsl-host-detect.js"
export * from "./bootstrap.js"
export * from "./nostr-identity.js"
export * from "./inventory.js"
export * from "./state.js"
