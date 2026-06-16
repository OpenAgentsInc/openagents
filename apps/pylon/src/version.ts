// Single source of truth for the Pylon release version.
//
// The `bun --compile` binary does NOT bundle package.json, so the compiled
// Pylon reports THIS constant from `status --json` and the OTA self-updater
// compares against it. Keep package.json's "version" in sync, but treat this as
// authoritative for the running binary.
//
// Bump for each release cut. v1.0-rc cut: 2026-06-16 (#5122).
export const PYLON_VERSION = "1.0.0-rc.6"
export type PylonVersion = typeof PYLON_VERSION

// Composed client-version string sent in presence/heartbeat payloads.
export const PYLON_CLIENT_VERSION = `openagents.pylon@${PYLON_VERSION}` as const
export type PylonClientVersion = typeof PYLON_CLIENT_VERSION
