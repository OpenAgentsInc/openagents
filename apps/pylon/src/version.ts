// Single source of truth for the Pylon release version.
//
// The `bun --compile` binary does NOT bundle package.json, so the compiled
// Pylon reports THIS constant from `status --json` and the OTA self-updater
// compares against it. Keep package.json's "version" in sync, but treat this as
// authoritative for the running binary.
//
// Bump for each release cut. v1.0-rc cut: 2026-06-16 (#5122).
// rc.9: spark-selftest diagnostic + readiness auto-report + binary SDK guard (#5166).
// rc.14: Spark wallet send/withdraw + Spark-primary balance + Spark treasury
// payouts (#5176). Supersedes the rc.13 cut, which bumped package.json but NOT
// this constant — so rc.13 binaries self-reported rc.12, auto-update-looped, and
// showed the wrong version in /api/pylons. Keep BOTH in sync on every cut.
// rc.33: install the Breez SDK stdout guard as a TOP-LEVEL side effect of the
// first entry import (src/breez-stdout-guard.ts) so the SDK's "Node.js storage
// automatically enabled" banner cannot reach stdout before the guard is in
// place in the compiled binary. The rc.32 runtime guard fixed backup-status but
// not status --json, because the bundled binary could eval the SDK before main()
// installed it. Breez stays lazily imported.
export const PYLON_VERSION = "1.0.0-rc.33"
export type PylonVersion = typeof PYLON_VERSION

// Composed client-version string sent in presence/heartbeat payloads.
export const PYLON_CLIENT_VERSION = `openagents.pylon@${PYLON_VERSION}` as const
export type PylonClientVersion = typeof PYLON_CLIENT_VERSION
