# CUT-26 Desktop RC publication receipt

Date: 2026-07-12  
Issue: #8706  
Current version source: `4c4bd8e2c5` plus subsequent packaging hardening

## Published candidate

- identity/version: `OpenAgents` / `com.openagents.desktop` / `0.1.0-rc.5`
- artifact: `OpenAgents-0.1.0-rc.5-arm64.dmg`
- final post-staple bytes: `234540944`
- final SHA-256:
  `cf17f5d987f26f4fda732e48fd86e662b3c9a54ac5d0f39d189a18b0753e8f2b`
- Developer ID: `OpenAgents, Inc. (HQWSG26L43)`
- Apple notarization: accepted; DMG staple and validation passed
- update-manifest key: production kid `2dbe811d19f67528`
- public RC feed:
  `https://updates.openagents.com/desktop/openagents/rc/manifest.json`
- Cloud Run: `oa-updates-00107-rob`, 100% traffic

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
100% traffic moved to the current RC5 revision.

Installed-artifact testing then found two additional counterexamples rather
than accepting publication as proof. RC1 requested a browser-specific V8
snapshot Electron did not ship. RC2 reached main but could not admit its
renderer from inside ASAR while `GrantFileProtocolExtraPrivileges=false`.
The repair keeps that security fuse disabled, materializes only the bounded
renderer under `app.asar.unpacked/dist/renderer`, excludes the redundant
workspace dependency copy, and signs only executable/bundle paths including
Squirrel `ShipIt`.

The exact mounted, stapled RC5 DMG completed the full packaged smoke through
shell mount, Runtime Gateway bootstrap, workspace/editor/reload recovery,
typed provider turns, Fleet, terminal, Git review, settings/diagnostics, image
attachment, and second-instance deep link. It ended with
`[openagents-desktop smoke] OK` and lifecycle teardown
`{"ok":true,"active":0}`. The production manifest is exact, mobile OTA remains
HTTP 200 (1918 bytes), and both deprecated Desktop feeds remain typed 410.

## Remaining close gate

Publication and the downloaded-artifact packaged smoke are complete. CUT-26
remains open only for the broader clean-supported-Mac lifecycle: real named-
account readiness/coding, interrupted update/resume, rollback,
uninstall/reinstall, and diagnostics export.
