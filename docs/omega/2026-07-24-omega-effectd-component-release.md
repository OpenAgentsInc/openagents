# Omega-effectd macOS component release

- Date: 2026-07-24
- Issue: [#9214](https://github.com/OpenAgentsInc/openagents/issues/9214)
- Release: [`omega-effectd-v0.1.0-rc.1`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.1)
- Source commit: `be342ea7525ac50fbf836978dcd1ba6714345f42`
- Status: complete

## RC.8 replacement

The next Omega candidate must use
[`omega-effectd-v0.1.0-rc.8`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.8).
RC.8 supersedes RC.7 because Stop could commit the durable terminal state while
an ACP turn remained alive when the in-memory evidence cache was empty after a
restart. Stop now always requests best-effort interruption of its bound host
thread without refreshing evidence, provider authentication, or lane
readiness. RC.8 also admits the additive `timed_out` local-turn disposition for
Omega's bounded native provider turns.

| Field | Value |
| --- | --- |
| Source commit | `509ae747f00f6f7ebb413809ff5bd6ea123e1c1c` |
| Source tree | `063f456be4e196b5eb6eff18f9a14453a52599fc` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `01d11597b054d009296d0381b6cd6ed3d31c83b93e75a58845f2ae47bf33226a` |
| Manifest SHA-256 | `74f7eb1043de6662f490351d6587a4e85fb50d65e95aac792b4b32c315081f94` |

Two clean builds produced identical archive, sidecar, and manifest bytes. The
package type check and all 208 tests passed. The repository fast guard passed
at the source commit. A clean GitHub prerelease download matched the local
archive, sidecar, and manifest bytes, and the release tag resolves to the exact
source commit.

## RC.7 replacement

The next Omega candidate must use
[`omega-effectd-v0.1.0-rc.7`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.7).
RC.7 supersedes RC.6 because a provider turn that finished after Pause could
leave the durable run in `pausing`. The framed read paths now refresh host
evidence for pausing runs before projecting them, allowing the existing state
machine to settle the run to `paused` without cancelling a healthy turn.

| Field | Value |
| --- | --- |
| Source commit | `fb3b17a873d2c09823a7292e8af3b853096f48ab` |
| Source tree | `68bc6dc7f79880a8a95da28e0f070890380c9818` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `51725a10e969dfefdcc35b29bc5ae4bacb69b9770ee1a1ae67e79daaebf9d6a1` |
| Manifest SHA-256 | `d9668118af540879a3e8d451922004137af6fe821bade9b50487d22902d44d2a` |

Two clean builds produced identical archive, sidecar, and manifest bytes. The
package type check and all 208 tests passed. The repository fast guard passed
at the source commit. A clean GitHub prerelease download matched the local
archive, sidecar, and manifest bytes, and the release tag resolves to the exact
source commit.

## RC.6 replacement

The next Omega candidate must use
[`omega-effectd-v0.1.0-rc.6`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.6).
RC.6 supersedes RC.5 because Full Auto reports and authenticated controls must
remain available away from the desktop, and a configured turn cap must be an
enforced terminal condition rather than advisory metadata.

RC.6 projects current run reports into the configured Sync directory and
ingests authenticated mobile Pause, Resume, and Stop commands with typed
outcomes. Reconciliation now stops a run when its configured maximum turn
count is reached.

| Field | Value |
| --- | --- |
| Source commit | `5bb31ac857b917b14c6455a7df268825cfbf773f` |
| Source tree | `ce9d37bb3a59ae7e2c807e2d524c196af17484ed` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `b55f703229ff9299923a84b0843f9c926fbd75b08e787f5d6e79744fd114c836` |
| Manifest SHA-256 | `13f0e094c5d120426f4ede3afedd24f04abec71e29abcd7700c0fd2e36037953` |

Two clean builds produced identical archive, sidecar, and manifest bytes. The
package type check and all 207 tests passed. The repository fast guard passed
at the source commit. A clean download from the GitHub prerelease matched the
local archive, sidecar, and manifest bytes. The release tag, source commit,
source tree, and embedded manifest were verified independently.

## RC.5 replacement

The next Omega candidate must use
[`omega-effectd-v0.1.0-rc.5`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.5).
RC.5 supersedes RC.4 because framed Stop refreshed host evidence before it
committed the local terminal transition. A paused run could therefore become
stranded when its selected provider lost authentication.

RC.5 makes Stop independent of provider authentication, lane readiness, and
host-evidence refresh. Cached live-turn evidence is used only to request
interruption of an already-known active turn.

| Field | Value |
| --- | --- |
| Source commit | `5307ec18f9717c8060ff49239ff368150bee40ed` |
| Source tree | `e5314142d2b0da57f3da0085854eeb2bef97c05e` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `a1d7662cb1e45a9a22bb71a742189babb55fcfbd5ca687204123d85824bbcb1b` |
| Manifest SHA-256 | `c6f89764b21c6299eadd57c8353675505535cf8f09059cd022961f9054426b10` |

Two clean builds produced identical archive and manifest bytes. The package
type check and all 201 tests passed. The repository fast guard passed before
the source commit reached `main`. A clean download from the GitHub prerelease
matched the local archive, sidecar, and manifest bytes.

## RC.4 replacement

The next Omega candidate must use
[`omega-effectd-v0.1.0-rc.4`](https://github.com/OpenAgentsInc/openagents/releases/tag/omega-effectd-v0.1.0-rc.4).
RC.4 supersedes RC.3 because `get_capacity` used a process-local readiness
cache. The cache was empty before the first run reconciliation. Thus, an
available host lane could appear unavailable during the initial capacity
check.

RC.4 probes all Full Auto lanes through the Omega host bridge before it
projects capacity. The host request and cache update use the active supervisor
generation. A generation change rejects the pending capacity result. It does
not let data from an old host generation enter the new cache.

| Field | Value |
| --- | --- |
| Source commit | `db6412190b3de5712d2a0957644af5c76679f697` |
| Source tree | `5958316faa32e327580e30ef7006002cfcae2471` |
| Archive | `omega-effectd-v0.1.0-macos-arm64.tar.gz` |
| Archive SHA-256 | `242da72ae741e27524c5782f5b6a38eb6869ed43fbfcb7ba05fe14fb04b737d6` |
| Manifest SHA-256 | `f1f2c238545d3fa51e43cc4365ef1df0d0340db0b3c6ee6139251140269600dd` |

Two clean builds produced the same archive digest. The package type check and
all 200 tests passed. The repository fast guard passed before the source
commit reached `main`. A clean download from the GitHub prerelease matched the
local archive and manifest digests.

## RC.3 replacement

The RC.3 Omega candidate had to use
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
the RC.3 Omega candidate. RC.3 is now an immutable historical input.

Independent clean-room verification used a strict host that permanently bound
each created thread to its creation lane and rejected mismatched dispatch. The
Codex thread and distinct Claude thread completed start, pause, handoff, resume,
mission-context transfer, restart recovery, and secret redaction. A simulated
crash after the run-registry rebind but before the continuation-registry move
was repaired before reconciliation. Release tag, source tree, archive,
sidecar, manifests, Node runtime, licenses, deterministic archive metadata,
and private file modes all matched their pinned values.

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
