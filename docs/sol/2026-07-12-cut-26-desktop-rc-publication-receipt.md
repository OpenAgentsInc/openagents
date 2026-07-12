# CUT-26 Desktop RC publication receipt

Date: 2026-07-12  
Issue: #8706  
Version source: `a74cb24ef9`

## Published candidate

- identity/version: `OpenAgents` / `com.openagents.desktop` / `0.1.0-rc.1`
- artifact: `OpenAgents-0.1.0-rc.1-arm64.dmg`
- final post-staple bytes: `235069166`
- final SHA-256:
  `ab6b075fb8a6eae27c95f4d90ca380f7902e06bd52c7b55db33d1d4a7a95a962`
- Developer ID: `OpenAgents, Inc. (HQWSG26L43)`
- Apple notarization: accepted; DMG staple and validation passed
- update-manifest key: production kid `2dbe811d19f67528`
- public RC feed:
  `https://updates.openagents.com/desktop/openagents/rc/manifest.json`
- Cloud Run: `oa-updates-00101-kov`, 100% traffic
- incremental build: `537eb006-7662-4cbc-ad4a-9ddba38753ac`

The live manifest and detached signature are exact with the production-signed
seed and describe the final public GCS object size and hash.

## Safety findings

Two Bun-isolated DMG dependencies initially lacked native binaries:
`macos-alias/volume.node` and `fs-xattr/xattr.node`. The repeatable maker fix
landed at `6297f67d7c`; the production runbook records the boundary.

A fresh source deploy was rejected during verification because the source tree
held only OTA metadata, not the complete baked Expo asset tree; the mobile
manifest returned 404. Traffic was immediately restored to
`oa-updates-00096-f7z`. The final Desktop image instead used
`Dockerfile.incremental` with the immutable known-good image as its base.
Before traffic moved, its tagged candidate returned the exact Desktop manifest
and a 200, 1918-byte mobile Expo manifest for runtime
`44f4fbd0b8ab6bdd1aa410467e6df96f572762b2`. Both checks passed again after
100% traffic moved to `oa-updates-00101-kov`.

## Remaining close gate

Publication is complete. CUT-26 remains open until the public DMG completes the
installed-artifact lifecycle on a clean supported Mac: install/first run,
named-account readiness, coding smoke, interrupted update/resume, rollback,
uninstall/reinstall, and diagnostics export. No release-code or publication
gate remains.
