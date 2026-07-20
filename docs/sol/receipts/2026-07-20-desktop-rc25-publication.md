# OpenAgents Desktop RC25 experimental publication receipt

Date: 2026-07-20

## Outcome

OpenAgents Desktop `0.1.0-rc.25` is public as an immutable experimental
GitHub prerelease with the complete five-target artifact set:

- release: https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.25
- source revision: `e1a0514568d94f8a951ec296d44e4497b212c086`
- publication class: `desktop_experimental_prerelease`
- published desktop assets: 11 (plus `SHA256SUMS` and the retained `IDE.png`)
- signed Desktop feed promoted: no
- stable channel promoted: no

The release puts every supported platform in testers' hands at one coherent
source revision. It does not represent the release as a complete signed
`ReleaseSet` or a stable promotion.

## Authority and attribution

- trigger: `owner_direction`
- triggered by: OpenAgents owner direction to publish the complete rc.25 asset
  set to the existing experimental prerelease
- release actor: OpenAgents release operator
- authority profile: `AUTHORITY.md` revision 2
- grant: `grant.autonomous_rc_release_and_communication`

This grant admits experimental RC publication, candidate communication, and
rollback. It does not admit stable promotion or signed-feed movement.

## Immutable artifact manifest

All eleven desktop artifacts share source revision
`e1a0514568d94f8a951ec296d44e4497b212c086` and version `0.1.0-rc.25`, channel
`rc`.

| Target | Artifact | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| darwin-arm64 | `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.dmg` | 204471445 | `8ed47f69be023a8568b8a5eee1d65c3401bd5a1c5a8a2053da6f721becfdf0b3` |
| darwin-arm64 | `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.zip` | 206482442 | `19284f1b3892c356e4138fde667fc788693e0cc992283d83bacba2abd7c652d4` |
| darwin-x64 | `OpenAgents-0.1.0-rc.25-rc-darwin-x64.dmg` | 212294508 | `2675bf7ba5ba9d8ec334a41404b387f44a7556bc47b8f8cf4bbb54a18ae2228d` |
| darwin-x64 | `OpenAgents-0.1.0-rc.25-rc-darwin-x64.zip` | 214673703 | `a65e39d1105c4c370326570eb883eedb49aede42814a12a5ede78ea87f139679` |
| linux-x64 | `OpenAgents-0.1.0-rc.25-rc-linux-x64.AppImage` | 225096605 | `a6072f0c64aad76ad7ea532a7b6efd7202f65fc02d33100351ae666d5c94bac5` |
| linux-x64 | `OpenAgents-0.1.0-rc.25-rc-linux-x64.deb` | 165876430 | `af1274d25eb19555f4f9e8d2b1fb25dcb95ea8a37d2e15cc40a3b8375b0addda` |
| linux-x64 | `OpenAgents-0.1.0-rc.25-rc-linux-x64.rpm` | 176260621 | `428b9d83f2f8538ba73423aa2471da19fe8d44377270098110673a17c83f570b` |
| linux-arm64 | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.AppImage` | 224675408 | `257e57459b62009a712c83f02e08a01da73610180367446badc81dbe0dc1f6d4` |
| linux-arm64 | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.deb` | 164270022 | `7585c3df4e05228001de808307d716aaa39fe0f31fc8b1a27fa606843b4da4f8` |
| linux-arm64 | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.rpm` | 167773001 | `2f85492585fbe968a34578262a56e8fd53a024a170c73aef40a2a78ad8f7fb5a` |
| win32-x64 | `OpenAgents-0.1.0-rc.25-rc-win32-x64-portable.zip` | 233443404 | `38f98a35aaaa5897afe6c217953e45a6b0020c6d15d82a9f87457201e171bce2` |

A `SHA256SUMS` file that covers all eleven assets is attached to the release.
The retained `OpenAgents-0.1.0-rc.25-IDE.png` screenshot stays on the release.

## Per-target evidence and honesty notes

The nine Linux and macOS-x64 artifacts were verified byte-for-byte against
their candidate receipts before upload:

- macOS x64: `docs/deploy/receipts/2026-07-20-macos-x64-signed-candidate.md`.
  Developer ID signed, Apple notarized, stapled, Gatekeeper-accepted, thin
  `x86_64`.
- Linux x64 AppImage and DEB:
  `docs/deploy/receipts/2026-07-20-linux-signed-candidate.md`.
- Linux x64 RPM and all three Linux arm64 formats:
  `docs/deploy/receipts/2026-07-20-linux-rpm-and-arm64.md`. Each carries a
  detached Ed25519 release signature (kid `2dbe811d19f67528`).
- Windows x64 portable ZIP:
  `docs/deploy/receipts/2026-07-20-win32-x64-portable-candidate.md`. This is an
  unsigned experimental portable artifact with no Authenticode signature and no
  installer trust claim. It matches the rc.20 and rc.21 Windows coverage.

### macOS arm64 rebuild note

The macOS arm64 DMG and ZIP recorded in
`docs/deploy/receipts/2026-07-20-macos-arm64-signed-candidate.md`
(`807a3e46…` / `79fa00a9…`) were no longer present on the Apple Silicon build
host at publication time. To keep the whole set coherent at one source
revision, the macOS arm64 pair was rebuilt from a fresh detached worktree
checked out at exactly `e1a0514568d94f8a951ec296d44e4497b212c086` through the
repository entrypoint `pnpm run make:mac`, then Developer ID signed, Apple
notarized, stapled, and Gatekeeper-verified. macOS notarized DMG and ZIP bytes
are not reproducible (the staple ticket and container timestamps differ per
build), so the rebuilt arm64 hashes above differ from the earlier candidate
receipt while the source revision is identical. The rebuilt artifacts match the
hashes in the manifest above and were verified as thin `arm64` with a valid
Developer ID staple before upload.

## Publication method

The assets were attached to the existing experimental prerelease with
`gh release upload openagents-desktop-v0.1.0-rc.25 <files> --clobber`. The
release stays a prerelease. It is not marked latest. No channel pointer moved.
The signed Desktop update feed and `updates.openagents.com` were not touched.

## Experimental limitations

- The signed Desktop `ReleaseSet` and update feed were not promoted.
- Windows x64 is an unsigned experimental portable ZIP, not an
  Authenticode-signed installer.
- Linux artifacts are native candidate downloads, not signed-feed support
  evidence.
- These conditions prohibit calling rc.25 a stable release or a complete signed
  five-target `ReleaseSet`.

## Communication receipts

The publication summary and release URL were posted to the DIST target issues
#8919 (macOS), #8920 (Windows), and #8921 (Linux).
