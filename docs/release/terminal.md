# Releasing OpenAgents Terminal

Coder's new terminal uses `scripts/release/coder.sh` and the separate
`coder/` bucket prefix. Coder is one download per platform: from
`1.0.0-rc.6` on, each of the seven platforms publishes one archive,
`coder-<version>-<platform>.tar.gz` (Windows: `.zip`), holding `coder`,
`openagents`, and the `microcoder` engine at its top level, plus
`coder-boundary.exe` on Windows. `SHA256SUMS-coder-<version>` names the
archives, and the manifest records each archive and each signed binary in
it. Versions up to `1.0.0-rc.5` were published as separate executables;
the installers still install those, and `/download` lists manual
downloads only for an archive release (`published_as_archives` in
`crates/openagents-web/src/pages/download.rs`).

To publish a release candidate: bump the three crates' versions and commit;
deploy the website first if its installers changed (the hosted installers
read both layouts, so an older site installer must not meet an archive-only
channel); run `--version 1.0.0-rc.6` to build and check locally, then the
same with `--publish --channel rc`; run `--publish-installers` from the same
commit; then set `CODER_VERSION` in `download.rs` to the new version and
deploy the site, which turns on the per-platform "Coder for <platform>"
downloads. The website serves the installers at `/cli/install.sh` and
`/cli/install.ps1`. `scripts/test-release-coder.sh` and
`scripts/test-install-coder-hosted.py` test the archive layout, channel
coverage, and the installers (both layouts) without a bucket.

### Channels

The installers take `stable` or `rc` (`install.sh rc`, `CODER_CHANNEL=rc`,
or `-Channel rc` on Windows). With no channel named they follow `stable`,
and fall back to `rc`, saying so, while `coder.stable` doesn't exist. So
the installers default to stable the moment 1.0.0 is published, with no
second website deploy. `--channel stable` takes only a release (`X.Y.Z`,
never `-rc.N`), and moves `coder.rc` to the same version, so `rc` never
names an older build than `stable`.

### Publishing 1.0.0

The crates carry `1.0.0` from #11091. `CODER_VERSION` in
`crates/openagents-web/src/pages/download.rs` stays at the newest
published version (`1.0.0-rc.5`) until 1.0.0 is published, because the
page links that version's files. On the release Mac, from a clean checkout
of `main`:

1. **Deploy the website** from this commit (its installers default to
   stable with the rc fallback). Launch step 3 in
   [operations.md](../launch/1.0/operations.md).
2. **Build and check, publishing nothing:**
   `CARGO_TARGET_DIR=~/work/openagents-target-release scripts/release/coder.sh --version 1.0.0`
3. **Publish, point stable and rc, publish the installers** (the one
   command):
   `CARGO_TARGET_DIR=~/work/openagents-target-release scripts/release/coder.sh --version 1.0.0 --publish --channel stable --publish-installers`
4. **Read it back and install it:**
   `curl -fsS https://storage.googleapis.com/openagentsgemini-cli-releases/coder/coder.stable`
   prints `1.0.0`; then `curl -fsSL https://openagents.com/cli/install.sh | bash`
   in a new terminal and `coder --version` prints `coder 1.0.0 (...)`.
5. **Turn on the downloads:** set `CODER_VERSION` to `"1.0.0"`, commit, and
   deploy the website. `/download` then says "Version 1.0.0." and lists the
   seven archives.

Rollback: [operations.md](../launch/1.0/operations.md#terminal-coder).

Published on 2026-10-06: `1.0.0-rc.3`, from commit `1701e1d3c1`.
All 22 public executables passed checksum verification. The Mac executables
for both architectures passed notarization and Gatekeeper. The hosted installer
installed and verified all three commands on an Apple silicon Mac. A native
Windows install remains untested.

The shipped OpenAgents Terminal chat TUI is in the `openagents` program
([user guide](../terminal/README.md)). A release is seven platforms of two
bare executables each, a checksum file, and a channel pointer, in the public
bucket `gs://openagentsgemini-cli-releases` under the prefix `openagents/`.
`scripts/release/terminal.sh` builds, signs, checks, and publishes them;
`scripts/install/openagents.sh` and `.ps1` install them. The flow is the one
Coder Terminal used in the private `coder` repo, adapted
([#10114](https://github.com/OpenAgentsInc/openagents/issues/10114)).

The [workbench roadmap](../terminal/workbench-roadmap.md#3-integrate-install-and-retain-the-demo)
adds the graphical terminal as a separate installable package, with the
Grid using the same implementation. Its first release targets native
macOS and declares its supported platform set. Reuse this process's
signing, checksums, source manifest, readback, and isolated install checks;
use the `openagents-terminal/` package prefix and manifest instead of bypassing this
channel's seven-platform coverage rule. The existing scripts below release
the TUI payload, not the proposed graphical package.

## What a release is

The base URL is
`https://storage.googleapis.com/openagentsgemini-cli-releases/openagents`.
It serves today, without the website. Under it:

| Object | What it is |
| --- | --- |
| `openagents.<channel>` | The version a channel (`rc`, `stable`) names, on one line. |
| `openagents-<version>-<platform>` | The `openagents` program, a bare executable. |
| `microcoder-<version>-<platform>` | The `microcoder` engine Coder runs a turn with. |
| `SHA256SUMS-openagents-<version>` | `<sha256>  <name>` for both, every platform. |
| `openagents-<version>.release-manifest.json` | The commit, its tree, the toolchain, each artifact's digest, notarization, and Gatekeeper verdict. |
| `install.sh`, `install.ps1` | The installers. |

The engine ships as a second artifact, not inside an archive. Coder looks
for `microcoder` beside the running `openagents`
(`coder::task::local::controller`), so the installers put both in one
directory, `~/.openagents/bin` by default, and verify both before replacing
either. A Windows artifact's URL has no extension; the sums file names it
with `.exe`.

The bucket root holds other lines (the older OpenAgents CLI's
`openagents-<version>-<platform>`, `SHA256SUMS-<version>`, `stable`, `rc`;
`openagents-coder-api-*`; Coder Terminal's `coder-terminal-*`). Nothing here
writes outside `openagents/`, and nothing there is read or replaced.

## Platforms

| Platform | Target | Built with | Signed |
| --- | --- | --- | --- |
| `macos-aarch64` | `aarch64-apple-darwin` | `cargo` | Developer ID, notarized |
| `macos-x86_64` | `x86_64-apple-darwin` | `cargo` | Developer ID, notarized |
| `linux-x86_64` | `x86_64-unknown-linux-gnu` (glibc 2.28) | `cargo zigbuild` | no |
| `linux-x86_64-musl` | `x86_64-unknown-linux-musl` (static) | `cargo zigbuild` | no |
| `linux-aarch64` | `aarch64-unknown-linux-gnu` (glibc 2.28) | `cargo zigbuild` | no |
| `linux-aarch64-musl` | `aarch64-unknown-linux-musl` (static) | `cargo zigbuild` | no |
| `windows-x86_64` | `x86_64-pc-windows-gnu` | `cargo zigbuild` | no |

All seven build on one Mac: the macOS targets natively, the rest with
`cargo-zigbuild` and `zig`. Install the Rust targets with
`rustup target add <target>` inside the checkout, so the pinned toolchain
gets them. On Windows, `openagents connect`, `labor`, `service`, `ssh`,
`wallet`, and `x402` answer that they need macOS or Linux: they stand on the
host's control socket and service manager, the resident wallet, Unix file
modes, and the system `ssh`'s process groups. The terminal, chat, and Coder
runs build there.

## Cut a release

1. **Bump the version.** Set `version` in `crates/openagents-cli/Cargo.toml`,
   `crates/openagents-terminal/Cargo.toml`, and `crates/microcoder/Cargo.toml`
   to the release, such as `1.0.0-rc.2` or `1.0.0`. `openagents --version`,
   the welcome card, and `microcoder --version` print it. It moves apart from the workspace version. Commit and push it.
2. **Build and check.**
   `CARGO_TARGET_DIR=~/work/openagents-target-release scripts/release/terminal.sh --version 1.0.0-rc.2`
   builds every platform from an archive of `HEAD` and stages the
   artifacts in `dist/releases/openagents/<version>/`. It publishes nothing
   without `--publish`.
3. **Publish and point the channel.** Run it again with
   `--publish --channel rc` (or `stable`). It uploads the artifacts, the sums
   file, and the manifest, reads the sums file back through the public URL,
   then moves the channel.
4. **Install it.** On a Mac and on a Linux computer, install into a
   temporary home with the published `install.sh`, run `openagents
   --version`, and open `openagents terminal --scratch`.
5. **Clean up.** `dist/` is a staging folder and is not kept; the bucket and
   the published manifest are the record. Delete
   `dist/releases/openagents/<version>/` once the release is published.

`--publish-installers` uploads `scripts/install/openagents.sh` and `.ps1`
from the commit as `install.sh` and `install.ps1`. Run it when they change.

## What the script refuses

- **A malformed version.** Stable is `X.Y.Z`; a candidate is `X.Y.Z-rc.N`
  with `N` a decimal with no leading zeros. `1.0.0-rc1` and `1.0.0-rc.01` are
  refused. The owner's "v1.0.0-rc1" is `1.0.0-rc.1`.
- **A version the crates don't carry.** `--version` must equal the `version`
  of both crates at the commit.
- **A published version.** If the bucket holds the version's sums file or
  any of its artifacts, the script stops before building. Uploads use
  `--no-clobber`, so nothing is ever replaced. A new build takes the next
  `rc.N`.
- **A build of the wrong thing.** Each artifact is read with `file` and must
  match its platform's signature (Mach-O arm64 or x86_64, dynamic or static
  ELF for the right machine, PE32+ x86-64).
- **A source that moved.** The build reads an archive of the commit. After
  the builds, the extracted tree is hashed against the commit's tree.
- **A native artifact that doesn't run.** The artifact for this machine must
  print the version from `--version`, and its engine must start.
- **A macOS artifact Gatekeeper refuses.** Both binaries are signed with the
  `Developer ID Application: OpenAgents, Inc.` identity under the hardened
  runtime, with `scripts/release/openagents-terminal.entitlements` (wasmtime
  needs JIT pages). Identifiers: `com.openagents.terminal.openagents` and
  `com.openagents.terminal.microcoder`. They go to `notarytool` in one
  submission per platform. A bare Mach-O cannot carry a stapled ticket, so
  Gatekeeper reads it from Apple's ticket store, which can lag `Accepted` by
  minutes. The script runs `spctl --assess -vv -t install` on each, 60
  seconds apart, up to 45 times (`OPENAGENTS_RELEASE_ASSESS_DELAY`,
  `OPENAGENTS_RELEASE_ASSESS_ATTEMPTS`), and publishes nothing until every
  verdict is `accepted` with `source=Notarized Developer ID`.
- **A channel past a gap.** A channel moves only after the bucket listing
  holds both artifacts, and the sums file names both, for all seven
  platforms. `--allow-partial` publishes what built and leaves the channel
  where it was. `--point-channel NAME --version V` moves a channel to a
  version already covered, building nothing.

Signing credentials come from the file `OPENAGENTS_NOTARY_ENV` names
(default `~/work/.secrets/appstoreconnect.env`: `ASC_API_KEY_ID`,
`ASC_API_ISSUER_ID`, `ASC_API_PRIVATE_KEY_PATH`,
`OA_DEVELOPER_ID_APPLICATION`), the same file
`scripts/desktop/package-macos.sh --notary-env` reads, or a saved
`NOTARY_KEYCHAIN_PROFILE`. The bucket is written with the gcloud
configuration in `CLOUDSDK_CONFIG` (default
`~/work/.secrets/gcloud-sa-config`, the automation service account).

`scripts/test-release-terminal.sh` tests the version grammar, the channel
coverage rule, and the installer against a local server (checksum mismatch,
unpublished version, missing platform), with no bucket and no network.

## Releases

| Version | Channel | Commit | Date |
| --- | --- | --- | --- |
| `1.0.0-rc.1` | `rc` | `8701d2f8bf` | 2026-10-01 |
| `1.0.0-rc.2` | `rc` | `68662bd344` | 2026-10-01 |

`1.0.0-rc.1` built all seven platforms on one Mac. Notarization:
`cfb4306d-b2f3-4b31-8a03-e8a36c9856a2` (macos-aarch64) and
`ad204ab0-8f6b-413a-8b9f-1335e95d8acf` (macos-x86_64), both `Accepted`;
Gatekeeper accepted all four macOS binaries on the first assessment, and
again with a browser quarantine flag after a public install. Installed with
the published `install.sh` into a temporary home on a Mac (macos-aarch64)
and on NixOS (linux-x86_64): `openagents --version` printed
`openagents 1.0.0-rc.1 (OpenAgentsInc/openagents 8701d2f8bf clean)`, and
`openagents terminal --scratch` in a pty showed the `OpenAgents v1.0.0-rc.1`
welcome card and an answer from the live chat worker. The Windows build was
not run on Windows.

`1.0.0-rc.2` built all seven platforms on one Mac from `68662bd344`
([#10126](https://github.com/OpenAgentsInc/openagents/issues/10126)), and
the `rc` channel moved to it. Notarization:
`82f546ab-b194-4b20-bd5b-5188be45e5fe` (macos-aarch64) and
`6bc9e85a-b689-4046-8c3d-6a9aa4b4a5e0` (macos-x86_64), both `Accepted`;
Gatekeeper accepted all four macOS binaries on the first assessment, and
again after the public install. Installed with the published `install.sh`
(`OPENAGENTS_CHANNEL=rc`) into a temporary home on a Mac (macos-aarch64):
`openagents --version` printed
`openagents 1.0.0-rc.2 (OpenAgentsInc/openagents 68662bd344 clean)`, and
`openagents terminal --scratch` in a pty showed the `OpenAgents v1.0.0-rc.2`
welcome card and answered a question from the live chat worker. The Linux
and Windows builds were not run on those systems.
