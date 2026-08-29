# Release the OpenAgents CLI

`ops/release-cli.sh` builds the `openagents-cli` crate for every platform the
public installer knows how to ask for, signs and notarizes the macOS artifacts,
writes a checksum file, and publishes the result to the release bucket. An
operator runs it from a terminal. There is no hosted build service and no
GitHub Actions workflow.

## What the installer expects

The installer served at `https://openagents.com/install.sh` is the contract.
Under a base URL of `https://openagents.com/releases` it fetches these shapes:

| Path | Body |
| --- | --- |
| `<base>/<channel>` | A bare version string, such as `0.1.0` |
| `<base>/openagents-<version>-<platform>` | The CLI executable |
| `<base>/openagents-coder-api-<version>-<platform>` | The local inference door `coder --dev` starts |
| `<base>/SHA256SUMS-<version>` | One `<sha256>  <name>` line per object |

The platform strings are `macos-aarch64`, `macos-x86_64`, `linux-x86_64`,
`linux-x86_64-musl`, `linux-aarch64`, `linux-aarch64-musl`, and
`windows-x86_64`.

A release that names `openagents-coder-api` in `SHA256SUMS` is what the
installer copies into the same bin directory as `openagents`. A historical
sums file with no such entry still installs the CLI alone.

## Version names

`--version` is the published name. The script accepts only two shapes:

| Kind | Form | Example |
| --- | --- | --- |
| Stable | `X.Y.Z` | `0.2.0` |
| Release candidate | `X.Y.Z-rc.N` | `0.2.0-rc.10` |

`N` is a decimal integer with no leading zeros. The script refuses `rc8`,
`rc.8`, `0.2.0-rc8`, `0.2.0-rc8.12`, and any other suffix. Already-published
names such as `0.2.0-rc7` remain fetchable by the installer; this script will
not mint another.

A published `<version, platform>` object is immutable. A new build of the same
line takes the next `rc.N`. The bucket already holds `0.2.0-rc8`,
`0.2.0-rc.8`, `0.2.0-rc9`, `0.2.0-rc.10`, `0.2.0-rc.11`, `0.2.0-rc.12`,
`0.2.0-rc.13`, `0.2.0-rc.14`, `0.2.0-rc.15`, `0.2.0-rc.16`, and
`0.2.0-rc.17`. Do not reuse 8 through 17. The next dotted name is
`0.2.0-rc.18`.

`--version` must already be the `openagents-cli` version in
`crates/openagents-cli/Cargo.toml` and in `Cargo.lock`, and
`docs/changelog/UNRELEASED.md` must name that exact release. The checks run
before any compile, so a mismatched name never writes artifacts.

`--publish` also requires the Cargo completion gate:
`cargo fmt --all -- --check` then `cargo test --workspace`. The script runs
that pair unless `OPENAGENTS_CLI_RELEASE_GATE=passed` records that this
session already did. `--skip-tests` is refused with `--publish`.

Three details of that contract are easy to get wrong, so the script derives
each rather than leaving it to a human:

- The artifact URL never carries a file extension, on any platform. The
  installer computes the URL before it branches on Windows and never revisits
  it.
- The `SHA256SUMS` entry for Windows *does* carry `.exe`, because the installer
  appends that suffix to the name it searches for. The published object and the
  checksum line therefore disagree by design. Changing either one alone breaks
  every Windows install with a checksum mismatch.
- The glibc Linux artifacts carry no libc suffix and the musl ones do. Renaming
  the glibc artifacts to `linux-x86_64-gnu` would be tidier and would strand
  every installer already in circulation, which asks for the unsuffixed name.

## The two Linux builds

A glibc-linked executable does not run on a musl system. The kernel cannot find
the interpreter named in the binary's `PT_INTERP` and reports it as `no such
file or directory` against a path that plainly exists, which is close to the
least legible failure a first install can produce.

So Linux ships twice per architecture. The `-gnu` targets are dynamically
linked and name `/lib64/ld-linux-x86-64.so.2` or `/lib/ld-linux-aarch64.so.1`.
The `-musl` targets are statically linked and name no interpreter at all.

The installer chooses between them by asking whether the glibc loader for the
reader's architecture exists, which is the question that actually decides it
rather than a proxy for it. Distribution detection would need files a minimal
image may not carry, and `ldd` disagrees with itself across implementations:
GNU's prints a version banner to stdout and exits 0, musl's prints `musl libc`
to stderr, and BusyBox's does neither. Loader presence needs no tools at all.
A system that has the loader can run either artifact and takes the dynamically
linked one; a system that does not can only run the static one. That reads
correctly on Alpine, on a BusyBox or distroless image with no `ldd`, on a
Debian host with the `musl` package installed beside glibc, and on NixOS, where
the loader lives in the Nix store and the static build is genuinely the right
answer. Every way the test can be wrong sends the reader to the artifact that
still runs.

`test/openagents_web/install_script_test.exs` in the `openagents.com`
repository extracts `linux_libc` from the served script and runs it against
fixture roots covering each of those cases.

The `file` signatures in the platform table encode the difference — the gnu
rows require `dynamically linked` and the musl rows require `static` — so a
build that silently produced the wrong flavour cannot be published under the
other one's name.

## Prerequisites

- A Rust toolchain with the five targets installed. Add a missing one with
  `rustup target add <triple>`.
- `cargo-zigbuild` and `zig`, which cross-compile the Linux and Windows targets
  from macOS.
- `gcloud`, for publishing.
- For macOS artifacts, the Developer ID Application certificate in the login
  keychain and an App Store Connect API key. The script reads the key id,
  issuer, private key path, and signing identity from the operator secret file
  named by `OPENAGENTS_NOTARY_ENV`.

## Build without publishing

Start here. The script stages everything under `dist/releases/<version>/`,
which is ignored by Git, and reports what it built:

```sh
ops/release-cli.sh --version 0.1.0-rc.1
```

Restrict the attempt to a subset while you iterate:

```sh
ops/release-cli.sh --version 0.1.0-rc.1 --targets "macos-aarch64 linux-x86_64"
```

## Publish

```sh
ops/release-cli.sh --version 0.1.0-rc.1 --publish
```

Publishing uploads the artifacts and the checksum file. It does not move any
channel, so nothing that resolves `stable` sees the new version yet.

Point a channel at a version only when you intend readers to receive it:

```sh
ops/release-cli.sh --version 0.1.0 --publish --channel stable
```

### Publish one Mac first

An Apple-aarch64-only RC is a partial release. Pass `--allow-partial` so the
script uploads that one platform instead of refusing the missing six. You can
then test the public installer and add the other platforms without changing
the tested Mac binary:

```sh
ops/release-cli.sh --version 0.2.0-rc.10 \
  --targets "macos-aarch64" --publish --allow-partial --allow-prerelease-channel

ops/release-cli.sh --version 0.1.0 \
  --targets "macos-aarch64" --publish --allow-partial

ops/release-cli.sh --version 0.1.0 \
  --targets "macos-x86_64 linux-x86_64 linux-x86_64-musl linux-aarch64 linux-aarch64-musl windows-x86_64" \
  --publish --channel stable
```

The second pass merges the first pass's checksum entry into the completed sums
file. Published `<version, platform>` artifacts are immutable. If you rebuild a
published target and its checksum differs, the script refuses the release
instead of exposing a binary and checksum that disagree. Use a new version when
you need to replace a published target.

## What the script refuses

Each refusal exists because the failure it prevents is silent.

**A version that is not `X.Y.Z` or `X.Y.Z-rc.N`.** `0.2.0-rc8` and `rc8`
look like release candidates and are not. The next RC of a line is the next
integer `N`, never a rebuild of a name that already left the bucket.

**A `--version` that the source tree does not already carry.** The crate
manifest, `Cargo.lock`, and `docs/changelog/UNRELEASED.md` must name the
same version. Threading `OPENAGENTS_CLI_RELEASE_VERSION` does not replace
that agreement.

**A publish without the Cargo completion gate.** `--publish` runs
`cargo fmt --all -- --check` and `cargo test --workspace` unless
`OPENAGENTS_CLI_RELEASE_GATE=passed`. `--skip-tests` cannot waive that.

**A binary that does not match its platform name.** After every build the
script reads the produced file's actual Mach-O, ELF, or PE header and compares
it against the header that platform must have. A cross-compilation that fails
while leaving a stale or host-native binary at the output path would otherwise
publish a macOS executable under a `linux-x86_64` name, and the first person to
learn about it would be a reader whose install produced a file their kernel
refuses to run.

**A partial release, unless you say so.** If any requested platform fails to
build, the script names exactly which ones and exits rather than publishing.
The installer reports a bare download failure, not "unsupported platform", so a
channel pointing at a version that only covers three platforms looks to
everyone else like an outage. Pass `--allow-partial` when a partial release is
what you actually want.

**A channel pointed at a release that does not cover every platform.** The
partial-release check above only sees platforms that were attempted, so
narrowing `--targets` makes a one-platform build look complete. That is fine
for a rehearsal and fatal for a channel, because a channel is what readers
resolve without naming a version. Coverage is therefore judged against the
whole platform table rather than against the request, and only at the moment a
channel is about to be claimed — before anything is uploaded.

**A prerelease claiming a channel.** A version with a suffix, such as
`0.1.0-rc.1`, cannot become the target of a channel without
`--allow-prerelease-channel`. Rehearsals publish release candidates; the
channel a reader resolves by default should keep meaning a real release.

**An unsigned or unnotarized macOS artifact.** Signing failure, a missing
secret file, or a notarization result other than `Accepted` stops the run. Pass
`--skip-notarization` to build macOS artifacts without submitting them, which
is useful while iterating and is recorded as `skipped` in the manifest.

**A changed artifact under a published version.** The release script keeps a
published `<version, platform>` artifact byte-for-byte. This rule matters most
for signed macOS builds, whose signatures can differ across builds. Replacing
an artifact and its checksum requires two object writes, so either write order
creates an installer failure window. Publish replacement bytes under a new
version instead.

## Why the artifacts are bare binaries rather than tarballs

Issue 66 asked for `openagents-<os>-<arch>.tar.gz`. The release publishes bare
executables instead. This is the deliberate shape, not an unfinished step.

A tarball would buy three things. It compresses, which matters to whoever pays
for egress; it carries a directory of files, which matters when a release is
more than independently named objects; and on macOS it is a container, which is
the only thing Apple can staple a notarization ticket to.

None of the three pays here. The binaries are already stripped and the wire is
already compressed by the transport where it helps. A release is two named
executables per platform — `openagents-<version>-<platform>` and
`openagents-coder-api-<version>-<platform>` — each with its own sums line, so
an archive would exist only to be immediately unpacked.
And the stapling argument runs backwards once you follow it: Apple cannot
staple a ticket to a bare Mach-O — `xcrun stapler staple` fails with error 73 —
but the thing stapling buys is offline Gatekeeper verification, and Gatekeeper
is not consulted on this install path at all, because `curl` sets no
`com.apple.quarantine` attribute on what it writes. The artifacts are signed
and notarized regardless, so the paths where quarantine *does* apply still
succeed; `spctl --assess -t install` reports `source=Notarized Developer ID`
from Apple's online lookup of the ticket recorded against the code directory
hash. The script prints that assessment during every macOS build.

What a tarball would cost is concrete. Unpacking is a second failure mode
between a verified download and an executable on disk, and `tar` is one more
tool the installer would have to find and one more thing to refuse when it is
missing. The checksum would cover the archive rather than the executable, so a
reader verifying a binary they already have could no longer compare it against
the published sums. And the change is not additive: the installer, the sums
file, `oa update`, and every published object name would all have to move
together, breaking every installer already in circulation.

If a release ever becomes more than these two named objects per platform — a
shell completion set, a man page, another sidecar — the calculation changes
and the tarball becomes the right container. Two independently checksummed
executables are not that case: the installer already fetches named objects
and verifies each against `SHA256SUMS`.

## Credential handling

The script passes the App Store Connect key, key id, and issuer to
`notarytool` on each invocation rather than storing a `notarytool` keychain
profile. `notarytool store-credentials` copies the private key into the login
keychain, which would leave a second durable copy of a credential that already
exists on disk. Reading the operator secret file at call time leaves nothing
new behind.

Never print the key, its contents, or a `.p12` password into a log, a commit
message, or an issue comment. Reference them by path and variable name.

## The build manifest

Every run writes `dist/releases/<version>/release-manifest.json` recording, per
platform, the Rust target triple, the builder, the SHA-256, the byte count, the
notarization status, and the notarization submission id. It also records the
Git commit the artifacts were built from and, in `git_clean`, whether the
worktree matched that commit. A dirty build produces artifacts no commit
describes, and someone checking out that sha later would build something else
with no way to know. Rehearsals are routinely built dirty; a release should not
be.

`dist/` is ignored by Git, so keep the manifest with the release record — the
issue or changelog entry the release belongs to. It is the evidence for what
shipped.

## Verifying a published release by hand

```sh
curl -fsSL https://openagents.com/releases/SHA256SUMS-0.1.0-rc.3
curl -fsSL -o oa https://openagents.com/releases/openagents-0.1.0-rc.3-macos-aarch64
shasum -a 256 oa
codesign -dv --verbose=4 oa
spctl --assess -vv -t install oa
```

The Linux pair is worth checking against each other, because the whole point of
shipping two is that they are not the same file:

```sh
curl -fsSL -o oa-gnu  https://openagents.com/releases/openagents-0.1.0-rc.3-linux-x86_64
curl -fsSL -o oa-musl https://openagents.com/releases/openagents-0.1.0-rc.3-linux-x86_64-musl
file oa-gnu oa-musl
```

`oa-gnu` must read `dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2`
and `oa-musl` must read `statically linked`. Those two strings are what the
installer's choice is about.

The musl build only proves itself on a musl system:

```sh
docker run --rm --platform linux/amd64 -v "$PWD/install.sh:/install.sh:ro" \
  alpine:3.20 sh -c 'apk add -q --no-cache curl && sh /install.sh 0.1.0-rc.3 && oa --version'
```

Alpine ships no bash, which is why the installer is POSIX shell. If that
command ever needs `apk add bash` to work, a bashism has crept back in.

## The version the binary reports

`--version` names the release, and the script passes it to the build as
`OPENAGENTS_CLI_RELEASE_VERSION`. `openagents_cli::VERSION` reads it through
`option_env!` and falls back to the crate manifest for any build that is not a
release, which is the honest answer for one. `crates/openagents-cli/build.rs`
declares `cargo:rerun-if-env-changed` on that variable, so a second build under
a different version rebuilds rather than reusing the first one's artifact.

This matters beyond cosmetics. `oa update` compares what the running binary
reports against what the channel pointer resolves to. A binary published as
`0.1.0-rc.2` that reported `0.1.0` would make every update either a no-op or a
perpetual reinstall, depending on which way the comparison fell.

`oa update` still replaces only the CLI binary. A reader who installed with
`install.sh` already has `openagents-coder-api` in the same bin directory;
re-running the installer is what refreshes that sibling today.

## Where `oa update` fits

`oa update` is the installer read from the other end, and it is deliberately
not a second implementation of the trust decisions. It resolves the same
channel pointer, downloads the same artifact, fetches `SHA256SUMS-<version>`
over its own request, refuses a missing sums file, an absent entry, and a
digest mismatch, and only then writes anything.

Two things it knows that the shell script has to work out. The platform is
settled at compile time — a binary knows its own architecture and its own C
library, so there is no libc probe in the update path and no way for one to be
wrong. And the path it replaces comes from `current_exe`, canonicalized, so an
`oa` invoked through the symlinks in `~/.openagents/bin` replaces the file
those symlinks point at and both names keep working.

The replacement is staged beside the target and renamed into place, so a reader
who runs `oa` during an update gets the old binary or the new one and never
half of either.

## There is no CDN in front of the releases, and that is a decision

Issue 66 asked for CDN caching under `https://openagents.com/releases/...`.
There is none. What exists is a Google global external Application Load
Balancer (`sarah-urlmap` → `sarah-backend`, three zonal instance groups in
`us-central1`) with `enableCDN: false`, fronting the Phoenix app, which proxies
`storage.googleapis.com`. Requests already enter Google's edge near the reader
and cross Google's backbone from there; what they do not do is get cached at
that edge.

The measurement that settles whether it matters, taken from a laptop on a
domestic connection against the published 6.5 MB `linux-x86_64` artifact:

| Path | Time | Throughput |
| --- | --- | --- |
| `openagents.com/releases/...` (Phoenix proxy) | 0.66 s | 9.9 MB/s |
| `storage.googleapis.com/...` (direct) | 0.76 s | 8.6 MB/s |

The proxy is not slower than the bucket it proxies. Both runs are saturating
the client's link, not the origin, and the controller streams chunk by chunk so
a 40 MB artifact is never a 40 MB message in the app. The bottleneck a CDN
removes is not the bottleneck that exists.

What a CDN would buy, when the numbers change: repeat downloads served from the
edge instead of `us-central1`, which matters for a fleet installing the same
version, and for readers far from Iowa; and insulation for the app from a
release-day spike, which matters when the app and the download path stop being
the same origin's problem.

What it would take, concretely. Create a backend bucket over
`openagentsgemini-cli-releases` with `--enable-cdn`; add a path matcher to
`sarah-urlmap` sending `/releases/*` to it with a `pathPrefixRewrite` to `/`,
because the bucket is flat and the objects carry no `releases/` prefix; and set
`Cache-Control` on the objects at upload time, since a backend bucket serves
object metadata rather than headers a controller chose. `ops/release-cli.sh`
already sets `public, max-age=60` on channel pointers, which is the half that
is easy to get catastrophically wrong — a channel cached for a year pins every
installer that reads it to the release it named that day — but artifacts and
sums files are uploaded without one and would need
`public, max-age=31536000, immutable` to match what `ReleaseController` sends
today.

Two things to weigh before doing it. `sarah-urlmap` currently has no path
matchers at all and routes the entire site to one default service, so this is a
routing change to production for openagents.com, not an isolated addition.
And it moves `/releases` off `ReleaseController`, which is where the object
name allowlist, the `Range` passthrough, and the cache-lifetime split live;
the bucket is world-readable and flat so the allowlist guards nothing that is
not already public, but the behaviour would have to be re-established in the
URL map rather than assumed.

The recommendation is to leave it until there is traffic that justifies a
production routing change. Do not describe openagents.com as CDN-backed for
releases in the meantime.

## Verifying the Windows artifact

The release machine is an Apple Silicon Mac. It cross-compiles the Windows
build and until now could not run it, so the artifact shipped on the strength
of its PE header alone.

Wine is not the way out of that here. An `x86_64` container under Docker's
emulation inherits the host's 16 KiB page size, and Wine assumes 4 KiB:
`wineboot` aborts on `anon_mmap_fixed: Assertion !((UINT_PTR)start &
host_page_mask) failed` before a prefix is ever created. That is structural,
not a missing package.

A throwaway GCE Windows instance runs the real artifact on a real Windows
kernel, with no RDP and no interactive step, because a startup script's output
lands on the serial console:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config

gcloud compute instances create oa-cli-windows-verify \
  --project openagentsgemini --zone us-central1-a \
  --machine-type e2-standard-2 \
  --image-family windows-2022-core --image-project windows-cloud \
  --metadata-from-file windows-startup-script-ps1=verify.ps1 \
  --no-service-account --no-scopes

gcloud compute instances get-serial-port-output oa-cli-windows-verify \
  --project openagentsgemini --zone us-central1-a | grep OA-PROOF
```

`verify.ps1` downloads the published artifact from `openagents.com`, compares
`Get-FileHash` against the entry in `SHA256SUMS-<version>`, and runs
`--version`, `--help`, and `computer probe`, printing each result with a
greppable prefix. The instance needs no service account: it reads nothing from
Google and fetches the artifact over its external IP like any other reader.

Set `$ProgressPreference = 'SilentlyContinue'` before `Invoke-WebRequest`. The
progress bar rendering to a serial console makes a 9 MB download look like a
hang.

Re-run against a later version by replacing the metadata and resetting the
instance — a Windows startup script runs on every boot:

```sh
gcloud compute instances add-metadata oa-cli-windows-verify ... \
  --metadata-from-file windows-startup-script-ps1=verify.ps1
gcloud compute instances reset oa-cli-windows-verify ...
```

Delete the instance when the release is verified. It exists to answer one
question and costs money while it sits there.
