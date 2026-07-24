# Omega-effectd macOS component release

- Date: 2026-07-24
- Issue: [#9214](https://github.com/OpenAgentsInc/openagents/issues/9214)
- Release: [`omega-effectd-v0.1.0-rc.1`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.1)
- Source commit: `be342ea7525ac50fbf836978dcd1ba6714345f42`
- Status: complete

## RC.2 replacement

The final Omega candidate must use
[`omega-effectd-v0.1.0-rc.2`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.2),
which supersedes RC.1 by exposing the existing paused-only Full Auto provider
handoff authority over the framed protocol.

| Field | Value |
| --- | --- |
| Source commit | `c164bc25c42dab369d645d77c99f86b083323540` |
| Source tree | `26378837c60dbcd4253918cb1a07ce8c36f0c5a8` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `f9b4bb93216d4a09dea97a3a94ab1679e1a8d9c13c68a93affd5818d08554cd8` |

Two clean builds produced the same archive digest. The package type check and
all 197 tests passed. The framed test covers running-state refusal, unknown
lane refusal, paused Codex-to-Claude handoff, durable run and dispatch-profile
rebinding, resume, and secret-shaped handoff-reason redaction. RC.1 remains an
immutable historical input but lacks this packaged handoff method.

## Result

OpenAgents released a standalone macOS arm64 component for Omega. The component
does not need an OpenAgents source checkout at build time or at run time.

The archive contains these items:

- the bundled `@openagentsinc/omega-effectd` `0.1.0` service.
- the fixed Node.js `24.13.1` Darwin arm64 runtime.
- the executable `bin/omega-effectd` wrapper.
- the component manifest.
- the Node.js license.
- the OpenAgents Apache 2.0 license.
- the notices and license text for each bundled production dependency.

## Artifact

| Field | Value |
| --- | --- |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `52fb8333ee65b944ba47b2ec00abc77b3826aa7f9a4cacc3ca6f7d37e139ffa5` |
| Protocol | `openagents.omega.effectd.v1` |
| Node.js archive | `node-v24.13.1-darwin-arm64.tar.gz` |
| Node.js archive SHA-256 | `8c039d59f2fec6195e4281ad5b0d02b9a940897b4df7b849c6fb48be6787bba6` |

The GitHub prerelease has the archive, its SHA-256 sidecar, and the component
manifest. The release is a component input for an Omega RC. It is not a stable
product release.

## Verification

The build command produced the same archive digest in two runs. A clean
download from the GitHub prerelease had the same digest. The extracted service
passed the framed `initialize` and `health` checks with a temporary data root.

The package type check passed. All 197 package tests passed. The repository
fast main guard passed before the source commit reached `main`.

## Omega use

The Omega packager must download this exact asset and verify its SHA-256 value.
It must install the component at
`Omega.app/Contents/Resources/omega-effectd`. It must sign the nested Node.js
runtime before it signs the Omega executables and the app. The installed proof
must run the framed health check from the mounted app and from the installed
app.
