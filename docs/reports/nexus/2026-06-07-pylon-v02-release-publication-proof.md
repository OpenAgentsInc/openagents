# Pylon v0.2 Release Publication Proof

Date: 2026-06-07

## Summary

`pylon-v0.2.2` is the current stable public Pylon v0.2 binary release in the
canonical `OpenAgentsInc/openagents` repository.

The release is published with Darwin arm64 and Linux x86_64 assets. Both assets
were downloaded from the public GitHub release URL, checksum verified, extracted
outside the source checkout, and used to run the bounded
`cs336-a1-hosted-starter` proof lane to terminal `completed` state.

This report does not treat the old GCP/native Nexus lane as a release blocker.
The current v0.2 release path is the MDK-default Pylon path with
Omega/Cloudflare MDK checkout proof, the Artanis SHC bootstrap proof, and local
proof-runtime accepted-work evidence recorded in this repository.

## Current Published Release

- Repository: `OpenAgentsInc/openagents`
- Release tag: `pylon-v0.2.2`
- Release URL:
  `https://github.com/OpenAgentsInc/openagents/releases/tag/pylon-v0.2.2`
- Release name: `Pylon v0.2.2`
- Published at: `2026-06-07T17:39:57Z`
- Target commit: `673fefd4b230242198b053e7a4be3e08727b0a8b`
- Release status: not draft, not prerelease

Published assets:

| Asset | Size | GitHub digest |
| --- | ---: | --- |
| `pylon-v0.2.2-darwin-arm64.tar.gz` | `65207049` bytes | `sha256:08d9c2283c4636930c53eb5a428df1188f9f3789d7183c1e64f8d390f272c92b` |
| `pylon-v0.2.2-darwin-arm64.tar.gz.sha256` | `99` bytes | `sha256:1798a32a67a6d533a8b72a268ed90f207f4469171429195c24331d1970e1e3c1` |
| `pylon-v0.2.2-linux-x86_64.tar.gz` | `233130191` bytes | `sha256:60e01f28851437b486319a53b688f44f7a4e6e3a17734770bde9867886810a3c` |
| `pylon-v0.2.2-linux-x86_64.tar.gz.sha256` | `99` bytes | `sha256:2460bceffa63ae613bd86fb244c34e9458e322a12e08ee2da8381240fb8b8ce7` |

## Why v0.2.2 Exists

The first v0.2 publication attempts exposed real packaging defects:

- `pylon-v0.2.0` shipped public binaries, but the proof lane could not run from
  a fresh archive on SHC because the archive did not include the support
  binaries `nexus-relay` and `nexus-control`.
- `pylon-v0.2.1` packaged those support binaries and fixed release README
  escaping, but the proof runtime still preferred source-tree `cargo build`
  fallback paths over colocated packaged support binaries when a source checkout
  happened to exist.
- `pylon-v0.2.2` fixes both problems. The archive includes `pylon`,
  `pylon-tui`, `nexus-relay`, `nexus-control`, and the packaged Psionic runtime
  surface. The runtime resolver now prefers colocated support binaries before
  falling back to Cargo.

Treat `pylon-v0.2.0` and `pylon-v0.2.1` as superseded. Operators should install
or test `pylon-v0.2.2` or a later patch release.

## Darwin Public Asset Proof

The Darwin arm64 archive was downloaded through the public GitHub release URL,
verified with the uploaded checksum file, extracted into a temp directory, and
run outside the source checkout.

Proof command shape:

```bash
curl -fsSL -o pylon-v0.2.2-darwin-arm64.tar.gz \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.2/pylon-v0.2.2-darwin-arm64.tar.gz
curl -fsSL -o pylon-v0.2.2-darwin-arm64.tar.gz.sha256 \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.2/pylon-v0.2.2-darwin-arm64.tar.gz.sha256
shasum -a 256 -c pylon-v0.2.2-darwin-arm64.tar.gz.sha256
tar -xzf pylon-v0.2.2-darwin-arm64.tar.gz
cd pylon-v0.2.2-darwin-arm64
./pylon --version
./pylon-tui --version
PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  ./pylon proof run cs336-a1-hosted-starter \
  --namespace pylon-v022-darwin-public-proof-20260607174023 \
  --timeout-seconds 600 \
  --json
```

Observed results:

- Checksum verification returned `pylon-v0.2.2-darwin-arm64.tar.gz: OK`.
- `./pylon --version` returned `pylon 0.2.2`.
- `./pylon-tui --version` returned `pylon-tui 0.2.2`.
- `nexus-relay` and `nexus-control` were executable in the extracted archive.
- `PATH` excluded the local Cargo toolchain during the proof run.
- Process inspection showed the proof using the extracted archive's `pylon` and
  colocated `nexus-relay`, with no Cargo or Rust compiler process.

Proof result:

```json
{
  "status": "completed",
  "lane": "cs336-a1-hosted-starter",
  "namespace": "pylon-v022-darwin-public-proof-20260607174023",
  "detail": "window window.cs336.a1.starter.20260607174056.8b8804ed.0001 reconciled with 1 accepted contribution(s), closeout=rewarded, workers_healthy=2, validators_healthy=1"
}
```

Public-safe artifact:
`docs/reports/nexus/pylon-v022-darwin-public-proof-20260607174023.json`.

## Linux SHC No-Source Public Asset Proof

The Linux x86_64 archive was built on SHC and uploaded to GitHub Releases. To
avoid false confidence from a colocated build checkout, the SHC build directory
was then moved out of the expected source path before the public install proof
was rerun.

Proof command shape:

```bash
curl -fsSL -o pylon-v0.2.2-linux-x86_64.tar.gz \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.2/pylon-v0.2.2-linux-x86_64.tar.gz
curl -fsSL -o pylon-v0.2.2-linux-x86_64.tar.gz.sha256 \
  https://github.com/OpenAgentsInc/openagents/releases/download/pylon-v0.2.2/pylon-v0.2.2-linux-x86_64.tar.gz.sha256
sha256sum -c pylon-v0.2.2-linux-x86_64.tar.gz.sha256
tar -xzf pylon-v0.2.2-linux-x86_64.tar.gz
cd pylon-v0.2.2-linux-x86_64
./pylon --version
./pylon-tui --version
test -x ./nexus-relay
test -x ./nexus-control
test -x ./psionic/target/release/psionic-train
OPENAGENTS_PYLON_HOME="$RUN_ROOT/pylon-home" ./pylon init
OPENAGENTS_PYLON_HOME="$RUN_ROOT/pylon-home" ./pylon status --json
PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
  OPENAGENTS_PYLON_HOME="$RUN_ROOT/pylon-home" \
  ./pylon proof run cs336-a1-hosted-starter \
  --namespace pylon-v022-shc-nosource-proof-20260607183407 \
  --timeout-seconds 600 \
  --json
```

Observed results:

- Checksum verification returned `pylon-v0.2.2-linux-x86_64.tar.gz: OK`.
- `./pylon --version` returned `pylon 0.2.2`.
- `./pylon-tui --version` returned `pylon-tui 0.2.2`.
- `nexus-relay`, `nexus-control`, and
  `psionic/target/release/psionic-train` were executable in the extracted
  archive.
- The original SHC build checkout was hidden before this proof.
- `PATH` excluded Cargo during the proof run.
- Process inspection showed the proof using only paths under the extracted
  public archive:
  `/tmp/pylon-v022-shc-nosource.4lwjA8/pylon-v0.2.2-linux-x86_64/...`.
- No Cargo or Rust compiler process was present during the no-source proof.

Proof result:

```json
{
  "status": "completed",
  "lane": "cs336-a1-hosted-starter",
  "namespace": "pylon-v022-shc-nosource-proof-20260607183407",
  "detail": "window window.cs336.a1.starter.20260607183442.404ae7ea.0001 reconciled with 1 accepted contribution(s), closeout=rewarded, workers_healthy=2, validators_healthy=1"
}
```

Public-safe artifacts:

- `docs/reports/nexus/pylon-v022-shc-nosource-proof-20260607183407.json`
- `docs/reports/nexus/pylon-v022-shc-nosource-status-20260607183407.json`

## Npm Bootstrap Proof

The npm bootstrap package is published as `@openagentsinc/pylon@0.2.2`.

Evidence:

- `npm view @openagentsinc/pylon version` returns `0.2.2`.
- `npm view @openagentsinc/pylon@0.2.2 dist --json` returns:
  - integrity:
    `sha512-JSnt0JbaFZ/F2ngjC+zMvtXqYBvbhRBdDsTfDUCQV++tLW1CtM+Kwx8P+5QGGjFxkJwmGaaNiuSW0A3IAHjOAA==`
  - shasum: `cbffc9e216196e218c261bf5328970c6cdf8982a`
  - tarball:
    `https://registry.npmjs.org/@openagentsinc/pylon/-/pylon-0.2.2.tgz`
- `npm whoami` succeeds for the `openagentsinc` account.
- The operator completed npm CLI one-time authorization and
  `npm publish --access public` returned `+ @openagentsinc/pylon@0.2.2`.

SHC clean npm bootstrap smoke:

```bash
HOME="$SMOKE_ROOT/home-env" \
NPM_CONFIG_CACHE="$SMOKE_ROOT/npm-cache" \
OPENAGENTS_DISABLE_TELEMETRY=1 \
npm exec --yes --package @openagentsinc/pylon@0.2.2 -- pylon \
  --version 0.2.2 \
  --install-root "$SMOKE_ROOT/install" \
  --pylon-home "$SMOKE_ROOT/pylon-home" \
  --no-launch \
  --no-updates \
  --json
```

Observed results:

- Node: `v18.19.1`.
- npm: `9.2.0`.
- `version`: `0.2.2`.
- `tagName`: `pylon-v0.2.2`.
- `target`: `linux` / `x86_64`.
- `installMethod`: `release_asset`.
- `cached`: `false`.
- installed binary:
  `/tmp/pylon-npm-022-shc.FuRL6t/install/versions/pylon-v0.2.2-linux-x86_64/pylon`.
- installed TUI:
  `/tmp/pylon-npm-022-shc.FuRL6t/install/versions/pylon-v0.2.2-linux-x86_64/pylon-tui`.
- fresh init config:
  `/tmp/pylon-npm-022-shc.FuRL6t/pylon-home/config.json`.
- `desiredMode`: `offline`.
- inventory rows: `2`.
- packaged Psionic source:
  `/tmp/pylon-npm-022-shc.FuRL6t/install/versions/pylon-v0.2.2-linux-x86_64/psionic`.
- `psionicRepoSource`: `exe_ancestor_sibling`.
- `runtimeSurfaceDetected`: `true`.
- `contributorSupported`: `true`.
- executable packaged files included `pylon`, `pylon-tui`, `nexus-relay`,
  `nexus-control`, and `psionic/.openagents-psionic-revision`.

Public-safe artifact:
`docs/reports/nexus/pylon-v022-shc-npm-bootstrap-202606071836.json`.

## Release Engineering Notes

The SHC Linux archive build completed from a clean clone, but the packaged
Psionic support binary build is expensive on cold cache:

- OpenAgents release-profile build: `23m35s`.
- Psionic `psionic-train` support build: `20m41s`.
- Final Linux archive size: `223M`.

The build is valid, but future patch releases should consider reducing the
support-runtime profile or caching the Psionic support artifact. Do not remove
the support runtime from the archive unless the proof lane is changed to avoid
needing it.

## Misplaced Psionic Release Cleanup

The stale `OpenAgentsInc/psionic` release `v0.2.0` was inspected and removed.
It was titled `Psionic v0.2.0 Pylon release`, had no assets, pointed at
`main`, and conflicted with the decision that Pylon releases belong in the
`OpenAgentsInc/openagents` repository under `pylon-v...` tags.

Cleanup command:

```bash
gh release delete v0.2.0 \
  --repo OpenAgentsInc/psionic \
  --cleanup-tag \
  --yes
```

Verification:

- `gh release view v0.2.0 --repo OpenAgentsInc/psionic` no longer resolves.

## Post-Release Artanis/Pylon Paid-Work Proof

The current post-release proof artifact is:

- `docs/reports/nexus/pylon-v02-post-release-artanis-paid-work-proof-20260607-155959.json`
- status: `completed`
- lane: `cs336-a1-hosted-starter`
- namespace: `pylon-v02-post-release-20260607-155959`
- detail:
  `window window.cs336.a1.starter.20260607160246.69d216ae.0001 reconciled with 1 accepted contribution(s), closeout=rewarded, workers_healthy=2, validators_healthy=1`

That proof was run from a downloaded Darwin Pylon binary using an isolated
Pylon home. It proves released-binary accepted-work closeout through the local
proof runtime with simulated treasury enabled. It does not claim real public
Bitcoin settlement.

## Current Release Judgment

`pylon-v0.2.2` is the stable public GitHub binary release for Pylon v0.2, and
`@openagentsinc/pylon@0.2.2` is published and verified through a clean SHC npm
bootstrap smoke.

If an npm or post-release smoke fix is needed later, cut a later patch release
such as `pylon-v0.2.3` or publish a package-only patch when the Rust artifacts
remain unchanged.
