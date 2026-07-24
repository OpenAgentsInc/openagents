# Omega-effectd macOS component release

- Date: 2026-07-24
- Issue: [#9214](https://github.com/OpenAgentsInc/openagents/issues/9214)
- Release: [`omega-effectd-v0.1.0-rc.1`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.1)
- Source commit: `be342ea7525ac50fbf836978dcd1ba6714345f42`
- Status: complete

## RC.3 replacement

The final Omega candidate must use
[`omega-effectd-v0.1.0-rc.3`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.3).
RC.3 supersedes RC.2 because GPUI Agent threads are provider-bound: changing
only the stored lane left the retained Codex thread unable to execute a Claude
continuation. No Omega candidate containing RC.2 was signed or published.

| Field | Value |
| --- | --- |
| Source commit | `b8057682598d3744f09f6c6daf24823644b1e3ab` |
| Source tree | `722c22887f43290161373d6c79e48914f5d27548` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `0fb275283686f9de27d326de1c87bbd0ed8ed1360126956658a2c29818d1ea6c` |

Two clean builds produced the same archive digest. The package type check and
all 198 tests passed. The framed path creates a distinct target-lane Agent
thread, atomically records the new run execution binding, transfers the full
continuation grant, and repairs an interrupted second registry write before
reconciliation. Source evidence is sealed into the report before migration.
The next target-provider mission includes the owner objective and prior
handoff. The transition receipt names both source and target thread refs.
RC.1 and RC.2 remain immutable historical inputs, but neither is eligible for
the final Omega candidate.

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

The package type check passed. All 198 package tests passed. The repository
fast main guard passed before the source commit reached `main`.

## Omega use

The Omega packager must download this exact asset and verify its SHA-256 value.
It must install the component at
`Omega.app/Contents/Resources/omega-effectd`. It must sign the nested Node.js
runtime before it signs the Omega executables and the app. The installed proof
must run the framed health check from the mounted app and from the installed
app.
